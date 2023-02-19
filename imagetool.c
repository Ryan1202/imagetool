#include "imagetool.h"
#include "ff.h"
#include "fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
	FILE *fp;
	struct ffi *ffi;
	partition_t *pt[4];

	if (argc < 2) {
		exit(-1);
	} else if (argc < 3) {
		printf("Need Commmand!\n");
		exit(-1);
	}
	fp = fopen(argv[1], "rb+");
	if (fp == NULL) {
		perror("imgtool");
		exit(-1);
	}
	ffi = ff_init(fp, argv[1]);
	if (ffi == NULL) {
		printf("Unknown file format!\n");
		fclose(fp);
		exit(-1);
	}
	fs_init(pt, ffi, fp, 0);
	do_commands(argc - 2, argv + 2, pt, ffi, fp);
	fclose(fp);
	exit(0);
}

partition_t *get_part(char *path, partition_t *pt[4], int *p) {
	int i;
	*p = 0;
	if (path[0] == '/') {
		path++;
		(*p)++;
	}
	if (path[0] != 0 && path[0] == 'p') {
		if ('0' <= path[1] && path[1] <= '9') {
			i = path[1] - '0';
			if (pt[i] != NULL) {
				(*p) += 2;
				if (pt[i]->private_data == NULL) // 如果是扩展分区则private_data为空（不保证能用）
				{
					return get_part(path + 2, pt[i]->childs, p);
				}
				return pt[i];
			}
		}
	}
	return NULL;
}

void do_commands(int argc, char **argv, partition_t *pt[4], struct ffi *ffi, FILE *fp) {
	if (strcmp(argv[0], "copy") == 0) {
		if (argc < 3) {
			printf("Too few arguments!\n");
			exit(-1);
		}
		copy_file(pt, ffi, fp, argv[1], argv[2]);
	} else if (strcmp(argv[0], "copydir") == 0) {
		if (argc < 3) {
			printf("Too few arguments!\n");
			exit(-1);
		}
		copy_dir(pt, ffi, fp, argv[1], argv[2]);
	} else if (strcmp(argv[0], "mkdir") == 0) {
		if (argc < 3) {
			printf("Too few arguments!\n");
			exit(-1);
		}
		mkdir(pt, ffi, fp, argv[1], argv[2]);
	} else {
		printf("Command Error!\n");
	}
}

void copy_file(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst) {
	int i, tmp;
	FILE *from;
	char *to, *p;
	char buf[512];
	partition_t *part;
	struct fnode *parent, *fnode;

	from = fopen(src, "rb");
	if (from == NULL) {
		perror("File open error");
		return;
	}

	p  = src;
	to = dst;

	i = strlen(src) - 1;
	while (i >= 0 && p[i] != '/')
		i--;
	p += i + 1;
	i = 0;

	part = get_part(dst, pt, &i);

	if (part == NULL) {
		printf("Unknown path  \"%s\"!\n", dst);
		fclose(from);
		return;
	}
	to += i;
	parent = part->fsi->opendir(ffi, fp, part, to);
	if (parent == NULL) {
		printf("Can't find directory \"%s\"\n", dst);
		fclose(from);
		return;
	}
	fnode = part->fsi->open(ffi, fp, part, parent, p);
	if (fnode == NULL) {
		fnode = part->fsi->createfile(ffi, fp, part, parent, p, strlen(p));
		if (fnode == NULL) {
			printf("Create file \"%s\" failed!\n", p);
			fclose(from);
			return;
		}
		printf("Create file \"%s\".", src);
	}

	int pos = 0;
	printf("Copying %s\n", src);
	do {
		fseek(from, pos, SEEK_SET);
		tmp = fread(buf, 1, SECTOR_SIZE, from);
		part->fsi->seek(ffi, fp, fnode, pos, SEEK_SET);
		part->fsi->write(ffi, fp, fnode, (uint8_t *)buf, tmp);
		pos += tmp;
	} while (tmp == SECTOR_SIZE);
	fclose(from);
}

void mkdir(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst) {
	int i, len1, len2;
	char *s;
	partition_t *part;
	struct fnode *parent;

	len1 = strlen(src);
	len2 = strlen(dst);
	s	 = malloc(len2 + len1 + 1);
	strncpy(s, dst, len2);
	strncpy(s + len2, src, len1);
	s[len1 + len2] = 0;
	part		   = get_part(dst, pt, &i);
	dst += i;
	parent = part->fsi->opendir(ffi, fp, part, s + i);
	free(s);
	if (parent == NULL) {
		parent = part->fsi->opendir(ffi, fp, part, dst);
		if (parent == NULL) {
			printf("Can't find directory \"%s\"\n", dst);
			return;
		}
		part->fsi->mkdir(ffi, fp, part, parent, src, len1);
		printf("Create directory \"%s\"\n", src);
	}
}
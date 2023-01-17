#include "ff.h"
#include "fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void do_commands(int argc, char **argv, partition_t *pt[4], struct ffi *ffi, FILE *fp);
void copy_file(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst);

int main(int argc, char **argv) {
	FILE *fp;
	struct ffi *ffi;
	partition_t *pt[4];

	if (argc < 2) {
		exit(-1);
	} else if (argc < 3) {
		printf("Need Commmand!");
		exit(-1);
	}
	fp = fopen(argv[1], "rb+");
	if (fp == NULL) {
		perror("imgtool");
		exit(-1);
	}
	ffi = ff_init(fp, argv[1]);
	if (ffi == NULL) {
		printf("Unknown file format!");
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
			printf("Too few arguments!");
			exit(-1);
		}
		copy_file(pt, ffi, fp, argv[1], argv[2]);
	} else {
		printf("Command Error!");
	}
}

void copy_file(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst) {
	int i, tmp;
	FILE *from;
	char *to, *p;
	char buf[512];
	partition_t *part;
	struct fnode *parent, *fnode;

	from = fopen(src, "rb+");
	if (fp == NULL) {
		perror("imgtool:");
		exit(-1);
	}
	p	 = src;
	to	 = dst;
	part = get_part(dst, pt, &i);
	if (part == NULL) {
		printf("Unknown path: %s!\n", dst);
		exit(-1);
	}
	to += i;
	parent = part->fsi->opendir(ffi, fp, part, to);
	if (parent == NULL) { printf("Can't Find %s", dst); }
	fnode = part->fsi->open(ffi, fp, part, parent, p);
	if (fnode == NULL) {
		fnode = part->fsi->createfile(ffi, fp, part, parent, p, strlen(p));
		if (fnode == NULL) {
			printf("Create file %s failed!\n", p);
			exit(-1);
		}
	}

	do {
		tmp = fread(buf, 1, SECTOR_SIZE, from);
		part->fsi->write(ffi, fp, fnode, (uint8_t *)buf, tmp);
	} while (tmp == SECTOR_SIZE);
}
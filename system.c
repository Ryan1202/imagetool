#include "system.h"
#include "ff.h"
#include "fs.h"
#include "imagetool.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void copy_dir(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst) {

#ifdef __linux__
	DIR *dir;
	struct dirent *ptr;

	dir = opendir(src);
	if (dir == NULL) {
		printf("Open dir %s failed!\n", src);
		return;
	}
	printf("enter directory \"%s\"\n", src);

	ptr = readdir(dir);
	while (ptr != NULL) {
		if (strcmp(ptr->d_name, ".") == 0 || strcmp(ptr->d_name, "..") == 0) {
			ptr = readdir(dir);
			continue;
		}
		int len	 = strlen(ptr->d_name);
		int len1 = strlen(src), len2 = strlen(dst);
		char *tmp1 = malloc(len1 + len + 1);
		char *tmp2 = malloc(len2 + len + 1);
		memset(tmp1, 0, len + len1 + 1);
		memset(tmp2, 0, len + len2 + 1);
		strncpy(tmp1, src, len1);
		strncpy(tmp2, dst, len2);
		strncpy(tmp1 + len1, ptr->d_name, len);
		strncpy(tmp2 + len2, ptr->d_name, len);
		if (ptr->d_type == DT_DIR) {
			int i;
			strncat(tmp1, "/", 2);
			strncat(tmp2, "/", 2);
			partition_t *part	= get_part(dst, pt, &i);
			struct fnode *fnode = part->fsi->opendir(ffi, fp, part, tmp2 + i);
			if (fnode == NULL) { mkdir(pt, ffi, fp, ptr->d_name, dst); }

			copy_dir(pt, ffi, fp, tmp1, tmp2);
		} else if (ptr->d_type == DT_REG) {
			copy_file(pt, ffi, fp, tmp1, dst);
		}
		free(tmp1);
		free(tmp2);
		ptr = readdir(dir);
	}

#else
	printf("Unsupport in this Operating System!\n");
#endif
}
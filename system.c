#include "system.h"
#include "ff.h"
#include "fs.h"
#include "imagetool.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void copy_dir(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst) {
	int flag;
#ifdef __linux__
	DIR *dir;
	struct dirent *ptr;

	dir = opendir(src);
	if (dir == NULL) {
		printf("Open dir %s failed!\n", src);
		return;
	}

	ptr = readdir(dir);
#elif _WIN32
	int i = strlen(src);
	char *new;
	if (src[i - 1] == '/') {
		new = malloc(i + 2);
		memcpy(new, src, i);
		new[i]	   = '*';
		new[i + 1] = 0;
	} else {
		new = malloc(i + 3);
		memcpy(new, src, i);
		new[i]	   = '/';
		new[i + 1] = '*';
		new[i + 2] = 0;
	}

	WIN32_FIND_DATA ptr;
	HANDLE handle = FindFirstFile(new, &ptr);
	free(new);
	if (handle == INVALID_HANDLE_VALUE) {
		printf("Open dir %s failed!\n", src);
		return;
	}
#else
	printf("Unsupport this Operating System!\n");
	return;
#endif
	do {
		char *filename = FILE_NAME(ptr);
		if (strcmp(filename, ".") == 0 || strcmp(filename, "..") == 0) { goto next; }
		
		int len	 = strlen(filename);
		int len1 = strlen(src), len2 = strlen(dst);

		char *tmp1 = malloc(len1 + len + 1);
		char *tmp2 = malloc(len2 + len + 1);
		memset(tmp1, 0, len + len1 + 1);
		memset(tmp2, 0, len + len2 + 1);

		strncpy(tmp1, src, len1);
		strncpy(tmp2, dst, len2);
		strncpy(tmp1 + len1, filename, len);
		strncpy(tmp2 + len2, filename, len);

		if (FILE_ATTR(ptr) & FILE_ATTR_DIR) {
			int i;
			strncat(tmp1, "/", 2);
			strncat(tmp2, "/", 2);
			partition_t *part	= get_part(dst, pt, &i);
			struct fnode *fnode = part->fsi->opendir(ffi, fp, part, tmp2 + i);
			if (fnode == NULL) { do_mkdir(pt, ffi, fp, filename, dst); }

			copy_dir(pt, ffi, fp, tmp1, tmp2);

		} else if (FILE_ATTR(ptr) & FILE_ATTR_FILE) {
			copy_file(pt, ffi, fp, tmp1, dst);
		}
		free(tmp1);
		free(tmp2);
	next:
#ifdef __linux__
		ptr = readdir(dir);
		flag = ptr != NULL;
#elif _WIN32
		flag = FindNextFile(handle, &ptr) != 0;
#endif
	} while(flag);
}
#include "ff.h"
#include <stdio.h>
#include <string.h>
#include <stdint.h>

struct ffi *ff_init(FILE *fp, char *filename) {
	struct ffi *ffi;
	char *ext = filename;
	while (*ext != '.')
		ext++;
	ext++;
	if (strncmp(ext, "img", 3) == 0) {
		ffi = &raw_ffi;
	} else {
		return NULL;
	}
	if (ffi->check(fp) != 0) { return NULL; }
	return ffi;
}
#pragma once

#include <stdint.h>
#include <stdio.h>

// 主机文件操作接口
struct ffi {
	int (*check)(FILE *fp);
	void (*init)(FILE *fp);
	void (*read)(FILE *fp, uint8_t *buffer, uint32_t size);
	void (*write)(FILE *fp, uint8_t *buffer, uint32_t size);
	void (*seek)(FILE *fp, long offset, int origin);
};

struct ffi *ff_init(FILE *fp, char *filename);

extern struct ffi raw_ffi;
#pragma once

#include <stdint.h>
#include <stdio.h>

#include "ff.h"

#define SECTOR_SIZE 512

typedef struct _partition_s {
	char *name;
	struct fnode *root;
	int start;
	void *private_data;
	struct fsi *fsi;
	struct _partition_s *childs[4]; // 为扩展分区预留
} partition_t;

struct fnode {
	char *name;
	uint32_t pos, dir_offset, size;
	uint32_t offset;
	struct fnode *parent;
	struct fnode *child;
	struct fnode *next;
	partition_t *part;
};

struct partition {
	uint8_t sign;
	uint8_t start_chs[3];
	uint8_t fs_type;
	uint8_t end_chs[3];
	uint32_t start_lba;
	uint32_t size;
};

struct fsi {
	int (*check)(struct ffi *ffi, FILE *fp, struct partition *pt);
	int (*read_superblock)(struct ffi *ffi, FILE *fp, struct _partition_s *partition);
	struct fnode *(*open)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
						  char *filename);
	struct fnode *(*opendir)(struct ffi *ffi, FILE *fp, struct _partition_s *part, char *path);
	void (*close)(struct fnode *fnode);
	void (*seek)(struct ffi *ffi, FILE *fp, struct fnode *fnode, uint32_t offset, int fromwhere);
	void (*read)(struct ffi *ffi, FILE *fp, struct fnode *fnode, uint8_t *buffer, uint32_t length);
	void (*write)(struct ffi *ffi, FILE *fp, struct fnode *fnode, uint8_t *buffer, uint32_t length);
	struct fnode *(*createfile)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
								char *name, int len);
	void (*delete)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *fnode);
	struct fnode *(*mkdir)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
						   char *name, int len);
	uint8_t (*get_attr)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *fnode);
	void (*set_attr)(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *fnode, uint8_t attr);
};

void fs_init(struct _partition_s *p[4], struct ffi *ffi, FILE *fp, int origin);

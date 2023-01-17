#include "ff.h"
#include "fs.h"
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>

#include "filesystem/fat32.h"

void fs_init(partition_t *p[4], struct ffi *ffi, FILE *fp, int origin) {
	int i;
	struct fsi *fsi;
	uint8_t *buffer = malloc(4 * sizeof(struct partition));
	struct partition *pt;

	ffi->seek(fp, 0x1be, origin);
	ffi->read(fp, (uint8_t *)buffer, 4 * sizeof(struct partition));
	for (i = 0; i < 4; i++) {
		pt = (struct partition *)(buffer + i * sizeof(struct partition));
		if (pt->sign == 0x80 || pt->sign == 0x00) {
			p[i] = malloc(sizeof(partition_t));
			if (pt->fs_type == 0x05 || pt->fs_type == 0x0f) { // 扩展分区（不保证能用）
				fs_init(p[i]->childs, ffi, fp, pt->start_lba);
				continue;
			}
			else if (fat32_fsi.check(ffi, fp, pt) == 0) {
				fsi = &fat32_fsi;
			} else {
				free(p[i]);
				p[i] = NULL;
				continue;
			}
			p[i]->start = pt->start_lba;
			p[i]->fsi = fsi;
			fsi->read_superblock(ffi, fp, p[i]);
		}
	}
}
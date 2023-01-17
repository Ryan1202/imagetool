#include "fat32.h"
#include "../ff.h"
#include "../fs.h"
#include <ctype.h>
#include <math.h>
#include <memory.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define MIN(a, b) ((a) < (b) ? (a) : (b))
#define MAX(a, b) ((a) > (b) ? (a) : (b))

#define DIV_ROUND_UP(x, step) ((x + step - 1) / (step))

struct fsi fat32_fsi = {
	.check			 = &fat32_check,
	.read_superblock = &fat32_readsuperblock,
	.open			 = &FAT32_open,
	.opendir		 = &FAT32_open_dir,
	.close			 = &FAT32_close,
	.read			 = &FAT32_read,
	.write			 = &FAT32_write,
	.createfile		 = &FAT32_create_file,
	.delete			 = &FAT32_delete_file,
};

int fat32_check(struct ffi *ffi, FILE *fp, struct partition *pt) {
	if (pt->fs_type == 0x0b || pt->fs_type == 0x0c) { return 0; }
	return -1;
}

int fat32_readsuperblock(struct ffi *ffi, FILE *fp, struct _partition_s *partition) {
	struct fnode *fnode;
	uint8_t *data		   = malloc(SECTOR_SIZE);
	struct pt_fat32 *fat32 = malloc(sizeof(struct pt_fat32));
	ffi->seek(fp, partition->start * SECTOR_SIZE, SEEK_SET);
	ffi->read(fp, data, SECTOR_SIZE);
	memcpy(fat32, data, SECTOR_SIZE);
	ffi->read(fp, (uint8_t *)&fat32->FSInfo, SECTOR_SIZE);
	free(data);

	if (fat32->FSInfo.FSI_LeadSig == 0x41615252) {
		fat32->fat_start		= partition->start + fat32->BPB_RevdSecCnt;
		partition->private_data = fat32;
		fat32->data_start		= fat32->fat_start + fat32->BPB_NumFATs * fat32->BPB_FATSz32;
		struct FAT32_dir sdir;
		ffi->seek(fp, fat32->data_start * SECTOR_SIZE, SEEK_SET);
		ffi->read(fp, (uint8_t *)&sdir, sizeof(struct FAT32_dir));
		if (sdir.DIR_Attr == FAT32_ATTR_VOLUME_ID) {
			int cnt = 1;
			while (sdir.DIR_Name[cnt] != ' ' && cnt < 11)
				cnt++;
			partition->name = malloc(cnt);
			strncpy(partition->name, (char *)sdir.DIR_Name, cnt);
		}
		fnode			= malloc(sizeof(struct fnode));
		fnode->name		= malloc(sizeof(2));
		fnode->name[0]	= '/';
		fnode->name[1]	= 0;
		fnode->parent	= NULL;
		fnode->part		= partition;
		fnode->pos		= 2;
		partition->root = fnode;
		return 0;
	}
	free(fat32);
	return -1;
}

void FAT32_read(struct ffi *ffi, FILE *fp, struct fnode *fnode, uint8_t *buffer, uint32_t length) {
	struct pt_fat32 *fat32 = fs_FAT32(fnode->part->private_data);
	int cnt				   = fnode->offset / (SECTOR_SIZE * fat32->BPB_SecPerClus), i;
	int pos;
	pos = fnode->pos;
	for (i = 0; i < cnt; i++)
		pos = find_member_in_fat(ffi, fp, fnode->part, pos);
	ffi->seek(fp,
			  ((fnode->offset / SECTOR_SIZE) % fat32->BPB_SecPerClus + fat32->data_start +
			   (pos - 2) * fat32->BPB_SecPerClus) *
				  SECTOR_SIZE,
			  SEEK_SET);
	ffi->read(fp, buffer, length);
}

void FAT32_write(struct ffi *ffi, FILE *fp, struct fnode *fnode, uint8_t *buffer, uint32_t length) {
	int i;
	int pos, tmp;
	uint8_t buf[SECTOR_SIZE];
	struct pt_fat32 *fat32 = fnode->part->private_data;
	struct FAT32_dir *sdir;
	int cnt = fnode->offset / (SECTOR_SIZE * fat32->BPB_SecPerClus);
	time_t timep;
	struct tm *p;
	time(&timep);
	p = gmtime(&timep);

	pos = fnode->pos + fnode->offset / (fat32->BPB_SecPerClus * SECTOR_SIZE);
	for (i = 0; i < cnt; i++)
		pos = find_member_in_fat(ffi, fp, fnode->part, pos);

	int off_sec	 = fnode->offset / SECTOR_SIZE;
	int size_sec = fnode->size / SECTOR_SIZE;
	if (off_sec > size_sec) // 不在一个扇区
	{
		if (off_sec == size_sec + 2) pos = fat32_alloc_clus(ffi, fp, fnode->part, pos);
	}
	if (fnode->offset % (SECTOR_SIZE * fat32->BPB_SecPerClus)) {
		tmp = fnode->offset % SECTOR_SIZE;
		ffi->seek(fp,
				  (fnode->offset / SECTOR_SIZE + fat32->data_start + (pos - 2) * fat32->BPB_SecPerClus) *
					  SECTOR_SIZE,
				  SEEK_SET);
		ffi->read(fp, buf, SECTOR_SIZE);
		memcpy(buf + tmp, buffer, MIN(length, SECTOR_SIZE) - tmp);
		ffi->seek(fp,
				  (fnode->offset / SECTOR_SIZE + fat32->data_start + (pos - 2) * fat32->BPB_SecPerClus) *
					  SECTOR_SIZE,
				  SEEK_SET);
		ffi->write(fp, buf, SECTOR_SIZE);
		tmp = MIN(length, SECTOR_SIZE) - tmp;
		buffer += tmp;
		length -= tmp;
		fnode->offset += tmp;
		if (fnode->offset / SECTOR_SIZE % fat32->BPB_SecPerClus) {
			tmp = MIN(SECTOR_SIZE * fat32->BPB_SecPerClus -
						  fnode->offset % (SECTOR_SIZE * fat32->BPB_SecPerClus),
					  length);
			ffi->write(fp, buffer, tmp);
			buffer += tmp;
			length -= tmp;
			fnode->offset += tmp;
		}
	}
	while (length > 0) {
		ffi->seek(fp, ((pos - 2) * fat32->BPB_SecPerClus + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
		tmp = MIN(SECTOR_SIZE * fat32->BPB_SecPerClus, length);
		ffi->write(fp, buffer, tmp);
		fnode->offset += tmp;
		buffer += tmp;
		length -= tmp;
		if (length <= 0) break;
		tmp = find_member_in_fat(ffi, fp, fnode->part, pos);
		if (tmp >= 0x0fffffff) {
			pos = fat32_alloc_clus(ffi, fp, fnode->part, pos);
		} else {
			pos = tmp;
		}
	}
	ffi->seek(fp,
			  (fnode->dir_offset / SECTOR_SIZE + fat32->data_start +
			   (fnode->parent->pos - 2) * fat32->BPB_SecPerClus) *
				  SECTOR_SIZE,
			  SEEK_SET);
	ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
	sdir = (struct FAT32_dir *)(buf + fnode->dir_offset % SECTOR_SIZE);
	if (fnode->offset + length > fnode->size) // 超出文件大小
	{
		sdir->DIR_FileSize = fnode->offset + length;
		fnode->size		   = fnode->offset + length;
	}
	sdir->DIR_LastAccDate = sdir->DIR_WrtDate = (p->tm_year - 1980) << 9 | p->tm_mon << 5 | p->tm_mday;
	sdir->DIR_WrtTime						  = p->tm_hour << 11 | p->tm_min << 5 | p->tm_sec;
	ffi->seek(fp,
			  (fnode->dir_offset / SECTOR_SIZE + fat32->data_start +
			   (fnode->parent->pos - 2) * fat32->BPB_SecPerClus) *
				  SECTOR_SIZE,
			  SEEK_SET);
	ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
}

struct fnode *FAT32_create_file(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
								char *name, int len) {
	char *buf = malloc(512), filename_short[11];
	unsigned int pos, i = 0, j, len2, name_count = 1, count = 1, name_len, tmp;
	unsigned char checksum = 0;
	struct pt_fat32 *fat32 = part->private_data;
	struct FAT32_long_dir *ldir;
	struct FAT32_dir *sdir;
	struct fnode *fnode = malloc(sizeof(struct fnode));
	uint32_t offset;
	int flag;

	time_t timep;
	struct tm *p;
	time(&timep);
	p = gmtime(&timep);

	fnode->name = malloc(len);
	strncpy(fnode->name, name, len);
	fnode->part	  = part;
	fnode->parent = parent;
	pos			  = parent->pos;
	tmp			  = pos;
	do {
		// 如果到了下一个扇区，重新读取
		if (i % SECTOR_SIZE == 0) {
			offset = (pos - 2) * fat32->BPB_SecPerClus + i / SECTOR_SIZE;
			ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
		}
		/**
		 * 统计短目录项重名个数（只在目录项是长文件名时使用）
		 * 如有重名，则短目录名以以下格式命名: SAMEFI~N.TXT (假设原名为samefilename.txt)
		 * 如果当前数字不够，则将'~'前移
		 */
		if (buf[i % SECTOR_SIZE + 11] != FAT32_ATTR_LONG_NAME) {
			flag = 1;
			for (j = 0; j < 8 && buf[i % SECTOR_SIZE + j] != '~'; j++) {
				if (buf[i % SECTOR_SIZE + j] != name[j]) {
					flag = 0;
					break;
				}
			}
			if (flag) { name_count++; }
		}
		i += 0x20;

		// 如果到了下一个簇，则获取下一个簇号
		if (i / SECTOR_SIZE / fat32->BPB_SecPerClus && i % (SECTOR_SIZE * fat32->BPB_SecPerClus)) {
			tmp = find_member_in_fat(ffi, fp, part, pos);
			// 如果簇不够了，分配新的簇
			if (tmp >= 0x0fffffff) pos = fat32_alloc_clus(ffi, fp, part, pos);
			else pos = tmp;
		}
	} while (buf[i]);
	i -= 0x20;

	flag = 0;
	pos	 = ((pos - 2) * fat32->BPB_SecPerClus) * SECTOR_SIZE + i;
	int len_without_ext;
	int len_ext;

	for (i = 0; i < len && name[i] != '.'; i++)
		if (islower(name[i])) flag |= 0x1;		// 文件名含小写
		else if (isupper(name[i])) flag |= 0x2; // 文件名含大写
	len_without_ext = i;
	len_ext			= len - len_without_ext - 1;
	if (i >= len) len_ext++; // 无扩展名文件
	for (; i < len; i++)
		if (islower(name[i])) flag |= 0x4;		// 扩展名含小写
		else if (isupper(name[i])) flag |= 0x8; // 扩展名含大写

	// 文件名和扩展名混杂大小写或长度过长则作为长目录项
	if (len > 11 || len_ext > 3 || len_without_ext > 8 || (flag & 0x03) == 0x03 || (flag & 0x0c) == 0x0c) {
		len2 = DIV_ROUND_UP(len, 13);

		// 计算数字部分长度(十进制)
		int tmp = name_count / 10;
		while (tmp) {
			count++;
			tmp /= 10;
		}

		name_len = MIN(6, 8 - count - 1);
		for (i = 0; i < 8; i++) {
			if (i < name_len) filename_short[i] = toupper(name[i]);
			else if (i == name_len) filename_short[i] = '~';
			else if (i > name_len && i < 8)
				filename_short[i] = ((name_count / (int)pow(10, 7 - i)) % 10 + '0');
			else if (i < 8) filename_short[i] = ' ';
		}
		for (; i < 11; i++) {
			if (i - 8 < len_ext) {
				filename_short[i] = name[len - len_ext + i - 8];
			}
			else {
				filename_short[i] = ' ';
			}
		}
		FAT32_checksum(filename_short, checksum);
		for (i = 0; i < len2; i++) {
			int k;
			if ((pos + i * 0x20) % SECTOR_SIZE == 0) {
				ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
				ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
				offset = (pos + i * 0x20) / SECTOR_SIZE;
				ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
				ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
			}
			ldir		   = (struct FAT32_long_dir *)(buf + (pos + i * 0x20) % SECTOR_SIZE);
			ldir->LDIR_Ord = len2 - i;
			if (i == 0) ldir->LDIR_Ord |= 0x40;
			int f = 0;
			for (j = 0; (j % 13) < 5; j++) {
				if (j < len - (len2 - i - 1) * 13)
				{
					ldir->LDIR_Name1[j] = name[j];
					f = 1;
				} else {
					if (!f) {
						ldir->LDIR_Name1[j] = 0xffff;
					} else {
						f = 0;
						ldir->LDIR_Name1[j] = 0;
					}
				}
			}
			ldir->LDIR_Attr	  = FAT32_ATTR_LONG_NAME;
			ldir->LDIR_Type	  = 0;
			ldir->LDIR_Chksum = checksum;
			for (; (j % 13) < 11; j++) {
				if (j < len - (len2 - i - 1) * 13)
				{
					ldir->LDIR_Name2[j-5] = name[j];
					f = 1;
				} else {
					if (!f) {
						ldir->LDIR_Name2[j-5] = 0xffff;
					} else {
						f = 0;
						ldir->LDIR_Name2[j-5] = 0;
					}
				}
			}
			ldir->LDIR_FstClusLO = 0;
			for (k = (j % 13); k < 13; k++) {
				if (k < len - (len2 - i - 1) * 13)
				{
					ldir->LDIR_Name3[k - 11] = name[j];
					f = 1;
				} else {
					if (!f) {
						ldir->LDIR_Name3[k - 11] = 0xffff;
					} else {
						f = 0;
						ldir->LDIR_Name3[k - 11] = 0;
					}
				}
			}
		}
		pos += i * 0x20;
		if (pos % SECTOR_SIZE == 0) {
			ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
			ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
			offset = pos / SECTOR_SIZE;
			ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
		}
		for (i = 0; i < 11; i++) {
			buf[pos % SECTOR_SIZE + i] = filename_short[i];
		}
		sdir		   = (struct FAT32_dir *)(buf + pos % SECTOR_SIZE);
		sdir->DIR_Attr = FAT32_ATTR_ARCHIVE;

		sdir->DIR_LastAccDate = sdir->DIR_CrtDate = sdir->DIR_WrtDate =
			(p->tm_year - 1980) << 9 | p->tm_mon << 5 | p->tm_mday;
		sdir->DIR_CrtTime = sdir->DIR_WrtTime = p->tm_hour << 11 | p->tm_min << 5 | p->tm_sec >> 1;
		sdir->DIR_CrtTimeTenth				  = p->tm_sec * 10;

		int file_clus		= fat32_alloc_clus(ffi, fp, part, 0);
		sdir->DIR_FstClusHI = file_clus >> 16;
		sdir->DIR_FstClusLO = file_clus & 0xffff;
		sdir->DIR_FileSize	= 0;
	} else {
		for (i = 0, j = 0; i < 8 && name[i] != '.'; i++, j++)
			buf[pos % SECTOR_SIZE + i] = toupper(name[j]);
		for (; i < 8; i++)
			buf[pos % SECTOR_SIZE + i] = ' ';
		for (j++; i < 11; i++, j++)
			buf[pos % SECTOR_SIZE + i] = toupper(name[j]);
		sdir		   = (struct FAT32_dir *)(buf + pos % SECTOR_SIZE);
		sdir->DIR_Attr = FAT32_ATTR_ARCHIVE;

		sdir->DIR_NTRes = 0;
		if (flag & 0x01) sdir->DIR_NTRes |= FAT32_BASE_L;
		if (flag & 0x04) sdir->DIR_NTRes |= FAT32_EXT_L;

		sdir->DIR_LastAccDate = sdir->DIR_CrtDate = sdir->DIR_WrtDate =
			(p->tm_year - 1980) << 9 | p->tm_mon << 5 | p->tm_mday;
		sdir->DIR_CrtTime = sdir->DIR_WrtTime = p->tm_hour << 11 | p->tm_min << 5 | p->tm_sec >> 1;
		sdir->DIR_CrtTimeTenth				  = p->tm_sec * 10;

		int file_clus		= fat32_alloc_clus(ffi, fp, part, 0);
		sdir->DIR_FstClusHI = file_clus >> 16;
		sdir->DIR_FstClusLO = file_clus & 0xffff;
		sdir->DIR_FileSize	= 0;
	}
	fnode->dir_offset = (pos % (SECTOR_SIZE * fat32->BPB_SecPerClus));
	fnode->pos		  = sdir->DIR_FstClusHI << 16 | sdir->DIR_FstClusLO;
	sdir			  = malloc(sizeof(struct FAT32_dir));
	memcpy(sdir, buf + pos % SECTOR_SIZE, sizeof(struct FAT32_dir));
	ffi->seek(fp, (offset + fat32->data_start) * SECTOR_SIZE, SEEK_SET);
	ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
	free(buf);
	return fnode;
}

void FAT32_delete_file(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *fnode) {
	char *buf = malloc(512);
	unsigned int pos, i;
	struct pt_fat32 *fat32 = part->private_data;
	uint32_t offset;
	uint8_t f = 1;
	offset	  = fat32->data_start + (fnode->parent->pos - 2) * fat32->BPB_SecPerClus +
			 fnode->dir_offset / SECTOR_SIZE;
	ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
	ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
	pos = fnode->dir_offset;
	do {
		if (pos % SECTOR_SIZE == 0 && pos >= SECTOR_SIZE) {
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
			offset -= 1;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
		}
		buf[pos % SECTOR_SIZE] = 0xe5;
		pos -= 0x20;
	} while (buf[pos % SECTOR_SIZE + 11] & FAT32_ATTR_LONG_NAME);
	ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
	ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
	pos = fnode->pos; // 释放文件在文件分配表中对应的簇
	while (f) {
		i = find_member_in_fat(ffi, fp, part, pos);
		if (i >= 0x0fffffff) f = 0;
		fat32_free_clus(ffi, fp, part, 0, pos);
		pos = i;
	}
	free(buf);
}

void FAT32_close(struct fnode *fnode) {
	return;
}

int fat32_alloc_clus(struct ffi *ffi, FILE *fp, partition_t *part, int last_clus) {
	int buf[128];
	int i				   = 3, j;
	struct pt_fat32 *fat32 = part->private_data;
	uint32_t offset		   = 0;
	ffi->seek(fp, fat32->fat_start * SECTOR_SIZE, SEEK_SET);
	ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
	while (buf[i % 128]) {
		if (i % 128 == 0) {
			ffi->seek(fp, SECTOR_SIZE, SEEK_CUR);
			ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
		}
		i++;
	}
	for (j = 0; j < fat32->BPB_NumFATs; j++) {
		offset = fat32->fat_start + j * fat32->BPB_FATSz32 + (i / 128);
		ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
		ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
		buf[i % 128] = 0x0fffffff;
		ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
		ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
		if (last_clus >= 3) {
			offset = fat32->fat_start + j * fat32->BPB_FATSz32 + (last_clus / 128);
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
			buf[last_clus % 128] = i;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->write(fp, (uint8_t *)buf, SECTOR_SIZE);
		}
	}
	return i;
}

int fat32_free_clus(struct ffi *ffi, FILE *fp, partition_t *part, int last_clus, int clus) {
	int buf1[128], buf2[128];
	int j;
	struct pt_fat32 *fat32 = part->private_data;
	uint32_t offset;
	if (last_clus < 3 && clus < 3) return -1;
	for (j = 0; j < fat32->BPB_NumFATs; j++) {
		if (last_clus > 2 && clus > 2) {
			offset = fat32->fat_start + j * fat32->BPB_FATSz32 + last_clus / 128;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf1, SECTOR_SIZE);
			if (clus / 128 != last_clus / 128) {
				offset = fat32->fat_start + j * fat32->BPB_FATSz32 + clus / 128;
				ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
				ffi->read(fp, (uint8_t *)buf2, SECTOR_SIZE);
				buf1[last_clus % 128] = buf2[clus % 128];
				buf2[clus % 128]	  = 0x00;
				ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
				ffi->write(fp, (uint8_t *)buf2, SECTOR_SIZE);
			} else {
				buf1[last_clus % 128] = buf1[clus % 128];
				buf1[clus % 128]	  = 0x00;
			}
			offset = fat32->fat_start + j * fat32->BPB_FATSz32 + last_clus / 128;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->write(fp, (uint8_t *)buf1, SECTOR_SIZE);
		} else if (clus > 2) {
			offset = fat32->fat_start + j * fat32->BPB_FATSz32 + clus / 128;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->read(fp, (uint8_t *)buf1, SECTOR_SIZE);
			buf1[clus % 128] = 0x00;
			ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
			ffi->write(fp, (uint8_t *)buf1, SECTOR_SIZE);
		}
	}
	return 0;
}

struct fnode *FAT32_open(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
						 char *filename) {
	int i, j, x;
	struct FAT32_long_dir *ldir;
	struct FAT32_dir *sdir;
	struct fnode *fnode	   = malloc(sizeof(struct fnode));
	struct pt_fat32 *fat32 = part->private_data;
	uint32_t offset;
	uint8_t flag = 0, f = 1;
	unsigned int cc;
	uint8_t buf[fat32->BPB_SecPerClus * SECTOR_SIZE];
	fnode->part	  = parent->part;
	fnode->parent = parent;
	int len		  = strlen(filename);
	cc			  = parent->pos;

	while (f) {
		if (find_member_in_fat(ffi, fp, part, cc) >= 0x0fffffff) f = 0;
		offset = fat32->data_start + (cc - 2) * fat32->BPB_SecPerClus;
		ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
		ffi->read(fp, (uint8_t *)buf, fat32->BPB_SecPerClus * SECTOR_SIZE);
		for (i = 0x00; i < SECTOR_SIZE * fat32->BPB_SecPerClus; i += 0x20) {
			if (buf[i + 11] == FAT32_ATTR_LONG_NAME) continue;
			if (buf[i] == 0xe5 || buf[i] == 0x00 || buf[i] == 0x05) continue;
			ldir = (struct FAT32_long_dir *)(buf + i) - 1;
			sdir = (struct FAT32_dir *)(buf + i);
			j	 = 0;

			// 如果是长目录项
			while (ldir->LDIR_Attr == FAT32_ATTR_LONG_NAME && ldir->LDIR_Ord != 0xe5) {
				for (x = 0; x < 5; x++) {
					if (j > len && ldir->LDIR_Name1[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name1[x] != (unsigned short)(filename[j++])) goto cmp_fail;
				}
				for (x = 0; x < 6; x++) {
					if (j > len && ldir->LDIR_Name2[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name2[x] != (unsigned short)(filename[j++])) goto cmp_fail;
				}
				for (x = 0; x < 2; x++) {
					if (j > len && ldir->LDIR_Name3[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name3[x] != (unsigned short)(filename[j++])) goto cmp_fail;
				}

				if (j >= len) {
					flag = 1;
					goto cmp_success;
				}

				ldir--;
			}

			// 如果是短目录项
			j = 0;
			if (sdir->DIR_Attr & FAT32_ATTR_DIRECTORY) continue;
			for (x = 0; x < 8; x++) {
				if (sdir->DIR_Name[x] == ' ') {
					if (!(sdir->DIR_Attr & FAT32_ATTR_DIRECTORY)) {
						if (filename[j] == '.') continue;
						else if (sdir->DIR_Name[x] == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					} else goto cmp_fail;
				} else if ((sdir->DIR_Name[x] >= 'A' && sdir->DIR_Name[x] <= 'Z') ||
						   (sdir->DIR_Name[x] >= 'a' && sdir->DIR_Name[x] <= 'z')) {
					if (sdir->DIR_NTRes & FAT32_BASE_L) {
						if (j < len && sdir->DIR_Name[x] + 32 == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					} else {
						if (j < len && sdir->DIR_Name[x] == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					}
				} else if (sdir->DIR_Name[x] >= '0' && sdir->DIR_Name[x] <= '9') {
					if (j < len && sdir->DIR_Name[x] == filename[j]) {
						j++;
						continue;
					} else goto cmp_fail;
				} else {
					j++;
				}
			}
			j++;
			for (x = 0; x < 3; x++) {
				if ((sdir->DIR_Ext[x] >= 'A' && sdir->DIR_Ext[x] <= 'Z') ||
					(sdir->DIR_Ext[x] >= 'a' && sdir->DIR_Ext[x] <= 'z')) {
					if (sdir->DIR_NTRes & FAT32_BASE_L) {
						if (j < len && sdir->DIR_Ext[x] + 32 == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					} else {
						if (j < len && sdir->DIR_Ext[x] == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					}
				} else if (sdir->DIR_Ext[x] >= '0' && sdir->DIR_Ext[x] <= '9') {
					if (j < len && sdir->DIR_Ext[x] == filename[j]) {
						j++;
						continue;
					} else goto cmp_fail;
				} else if (sdir->DIR_Ext[x] == ' ') {
					if (sdir->DIR_Ext[x] == filename[j]) {
						if (sdir->DIR_Ext[x] == filename[j]) {
							j++;
							continue;
						} else goto cmp_fail;
					}
				} else {
					goto cmp_fail;
				}
			}
			flag = 1;
			goto cmp_success;
		cmp_fail:;
		}
		cc = find_member_in_fat(ffi, fp, part, cc);
	};
cmp_success:
	if (flag) {
		fnode->name = malloc(len);
		strncpy(fnode->name, filename, len);
		fnode->dir_offset = i;
		fnode->pos		  = (sdir->DIR_FstClusHI << 16 | sdir->DIR_FstClusLO) & 0x0fffffff;
		fnode->size		  = sdir->DIR_FileSize;
		return fnode;
	} else {
		free(fnode);
		return NULL;
	}
}

struct fnode *FAT32_open_dir(struct ffi *ffi, FILE *fp, struct _partition_s *part, char *path) {
	int i;
	struct fnode *fnode = NULL;
	char *name			= malloc(256);

	if (*path == '/') path++;
	fnode = part->root;

	// 从根目录开始
	while (*path != 0) {
		i = 0;
		while (path[i] != '/')
			i++;
		if (i >= 255) { return NULL; }
		strncpy(name, path, i);
		name[i] = 0;
		fnode	= FAT32_find_dir(ffi, fp, part, fnode, name);
	}
	return fnode;
}

struct fnode *FAT32_find_dir(struct ffi *ffi, FILE *fp, struct _partition_s *part, struct fnode *parent,
							 char *name) {
	unsigned int i, j, cc, x, pos;
	uint8_t flag, f;
	uint8_t *buf		   = malloc(((struct pt_fat32 *)part->private_data)->BPB_SecPerClus * SECTOR_SIZE);
	struct pt_fat32 *fat32 = part->private_data;
	struct FAT32_long_dir *ldir;
	struct FAT32_dir *sdir;
	struct fnode *dir;
	int len = strlen(name);
	uint32_t offset;
	dir			= malloc(sizeof(struct fnode));
	dir->parent = parent;
	dir->part	= part;
	flag		= 0;
	f			= 1;
	cc			= parent->pos;
	while (f) {
		if (find_member_in_fat(ffi, fp, part, cc) >= 0x0fffffff) f = 0;
		offset = fat32->data_start + (cc - 2) * fat32->BPB_SecPerClus;
		ffi->seek(fp, offset * SECTOR_SIZE, SEEK_SET);
		ffi->read(fp, (uint8_t *)buf, fat32->BPB_SecPerClus * SECTOR_SIZE);
		for (i = 0x00; i < SECTOR_SIZE * fat32->BPB_SecPerClus; i += 0x20) {
			if (buf[i + 11] == FAT32_ATTR_LONG_NAME) continue;
			if (buf[i] == 0xe5 || buf[i] == 0x00 || buf[i] == 0x05) continue;
			ldir = (struct FAT32_long_dir *)(buf + i) - 1;
			j	 = 0;

			// 如果是长目录项
			while (ldir->LDIR_Attr == FAT32_ATTR_LONG_NAME && ldir->LDIR_Ord != 0xe5) {
				for (x = 0; x < 5; x++) {
					if (j > len && ldir->LDIR_Name1[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name1[x] != (unsigned short)(name[j++])) goto cmp_fail;
				}
				for (x = 0; x < 6; x++) {
					if (j > len && ldir->LDIR_Name2[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name2[x] != (unsigned short)(name[j++])) goto cmp_fail;
				}
				for (x = 0; x < 2; x++) {
					if (j > len && ldir->LDIR_Name3[x] == 0xffff) continue;
					else if (j > len || ldir->LDIR_Name3[x] != (unsigned short)(name[j++])) goto cmp_fail;
				}

				if (j >= len) {
					flag = 1;
					goto cmp_success;
				}

				ldir--;
			}

			// 如果是短目录项
			j	 = 0;
			sdir = (struct FAT32_dir *)(buf + i);
			for (x = 0; x < 11; x++) {
				if (sdir->DIR_Name[x] == ' ') {
					if (sdir->DIR_Attr & FAT32_ATTR_DIRECTORY) {
						if (sdir->DIR_Name[x] == name[j]) {
							j++;
							continue;
						} else {
							goto cmp_fail;
						}
					}
				} else if ((sdir->DIR_Name[x] >= 'A' && sdir->DIR_Name[x] <= 'Z') ||
						   (sdir->DIR_Name[x] >= 'a' && sdir->DIR_Name[x] <= 'z')) {
					if (sdir->DIR_NTRes & FAT32_BASE_L) {
						if (j < len && sdir->DIR_Name[x] + 32 == name[j]) {
							j++;
							continue;
						} else {
							goto cmp_fail;
						}
					} else {
						if (j < len && sdir->DIR_Name[x] == name[j]) {
							j++;
							continue;
						} else {
							goto cmp_fail;
						}
					}

				} else if (j < len && sdir->DIR_Name[x] == name[j]) {
					j++;
					continue;
				} else if (sdir->DIR_Name[x] >= '0' && sdir->DIR_Name[x] <= '9') {
					goto cmp_fail;
				} else {
					goto cmp_fail;
				}
			}
			flag = 1;
			goto cmp_success;
		cmp_fail:;
		}
		cc = find_member_in_fat(ffi, fp, part, cc);
	};
cmp_success:
	if (flag) {
		dir->name		= malloc(len);
		dir->dir_offset = i;
		dir->pos		= (sdir->DIR_FstClusHI << 16 | sdir->DIR_FstClusLO);
		free(buf);
		return dir;
	} else {
		free(dir);
		free(buf);
		return NULL;
	}
}

unsigned int find_member_in_fat(struct ffi *ffi, FILE *fp, struct _partition_s *part, int i) {
	unsigned int buf[SECTOR_SIZE / sizeof(unsigned int)], next_clus;
	struct pt_fat32 *fat32 = part->private_data;
	uint32_t offset		   = fat32->BPB_RevdSecCnt + (i / 128);
	ffi->seek(fp, (offset + part->start) * SECTOR_SIZE, SEEK_SET);
	ffi->read(fp, (uint8_t *)buf, SECTOR_SIZE);
	next_clus = buf[i % 128];
	return next_clus;
}

#pragma once

#include "ff.h"
#include "fs.h"
#include <stdio.h>

void do_commands(int argc, char **argv, partition_t *pt[4], struct ffi *ffi, FILE *fp);
void copy_file(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst);
void mkdir(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst);
partition_t *get_part(char *path, partition_t *pt[4], int *p);

void copy_dir(partition_t *pt[4], struct ffi *ffi, FILE *fp, char *src, char *dst);
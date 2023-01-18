#include "../ff.h"

int raw_check(FILE *fp);
void raw_init(FILE *fp);
void raw_read(FILE *fp, uint8_t *buffer, uint32_t size);
void raw_write(FILE *fp, uint8_t *buffer, uint32_t size);
void raw_seek(FILE *fp, long offset, int origin);

struct ffi raw_ffi = {
    &raw_check,
    &raw_init,
    &raw_read,
    &raw_write,
    &raw_seek
};

int raw_check(FILE *fp) {
	return 0;
}

void raw_init(FILE *fp) {
	return;
}

void raw_read(FILE *fp, uint8_t *buffer, uint32_t size) {
	fread(buffer, size, 1, fp);
	return;
}

void raw_write(FILE *fp, uint8_t *buffer, uint32_t size) {
	fwrite(buffer, size, 1, fp);
	return;
}

void raw_seek(FILE *fp, long offset, int origin) {
	fseek(fp, offset, origin);
	return;
}
CC	=	gcc
RM	=	rm

version=0.3.0

.PHONY: clean build

SRC := 
SRC += imagetool.c fs.c ff.c system.c
SRC += fileformat/raw.c
SRC += filesystem/fat32.c

build:
	$(CC) -o imgtool $(SRC) -lm -DVERSION="\"$(version)\""

dbg:
	$(CC) -o imgtool $(SRC) -lm -g -DVERSION="\"$(version)\"" -DDEBUG

clean:
ifeq ($(OS), Windows_NT)
	$(RM) imgtool.exe
else
	$(RM) imgtool
endif
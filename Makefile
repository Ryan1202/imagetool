CC	=	gcc
RM	=	rm

.PHONY: clean build

SRC := 
SRC += imagetool.c fs.c ff.c system.c
SRC += fileformat/raw.c
SRC += filesystem/fat32.c

build:
	$(CC) -o imgtool $(SRC) -lm

dbg:
	$(CC) -o imgtool $(SRC) -lm -g

clean:
	$(RM) imgtool
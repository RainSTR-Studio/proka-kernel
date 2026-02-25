# Makefile for User-mode System Call Test Program

CC = gcc
CFLAGS = -m64 -nostdlib -nostartfiles -fno-builtin -fno-stack-protector \
         -fno-pic -fno-pie -mno-red-zone -mno-mmx -mno-sse -mno-sse2 \
         -Wall -Wextra -O2

LDFLAGS = -T user.ld -nostdlib

TARGET = syscall_test

.PHONY: all clean

all: $(TARGET)

$(TARGET): syscall_test.o
	$(CC) $(LDFLAGS) -o $@ $^

syscall_test.o: syscall_test.c
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -f $(TARGET) *.o

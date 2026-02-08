# Proka Kernel - Test Makefile
# Copyright (C) RainSTR Studio 2025, All Rights Reserved.

# Build configuration
CC      := gcc
CFLAGS  := -std=gnu99 -ffreestanding -O0 -Wall -Wextra -fPIE -fno-stack-protector
LDFLAGS := -lgcc

# Debug mode
ifeq ($(DEBUG),1)
    CFLAGS += -g
endif

# Source and objects
SRCS    := $(wildcard *.c)
OBJS    := $(patsubst %.c,$(OBJ_DIR)/%.o,$(SRCS))
DEPS    := $(OBJS:.o=.d)

# Ensure OBJ_DIR exists
$(shell mkdir -p $(OBJ_DIR))

.PHONY: all clean

all: $(OBJS)

# Include dependency files
-include $(DEPS)

# Compile C source to object
$(OBJ_DIR)/%.o: %.c
	@echo "  CC      $<"
	$(Q)$(CC) -MMD -MP -c $(CFLAGS) $< -o $@

clean:
	$(Q)rm -f $(OBJS) $(DEPS)

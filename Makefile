# Proka Kernel - Root Makefile
# Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.

.DEFAULT_GOAL := all

# Verbosity control
ifeq ($(V),1)
    Q :=
else
    Q := @
endif

# Core variables
BUILD_DIRS   ?= kernel
TARGET_DIR   ?= $(CURDIR)/target
OBJ_DIR      ?= $(TARGET_DIR)/obj
ISO_DIR      ?= $(TARGET_DIR)/iso
ISO_IMAGE    ?= proka-kernel.iso
INITRD       ?= assets/initrd.cpio

# Build tools & flags
XORRISO      ?= xorriso
XORRISOFLAGS ?= -as mkisofs --efi-boot limine/limine-uefi-cd.bin -quiet
QEMU         ?= qemu-system-x86_64
QEMU_FLAGS   ?= -bios ./assets/OVMF.fd -cdrom $(ISO_IMAGE) --machine q35 -m 1G
QEMU_OUT     ?= -serial stdio
QEMU_EXTRA   ?=

# Profile handling (default to release)
PROFILE      ?= dev
export PROFILE

.PHONY: all debug clean distclean run rundebug menuconfig iso $(BUILD_DIRS) docs-build docs-serve docs-clean

# Documentation targets
doc:
	@echo "Building guide (mdBook)..."
	$(Q)mdbook build
	@echo "Building API documentation (rustdoc)..."
	$(Q)cd kernel && cargo doc --no-deps
	$(Q)rm -rf book/api
	$(Q)cp -r kernel/target/x86_64-unknown-none/doc book/api
	@echo "Documentation built in book/"

docs-serve:
	@echo "Serving documentation..."
	$(Q)mdbook serve --hostname 0.0.0.0

docs-clean:
	@echo "Cleaning documentation..."
	$(Q)mdbook clean
	$(Q)rm -rf book
	$(Q)cd kernel && cargo clean --doc

# Standard build targets
all: $(BUILD_DIRS)

debug:
	$(Q)$(MAKE) PROFILE=dev all

$(BUILD_DIRS):
	@echo "Entering directory: $@"
	$(Q)mkdir -p $(OBJ_DIR)
	$(Q)$(MAKE) -C $@ OBJ_DIR=$(OBJ_DIR) V=$(V)

# ISO image creation
iso: all $(INITRD)
	@echo "Creating ISO image: $(ISO_IMAGE)"
	$(Q)mkdir -p $(ISO_DIR)
	$(Q)cp -r ./assets/rootfs/* $(ISO_DIR)/
	$(Q)cp $(INITRD) $(ISO_DIR)/initrd.cpio
	$(Q)cp ./kernel/kernel $(ISO_DIR)/kernel
	$(Q)$(XORRISO) $(XORRISOFLAGS) $(ISO_DIR) -o $(ISO_IMAGE)
	$(Q)rm -rf $(ISO_DIR)
	@echo "ISO build complete."

# Initrd creation
INITRD_SRC := $(shell find assets/initrd -type f 2>/dev/null)
$(INITRD): $(INITRD_SRC)
	@echo "Creating initrd: $@"
	$(Q)mkdir -p assets
	$(Q)cd assets/initrd && find . -print | cpio -H newc -o > ../initrd.cpio 2>/dev/null

# Execution & Debugging
run: iso
	$(Q)$(QEMU) $(QEMU_FLAGS) $(QEMU_OUT) $(QEMU_EXTRA)

rundebug:
	$(Q)$(MAKE) QEMU_EXTRA="-s -S" run

menuconfig:
	$(Q)$(MAKE) -C kernel menuconfig

fmt:
	$(Q)$(MAKE) -C kernel fmt

# Cleanup
clean: docs-clean
	@for dir in $(BUILD_DIRS); do \
		$(MAKE) -C $$dir clean V=$(V); \
	done
	$(Q)rm -f $(ISO_IMAGE) $(INITRD)
	$(Q)rm -rf $(OBJ_DIR)
	@echo "Cleaned."

distclean: clean
	$(Q)rm -rf $(TARGET_DIR)
	@echo "Full cleanup complete."

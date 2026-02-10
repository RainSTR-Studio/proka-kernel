# Proka Kernel - Root Makefile
# Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.

.DEFAULT_GOAL := help

# OS detection
UNAME_S := $(shell uname -s)

# Verbosity control
ifeq ($(V),1)
    Q :=
else
    Q := @
endif

# Logging macro
define log_info
	@echo "[INFO] $(1)"
endef
define log_success
	@echo "[OK]   $(1)"
endef
define log_warn
	@echo "[WARN] $(1)"
endef
define log_error
	@echo "[ERR]  $(1)"
endef

# Core variables
OUTPUT		 ?= output
BUILD_DIRS   ?= kernel fs
TARGET_DIR   ?= $(CURDIR)/target
OBJ_DIR      ?= $(TARGET_DIR)/obj
ISO_DIR      ?= $(TARGET_DIR)/iso
ISO_IMAGE    ?= $(OUTPUT)/proka-kernel.iso
INITRD       ?= assets/initrd.cpio

# Build tools & flags
XORRISO      ?= xorriso
XORRISOFLAGS ?= -as mkisofs --efi-boot limine/limine-uefi-cd.bin -quiet
QEMU         ?= qemu-system-x86_64

# Accelerator selection
ifeq ($(UNAME_S),Linux)
    QEMU_ACCEL ?= -enable-kvm
endif

QEMU_FLAGS   ?= -bios ./assets/OVMF.fd -cdrom $(ISO_IMAGE) --machine q35 -m 1G $(QEMU_ACCEL)
QEMU_OUT     ?= -serial stdio
QEMU_EXTRA   ?=

# Profile handling (default to release)
PROFILE      ?= dev
export PROFILE

.PHONY: all help debug clean distclean run rundebug menuconfig iso $(BUILD_DIRS) \
        docs-build docs-serve docs-clean test clippy fmt check-tools mkdir

help:
	@echo "$(COLOR_INFO)Proka Kernel Build System$(COLOR_RESET)"
	@echo ""
	@echo "Usage: make [target] [VARIABLES]"
	@echo ""
	@echo "Targets:"
	@echo "  all          Build the kernel"
	@echo "  iso          Create a bootable ISO image"
	@echo "  run          Run the kernel in QEMU (with $(QEMU_ACCEL))"
	@echo "  rundebug     Run the kernel in QEMU with GDB stub (-s -S)"
	@echo "  test         Run kernel tests"
	@echo "  clippy       Run clippy lints"
	@echo "  fmt          Format kernel source code"
	@echo "  menuconfig   Open kernel configuration menu"
	@echo "  clean        Remove build artifacts"
	@echo "  distclean    Full cleanup including target directory"
	@echo "  doc          Build documentation"
	@echo "  check-tools  Verify required build tools are installed"
	@echo ""
	@echo "Variables:"
	@echo "  V=1          Enable verbose output"
	@echo "  PROFILE=dev  Set build profile (dev/release, default: dev)"
	@echo "  QEMU_ACCEL=  Override QEMU accelerator flags"
	@echo ""
	@echo "Detected OS: $(UNAME_S)"

# Tool check
check-tools:
	$(call log_info,Checking build tools...)
	@command -v $(XORRISO) >/dev/null 2>&1 || ( $(call log_error,$(XORRISO) not found); exit 1 )
	@command -v $(QEMU) >/dev/null 2>&1 || ( $(call log_error,$(QEMU) not found); exit 1 )
	@command -v cargo >/dev/null 2>&1 || ( $(call log_error,cargo not found); exit 1 )
	@command -v cpio >/dev/null 2>&1 || ( $(call log_error,cpio not found); exit 1 )
	$(call log_success,All tools found.)

# Documentation targets
doc:
	$(call log_info,Building guide (mdBook)...)
	$(Q)mdbook build
	$(call log_info,Building API documentation (rustdoc)...)
	$(Q)cd kernel && cargo doc --no-deps --features "ttf"
	$(Q)rm -rf book/api
	$(Q)cp -r target/doc book/api
	$(call log_success,Documentation built in book/)

docs-serve:
	$(call log_info,Serving documentation...)
	$(Q)mdbook serve --hostname 0.0.0.0

docs-clean:
	$(call log_info,Cleaning documentation...)
	$(Q)mdbook clean
	$(Q)rm -rf book
	$(Q)cd kernel && cargo clean --doc

# Standard build targets
all: mkdir $(BUILD_DIRS)

$(BUILD_DIRS):
	$(call log_info,Entering directory: $@)
	$(Q)mkdir -p $(OBJ_DIR)
	$(Q)$(MAKE) -C $@ OBJ_DIR=$(OBJ_DIR) V=$(V) OUT_DIR=$(OUTPUT)

# ISO image creation
ROOTFS_SRC := $(shell find assets/rootfs -type f 2>/dev/null)
iso: all $(INITRD) $(ROOTFS_SRC)
	$(call log_info,Creating ISO image: $(ISO_IMAGE))
	$(Q)mkdir -p $(ISO_DIR)
	$(Q)cp -r ./assets/rootfs/* $(ISO_DIR)/
	$(Q)cp $(INITRD) $(ISO_DIR)/initrd.cpio
	$(Q)cp ./$(OUTPUT)/kernel $(ISO_DIR)/kernel
	$(Q)$(XORRISO) $(XORRISOFLAGS) $(ISO_DIR) -o $(ISO_IMAGE)
	$(Q)rm -rf $(ISO_DIR)
	$(call log_success,ISO build complete.)

# Initrd creation
INITRD_SRC := $(shell find assets/initrd -type f 2>/dev/null)
$(INITRD): $(INITRD_SRC)
	$(call log_info,Creating initrd: $@)
	$(Q)mkdir -p assets
	$(Q)cd assets/initrd && find . -print | cpio -H newc -o > ../initrd.cpio 2>/dev/null

# Execution & Debugging
run: iso
	$(Q)$(QEMU) $(QEMU_FLAGS) $(QEMU_OUT) $(QEMU_EXTRA)

rundebug:
	$(Q)$(MAKE) QEMU_EXTRA="-s -S" run

test:
	$(call log_info,Running kernel tests...)
	$(Q)$(MAKE) -C kernel test

clippy:
	$(call log_info,Running clippy...)
	$(Q)$(MAKE) -C kernel clippy

menuconfig:
	$(Q)$(MAKE) -C kernel menuconfig

fmt:
	$(Q)$(MAKE) -C kernel fmt

# Cleanup
clean:
	@for dir in $(BUILD_DIRS); do \
		$(MAKE) -C $$dir clean V=$(V); \
	done
	$(Q)rm -rf $(OUTPUT)
	$(call log_success,Cleaned.)

distclean: clean
	$(Q)rm -rf $(TARGET_DIR)
	$(call log_success,Full cleanup complete.)

# Auxilary target
mkdir:
	$(call log_info,Building guide (mdBook)...)
	$(Q)mkdir -p $(OUTPUT)
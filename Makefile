# Proka Kernel - Rust Kernel Makefile
# Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.

# Disable built-in rules and variables for performance
MAKEFLAGS += -rR
.SUFFIXES:

# Output directory
OUT_DIR := $(CURDIR)/output
# Output binary name
OUTPUT := $(OUT_DIR)/proka-kernel
# Cargo package name
PKG_NAME := proka-kernel
# Rust target triple
RUST_TARGET := x86_64-unknown-none

# Build profile (dev, release)
PROFILE ?= dev
# Map 'dev' to Cargo's 'debug' directory
ifeq ($(PROFILE),dev)
    PROFILE_DIR := debug
else
    PROFILE_DIR := $(PROFILE)
endif

# Verbosity control
ifeq ($(V),1)
    Q :=
else
    Q := @
endif

# Rust compilation flags
RUSTFLAGS := -C relocation-model=static \
             -C code-model=large \
             -C no-redzone \
             -C force-frame-pointers=yes

# Binary path relative to kernel directory
BIN_PATH ?= target/$(RUST_TARGET)/$(PROFILE_DIR)/$(PKG_NAME)

.PHONY: all clean menuconfig fmt clippy

all: $(OUTPUT)

$(OUTPUT): $(BIN_PATH)
	$(Q)mkdir -p $(OUT_DIR)
	$(Q)cp $< $@.elf
	$(Q)objcopy -O binary $@.elf $@
	$(Q)rm -f $(BIN_PATH)
	@echo "[INFO] Kernel binary ready: $@"

$(BIN_PATH): .FORCE
	@echo "[INFO] Building kernel in $(PROFILE) mode..."
	$(Q)RUSTFLAGS="$(RUSTFLAGS)" cargo anaxa build --no-env --target $(RUST_TARGET) --profile $(PROFILE)

clippy:
	$(Q)RUSTFLAGS="$(RUSTFLAGS)" cargo clippy --target $(RUST_TARGET) --all-features

.FORCE:

menuconfig:
	$(Q)cargo anaxa menuconfig

fmt:
	$(Q)cargo fmt

# Documentation targets
doc:
	@echo "[INFO] Building guide (mdBook)..."
	$(Q)mdbook build
	@echo "[INFO] Building API documentation (rustdoc)..."
	$(Q)cargo doc --no-deps
	$(Q)rm -rf book/api
	$(Q)cp -r target/doc book/api
	@echo "[INFO] Documentation has successfully built in book"

docs-clean:
	@echo "[INFO] Cleaning documentation..."
	$(Q)mdbook clean
	$(Q)rm -rf book
	$(Q)cargo clean --doc

clean: docs-clean
	$(Q)cargo clean
	$(Q)rm -rf $(OUT_DIR)

# Proka Kernel - Rust Kernel Makefile
# Copyright (C) RainSTR Studio 2025-2026, All Rights Reserved.

# Disable built-in rules and variables for performance
MAKEFLAGS += -rR
.SUFFIXES:

# Output binary name
OUTPUT     := kernel
# Cargo package name
PKG_NAME   := proka-kernel
# Rust target triple
RUST_TARGET := x86_64-unknown-none

# Build profile (dev, release)
PROFILE    ?= release
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
RUSTFLAGS  ?= -C relocation-model=static

# Binary path relative to kernel directory
BIN_PATH := target/$(RUST_TARGET)/$(PROFILE_DIR)/$(PKG_NAME)

.PHONY: all clean distclean menuconfig fmt clippy test

all: $(OUTPUT)

$(OUTPUT): $(BIN_PATH)
	$(Q)cp $< $@
	@echo "Kernel binary ready: $@"

$(BIN_PATH): .FORCE
	@echo "Building kernel in $(PROFILE) mode..."
	$(Q)RUSTFLAGS="$(RUSTFLAGS)" cargo anaxa build --no-env --target $(RUST_TARGET) --profile $(PROFILE)

clippy:
	$(Q)RUSTFLAGS="$(RUSTFLAGS)" cargo clippy --target $(RUST_TARGET) --all-features

test:
	$(Q)RUSTFLAGS="$(RUSTFLAGS)" cargo test --lib --target $(RUST_TARGET)

.FORCE:

menuconfig:
	$(Q)cargo anaxa menuconfig

fmt:
	$(Q)cargo fmt

clean:
	$(Q)cargo clean
	$(Q)rm -f $(OUTPUT)

distclean: clean

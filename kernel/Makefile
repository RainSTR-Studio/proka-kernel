# Nuke built-in rules and variables.
MAKEFLAGS += -rR
.SUFFIXES:

# This is the name that our final executable will have.
# Change as needed.
override OUTPUT := kernel

# Convenience macro to reliably declare user overridable variables.
override USER_VARIABLE = $(if $(filter $(origin $(1)),default undefined),$(eval override $(1) := $(2)))

# Target architecture to build for. Default to x86_64.
$(call USER_VARIABLE,KARCH,x86_64)

ifeq ($(RUST_TARGET),)
    override RUST_TARGET := $(KARCH)-unknown-none
	ifeq ($(KARCH),riscv64)
    	override RUST_TARGET := riscv64gc-unknown-none-elf
	endif
endif

ifeq ($(PROFILE), dev)
    override RUST_PROFILE := dev
endif

ifeq ($(PROFILE),)
    override RUST_PROFILE := release
endif

override RUST_PROFILE_SUBDIR := $(RUST_PROFILE)
ifeq ($(RUST_PROFILE),dev)
    override RUST_PROFILE_SUBDIR := debug
endif

# Default target.
.PHONY: all
all:
	RUSTFLAGS="-C relocation-model=static" cargo anaxa build --no-env --target $(RUST_TARGET) --profile $(RUST_PROFILE)
	@# Find the compiled binary and copy it to the current directory as 'kernel'
	@BIN_NAME=$$(cd target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR) && find . -maxdepth 1 -perm -111 -type f | head -n 1 | sed 's|^\./||'); \
	if [ -n "$$BIN_NAME" ]; then \
		cp "target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/$$BIN_NAME" $(OUTPUT); \
		echo "Kernel binary ready: $(OUTPUT)"; \
	else \
		echo "Failed to find kernel binary!"; exit 1; \
	fi

# Remove object files and the final executable.
.PHONY: clean
clean:
	cargo clean
	rm -f $(OUTPUT)

.PHONY: distclean
distclean: clean

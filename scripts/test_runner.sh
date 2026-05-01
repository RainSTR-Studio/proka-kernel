#!/bin/bash
#
# Proka Kernel Test Runner
#

# Get directories
SCRIPT_DIR="$(dirname "$0")"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Config
ASSETS_DIR="${PROJECT_ROOT}/assets"
ROOTFS_DIR="${ASSETS_DIR}/rootfs"
INITRD_DIR="${ASSETS_DIR}/initrd"
OVMF_BIOS="${ASSETS_DIR}/OVMF.fd"

TEST_BINARY="$1"
REPORT_DIR="${PROJECT_ROOT}/target/test-reports"
mkdir -p "$REPORT_DIR"

if [[ -z "$TEST_BINARY" ]] || [[ ! -f "$TEST_BINARY" ]]; then
    echo "Error: Test binary not found: $TEST_BINARY"
    exit 1
fi

# Create temp directory
TEMP_DIR=$(mktemp -d)
ISO_FILE="${TEMP_DIR}/test.iso"
OUTPUT_FILE="${TEMP_DIR}/output.log"

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Prepare ISO contents
cp -r "${ROOTFS_DIR}"/* "$TEMP_DIR/"
cp "$TEST_BINARY" "$TEMP_DIR/kernel"

# Generate initrd if exists
if [[ -d "$INITRD_DIR" ]]; then
    (cd "$INITRD_DIR" && find . -print | cpio -H newc -o 2>/dev/null) > "$TEMP_DIR/initrd.cpio"
fi

# Create ISO
xorriso -as mkisofs --efi-boot limine/limine-uefi-cd.bin \
    "$TEMP_DIR" -o "$ISO_FILE" 2>/dev/null

# Run QEMU and capture output
qemu-system-x86_64 \
    -bios "$OVMF_BIOS" \
    -cdrom "$ISO_FILE" \
    -serial stdio \
    -vnc :0 \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -no-reboot 2>&1 | tee "$OUTPUT_FILE"

# Get exit code from PIPESTATUS (before tee)
EXIT_CODE=${PIPESTATUS[0]}
STATUS=$((EXIT_CODE & 0xFF))



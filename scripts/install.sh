#!/bin/bash
# scripts/install.sh

set -e

ISO="output/proka-kernel.iso"

if [ ! -f "$ISO" ]; then
    echo "Error: ISO not found at $ISO"
    echo "Run 'make iso' first"
    exit 1
fi

if [ -z "$1" ]; then
    echo "Usage: sudo make install DEVICE=/dev/sdX"
    echo "Example: sudo make install DEVICE=/dev/sdb"
    echo ""
    echo "Available USB devices:"
    lsblk -d -o NAME,SIZE,MODEL | grep -E "sd[a-z]|nvme"
    exit 1
fi

DEVICE="$1"

# 确认设备
read -p "Will write to $DEVICE. ALL DATA WILL BE LOST! Continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# 卸载所有分区
sudo umount ${DEVICE}* 2>/dev/null || true

# 写入ISO
echo "Writing $ISO to $DEVICE..."
sudo dd if="$ISO" of="$DEVICE" bs=4M status=progress conv=fsync

echo "Done! UEFI-only ISO written to $DEVICE"
echo "Note: This will only boot on UEFI systems (BIOS/Legacy boot not supported)"
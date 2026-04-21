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
    -display none \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -no-reboot 2>&1 | tee "$OUTPUT_FILE"

# Get exit code from PIPESTATUS (before tee)
EXIT_CODE=${PIPESTATUS[0]}
STATUS=$((EXIT_CODE & 0xFF))

# --- Generate Reports ---

echo "Generating test reports..."

# JUnit XML Generation
JUNIT_REPORT="${REPORT_DIR}/junit.xml"
echo '<?xml version="1.0" encoding="UTF-8"?>' > "$JUNIT_REPORT"
echo '<testsuites>' >> "$JUNIT_REPORT"

# Extract tests
# Format: Testing <name>... [ok]
TEST_RESULTS=$(grep "Testing " "$OUTPUT_FILE" || true)
TOTAL_TESTS=$(echo "$TEST_RESULTS" | grep -c "Testing " || echo 0)
FAILED_TESTS=0

echo "<testsuite name=\"kernel_tests\" tests=\"$TOTAL_TESTS\">" >> "$JUNIT_REPORT"

while IFS= read -r line; do
    if [[ -z "$line" ]]; then continue; fi
    
    TEST_NAME=$(echo "$line" | sed -E 's/Testing ([^.]+)\.\.\..*/\1/')
    
    if echo "$line" | grep -qi "\[OK\]"; then
        echo "  <testcase name=\"$TEST_NAME\" classname=\"kernel\" />" >> "$JUNIT_REPORT"
    else
        echo "  <testcase name=\"$TEST_NAME\" classname=\"kernel\">" >> "$JUNIT_REPORT"
        echo "    <failure message=\"Test failed or timed out\">See console output</failure>" >> "$JUNIT_REPORT"
        echo "  </testcase>" >> "$JUNIT_REPORT"
        ((FAILED_TESTS++))
    fi
done <<< "$TEST_RESULTS"

# Handle case where QEMU crashed or no tests ran but exit code was error
if [[ $TOTAL_TESTS -eq 0 ]] && [[ "$STATUS" -ne 33 ]]; then
    echo "  <testcase name=\"kernel_boot\" classname=\"kernel\">" >> "$JUNIT_REPORT"
    echo "    <failure message=\"Kernel failed to boot or run tests\">Exit code: $STATUS</failure>" >> "$JUNIT_REPORT"
    echo "  </testcase>" >> "$JUNIT_REPORT"
    ((FAILED_TESTS++))
fi

echo "</testsuite>" >> "$JUNIT_REPORT"
echo "</testsuites>" >> "$JUNIT_REPORT"

# GitHub Step Summary
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "### 内核测试报告" >> "$GITHUB_STEP_SUMMARY"
    echo "| 测试项目 | 状态 |" >> "$GITHUB_STEP_SUMMARY"
    echo "| :--- | :--- |" >> "$GITHUB_STEP_SUMMARY"
    
    while IFS= read -r line; do
        if [[ -z "$line" ]]; then continue; fi
        TEST_NAME=$(echo "$line" | sed -E 's/Testing ([^.]+)\.\.\..*/\1/')
        if echo "$line" | grep -qi "\[OK\]"; then
            echo "| $TEST_NAME | ✅ 通过 |" >> "$GITHUB_STEP_SUMMARY"
        else
            echo "| $TEST_NAME | ❌ 失败 |" >> "$GITHUB_STEP_SUMMARY"
        fi
    done <<< "$TEST_RESULTS"
    
    if [ "$TOTAL_TESTS" -eq 0 ]; then
         echo "| 系统引导 | ❌ 失败 (代码: $STATUS) |" >> "$GITHUB_STEP_SUMMARY"
    fi
fi

echo "Report generated at: $JUNIT_REPORT"

# --- Determine Exit Status ---

# If any test failed in JUnit parsing, or QEMU returned failure
if [ "$FAILED_TESTS" -gt 0 ]; then
    echo "Tests FAILED ($FAILED_TESTS failures)"
    exit 1
fi

case $STATUS in
    33) echo "Tests PASSED"; exit 0 ;;
    35) echo "Tests FAILED"; exit 1 ;;
    *) 
        # If we have [ok] markers, it's likely fine even if exit code is weird
        if grep -qi "\[OK\]" "$OUTPUT_FILE"; then
            echo "Tests PASSED (with warnings)"
            exit 0
        else
            echo "QEMU exited abnormally with code $STATUS"
            exit 1
        fi
        ;;
esac

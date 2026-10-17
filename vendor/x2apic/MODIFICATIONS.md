# Local modifications

This crate is vendored into Proka‑kernel, copied from upstream x2apic 0.5.0.

Upstream repository: https://github.com/xxx/x2apic
Original authors: see original README.md
License: MIT OR Apache‑2.0

## Changes made:
1. Remove `ptr_internals` unstable feature (removed in modern nightly rust)
2. Replace `core::ptr::Unique<T>` → `core::ptr::NonNull<T>` for compatibility with newer rust toolchain
3. No functional logic changed, only compiler‑compat fixes.


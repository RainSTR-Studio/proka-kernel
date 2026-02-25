//! User memory validation and access
//!
//! This module provides functions to safely access and validate memory
//! pointers provided by user space during system calls.

use x86_64::VirtAddr;

/// Start of user address space
pub const USER_SPACE_START: u64 = 0x0000_0000_0000_0000;
/// End of user address space (canonical address limit)
pub const USER_SPACE_END: u64 = 0x0000_7FFF_FFFF_FFFF;

/// Check if a memory range is entirely within user space
///
/// # Arguments
/// * `ptr` - Starting address
/// * `len` - Length of the range in bytes
///
/// # Returns
/// * `true` if the range is within user space
/// * `false` if any part of the range is in kernel space or invalid
pub fn validate_user_ptr(ptr: *const u8, len: usize) -> bool {
    let start = ptr as u64;
    let end = match start.checked_add(len as u64) {
        Some(e) => e,
        None => return false, // Overflow
    };

    start >= USER_SPACE_START && end <= USER_SPACE_END
}

/// Check if a string is entirely within user space
pub fn validate_user_str(ptr: *const u8) -> bool {
    if ptr.is_null() {
        return false;
    }

    let mut current = ptr;
    while validate_user_ptr(current, 1) {
        unsafe {
            if *current == 0 {
                return true;
            }
            current = current.add(1);
        }
    }

    false
}

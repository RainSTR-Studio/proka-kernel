//! The task manager in syscall.
//!
//! Registered as syscall 0.

use crate::process::ProcType;

/// The process syscall request type.
pub enum ProcessSyscallRequest {
    /// Request to kill tasks.
    KillTasks,

    /// Request to create tasks.
    CreateTasks,
}

impl ProcessSyscallRequest {
    /// Convert to this type from u64.
    #[inline]
    pub fn from_u64(request: u64) -> Self {
        match request {
            0 => Self::KillTasks,
            1 => Self::CreateTasks,
            _ => panic!("Invalid process syscall request: {}", request),
        }
    }
}

/// The entry point of process syscall.
// TODO: Write this function once the structure of memory got refactored.
pub extern "C" fn process(
    request: u64,
    id_or_priority: u64,
    buf: u64,
    len: u64,
    proctyp: u64,
) -> i64 {
    // Get user table
    let user_table: u64;
    unsafe { core::arch::asm!("nop", out("r15") user_table) }; // Get the user table

    // Copy the specified buffer from user space to kernel space.
    // SAFETY: User must ensure that the buffer address that provided is valid.
    let kernel_buf = unsafe { crate::memory::copy_buffer_to_kernel::<u8>(user_table, buf, len) };

    // Check: Is `None` was returned?
    if kernel_buf.is_none() {
        return -1;
    }

    // So we can safely unwrap it.
    let kernel_buf = kernel_buf.unwrap();

    // Parse the request type.
    let request = ProcessSyscallRequest::from_u64(request);

    // Check the request type.
    match request {
        ProcessSyscallRequest::KillTasks => {
            // For killing tasks, the `id` parameters is being ignored.
            // SAFETY: User must ensure that the buffer address that provided is valid.
            let typ = match proctyp {
                0 => ProcType::Normal,
                1 => ProcType::Driver,
                _ => return -1,
            };

            if crate::process::remove(typ, id_or_priority as usize).is_err() {
                return -1;
            };
        }
        ProcessSyscallRequest::CreateTasks => {
            // Create tasks.
            // If `id_or_priority` is larger than u8::MAX, it will cause truncation.
            unsafe {
                if crate::process::create(&kernel_buf, id_or_priority as u8).is_err() {
                    return -1;
                }
            };
        }
    }

    0
}

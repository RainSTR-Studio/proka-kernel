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
                let data = core::slice::from_raw_parts(buf as *const u8, len as usize);
                if crate::process::create(data, id_or_priority as u8).is_err() {
                    return -1;
                }
            };
        }
    }

    0
}

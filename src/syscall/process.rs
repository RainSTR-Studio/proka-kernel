//! The task manager in syscall.
//! 
//! Registered as syscall 0.

use crate::syscall::ReturnType;

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
pub extern "C" fn process(request: u64, _id: u64, _buf: u64, _len: u64, _: u64) -> ReturnType {
    let request = ProcessSyscallRequest::from_u64(request);

    // Check the request type.
    match request {
        ProcessSyscallRequest::KillTasks => {
            // For creating tasks, the `id` parameters is being ignored.
            
        }
        ProcessSyscallRequest::CreateTasks => {
            // Create tasks.
        }
    }

    ReturnType::Success(0)
}
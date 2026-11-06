use num_enum::TryFromPrimitive;

/// The process syscall request type.
#[derive(Debug, PartialEq, Eq, TryFromPrimitive)]
#[repr(u64)]
pub enum ProcessSyscallRequest {
    /// Request to kill tasks.
    KillTasks,
    /// Request to create tasks.
    CreateTasks,
}

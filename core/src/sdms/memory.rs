use num_enum::TryFromPrimitive;

/// Types of this syscall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]#[repr(u64)]
pub enum MemorySyscallType {                                          /// Allocate heap memory.
    Allocate = 0,

    /// Deallocate specified address memory.
    Deallocate = 1, 
}

//! General process management entry and definitions.
/// The status of the current process.
#[repr(u16)]
#[derive(Default, Debug, Clone, Copy)]
pub enum Status {
    /// Means the process is ready and being run.
    #[default]
    Ready = 0,

    /// Means the kernel is now running.
    Running = 1,
}

/// The error of operations of process.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The memory is not enough to create process.
    MemoryNotEnough,

    /// The address is not aligned.
    AddressNotAligned,

    /// The process is not exist.
    ProcessNotExist,

    /// The index is invalid.
    InvalidIndex,

    /// The PKE format is invalid.
    InvalidFormat,

    /// An error about page table.
    PageError,
}

/// The type of processes.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcType {
    /// Normal process.
    Normal,

    /// Driver process.
    Driver,
}

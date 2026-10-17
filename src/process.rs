//! Process management (kernel-level orchestration).
//!
//! Public process types come from `proka-kernel-common`; the x86_64 process
//! loading / page-table logic lives in `proka-kernel-arch::process`.

pub use proka_kernel_common::process::{Error, ProcType, Status};

/// Create a process and push it into the process list by passing a valid
/// PKE format data.
///
/// # Safety
/// Caller must ensure that the data is already mapped.
pub unsafe fn create(data: &[u8], priority: u8) -> Result<(), Error> {
    unsafe { proka_kernel_arch::process::create(data, priority) }
}

/// Remove a process from the process list by type and index.
pub fn remove(proctype: ProcType, index: usize) -> Result<(), Error> {
    proka_kernel_arch::process::remove(proctype, index)
}

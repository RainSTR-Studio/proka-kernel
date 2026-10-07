//! The syscall module (kernel-level orchestration).
//!
//! The x86_64 MSR setup (STAR/LSTAR/SFMASK) is delegated to
//! `proka-kernel-x86_64::syscall::init_msr`; the generic registration
//! into `proka-common::syscall::SYSCALL` is done here.
use proka_common::syscall::{SYSCALL, SyscallEntry};

/// Syscall initializator.
pub fn init() {
    // Architecture-specific half: prepare the MSR-based syscall entry.
    proka_kernel_x86_64::syscall::init_msr();

    // Generic half: register the kernel's own syscalls.
    // For syscall 0 (process management)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 0,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_x86_64::syscall::process::process,
    });

    // For syscall 1 (power action)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 1,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_x86_64::syscall::power::power,
    });

    // For syscall 2 (memory allocation)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 2,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_x86_64::syscall::memory::memory,
    });

    // TODO: Add more types of syscall (0-16)
}

//! The syscall module (kernel-level orchestration).
use proka_kernel_common::syscall::{SYSCALL, SyscallEntry};

/// Syscall initializator.
pub fn init() {
    // Architecture-specific half: prepare the syscall entry.
    proka_kernel_arch::syscall::init_reg();

    // Generic half: register the kernel's own syscalls.
    // For syscall 0 (process management)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 0,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_arch::syscall::process::process,
    });

    // For syscall 1 (power action)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 1,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_arch::syscall::power::power,
    });

    // For syscall 2 (memory allocation)
    SYSCALL.write().push(SyscallEntry {
        sysnum: 2,
        page_table: 0x100000,
        stack: 0xffff8000005ffff0,
        entry: proka_kernel_arch::syscall::memory::memory,
    });

    // TODO: Add more types of syscall (0-16)
}

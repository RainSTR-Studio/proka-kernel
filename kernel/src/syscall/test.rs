//! System Call Tests
//!
//! This module contains tests for the system call mechanism.

use super::*;

/// Test that syscall initialization works
#[test_case]
fn test_syscall_init() {
    // The syscall subsystem should already be initialized by main()
    // Just verify it's enabled
    assert!(msr::is_syscall_enabled());
}

/// Test sys_get_pid from kernel space
#[test_case]
fn test_sys_get_pid() {
    let args = SyscallArgs {
        syscall_num: table::nr::GET_PID,
        arg1: 0,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
        arg6: 0,
        user_rip: 0,
        user_rflags: 0,
        user_rsp: 0,
    };

    let pid = handlers::sys_get_pid(&args);
    assert!(pid > 0);
}

/// Test sys_putc from kernel space
#[test_case]
fn test_sys_putc() {
    let args = SyscallArgs {
        syscall_num: table::nr::PUTC,
        arg1: b'X' as u64,
        arg2: 0,
        arg3: 0,
        arg4: 0,
        arg5: 0,
        arg6: 0,
        user_rip: 0,
        user_rflags: 0,
        user_rsp: 0,
    };

    let ret = handlers::sys_putc(&args);
    assert_eq!(ret, 0);
}

/// Test syscall dispatch table
#[test_case]
fn test_syscall_dispatch() {
    // Test valid syscall numbers
    assert_eq!(
        table::dispatch(table::nr::GET_PID, &SyscallArgs::default()),
        0
    );

    // Test invalid syscall number
    assert_eq!(table::dispatch(999, &SyscallArgs::default()), 38); // ENOSYS
}

impl Default for SyscallArgs {
    fn default() -> Self {
        Self {
            syscall_num: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            arg5: 0,
            arg6: 0,
            user_rip: 0,
            user_rflags: 0,
            user_rsp: 0,
        }
    }
}

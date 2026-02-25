//! System Call Dispatch Table
//!
//! This module implements the system call dispatch mechanism,
//! routing syscall numbers to their respective handlers.

use super::{handlers, SyscallArgs};

/// System call numbers
pub mod nr {
    /// sys_exit - Terminate the current process
    pub const EXIT: u64 = 0;
    /// sys_putc - Output a character (for debugging)
    pub const PUTC: u64 = 1;
    /// sys_ipc_send - Send an IPC message
    pub const IPC_SEND: u64 = 2;
    /// sys_ipc_recv - Receive an IPC message
    pub const IPC_RECV: u64 = 3;
    /// sys_get_pid - Get the current process ID
    pub const GET_PID: u64 = 4;
}

/// Maximum syscall number supported
const MAX_SYSCALL: usize = 5;

/// Error code for unsupported system calls (ENOSYS)
const ENOSYS: u64 = 38;

/// System call handler function type
pub type SyscallHandler = fn(&SyscallArgs) -> u64;

/// System call dispatch table
///
/// This static array maps syscall numbers to their handlers.
/// None entries will return ENOSYS.
static SYSCALL_TABLE: [Option<SyscallHandler>; MAX_SYSCALL] = [
    Some(handlers::sys_exit),     // 0: sys_exit
    Some(handlers::sys_putc),     // 1: sys_putc
    Some(handlers::sys_ipc_send), // 2: sys_ipc_send
    Some(handlers::sys_ipc_recv), // 3: sys_ipc_recv
    Some(handlers::sys_get_pid),  // 4: sys_get_pid
];

/// Dispatch a system call to its handler
///
/// # Arguments
/// * `syscall_num` - The system call number
/// * `args` - The system call arguments
///
/// # Returns
/// * The return value from the handler, or ENOSYS if the syscall is not implemented
pub fn dispatch(syscall_num: u64, args: &SyscallArgs) -> u64 {
    if syscall_num >= MAX_SYSCALL as u64 {
        log::warn!(
            "Invalid syscall number: {} (max: {})",
            syscall_num,
            MAX_SYSCALL
        );
        return ENOSYS;
    }

    match SYSCALL_TABLE[syscall_num as usize] {
        Some(handler) => handler(args),
        None => {
            log::warn!("Unimplemented syscall: {}", syscall_num);
            ENOSYS
        }
    }
}

/// Get the name of a system call for debugging
pub fn syscall_name(num: u64) -> &'static str {
    match num {
        nr::EXIT => "exit",
        nr::PUTC => "putc",
        nr::IPC_SEND => "ipc_send",
        nr::IPC_RECV => "ipc_recv",
        nr::GET_PID => "get_pid",
        _ => "unknown",
    }
}

//! System Call Handlers
//!
//! This module implements the actual system call handlers.
//! Each handler receives the syscall arguments and returns a value.

use super::mem;
use super::SyscallArgs;
use crate::ipc::{self, Message};
use crate::process::{self, scheduler};

/// sys_exit - Terminate the current process
///
/// # Arguments
/// * `args.arg1` (RDI) - Exit code
///
/// # Note
/// This function does not return
pub fn sys_exit(args: &SyscallArgs) -> u64 {
    let exit_code = args.arg1 as i32;
    log::debug!("sys_exit called with code: {}", exit_code);

    // Terminate the current thread
    scheduler::terminate_self();
}

/// sys_putc - Output a character for debugging
///
/// # Arguments
/// * `args.arg1` (RDI) - Character to output
///
/// # Returns
/// * 0 on success
pub fn sys_putc(args: &SyscallArgs) -> u64 {
    let c = args.arg1 as u8 as char;
    crate::serial_print!("{}", c);
    0
}

/// sys_ipc_send - Send an IPC message
///
/// # Arguments
/// * `args.arg1` (RDI) - Target thread ID
/// * `args.arg2` (RSI) - Pointer to message structure (user space)
///
/// # Returns
/// * 0 on success
/// * Error code on failure
pub fn sys_ipc_send(args: &SyscallArgs) -> u64 {
    let target_tid = args.arg1 as u16;
    let msg_ptr = args.arg2 as *const Message;

    // VALIDATION: Check if pointer is in user space
    if !mem::validate_user_ptr(msg_ptr as *const u8, core::mem::size_of::<Message>()) {
        log::warn!("sys_ipc_send: invalid user pointer {:#x}", args.arg2);
        return 22; // EINVAL
    }

    // SAFETY: We've validated the pointer is within user space range.
    // In a real kernel, we would also need to handle Page Faults during access
    // or ensure the memory is mapped and pinned.
    let msg = unsafe { &*msg_ptr };

    match ipc::send(target_tid, msg.clone(), true) {
        Ok(()) => 0,
        Err(e) => {
            log::warn!("sys_ipc_send failed: {:?}", e);
            1 // Error code
        }
    }
}

/// sys_ipc_recv - Receive an IPC message
///
/// # Arguments
/// * `args.arg1` (RDI) - Sender thread ID (0 = any)
/// * `args.arg2` (RSI) - Timeout in milliseconds (0 = infinite)
/// * `args.arg3` (RDX) - Buffer to copy received message into (user space)
///
/// # Returns
/// * 0 on success
/// * Error code on failure or timeout
pub fn sys_ipc_recv(args: &SyscallArgs) -> u64 {
    let sender_tid = if args.arg1 == 0 {
        None
    } else {
        Some(args.arg1 as u16)
    };
    let timeout_ms = if args.arg2 == 0 {
        None
    } else {
        Some(args.arg2)
    };
    let buffer_ptr = args.arg3 as *mut Message;

    // VALIDATION: Check if pointer is in user space
    if !buffer_ptr.is_null()
        && !mem::validate_user_ptr(buffer_ptr as *const u8, core::mem::size_of::<Message>())
    {
        log::warn!("sys_ipc_recv: invalid user pointer {:#x}", args.arg3);
        return 22; // EINVAL
    }

    match ipc::recv(sender_tid, timeout_ms) {
        Ok(msg) => {
            if !buffer_ptr.is_null() {
                // SAFETY: We've validated the pointer range
                unsafe {
                    core::ptr::write(buffer_ptr, msg);
                }
            }
            0
        }
        Err(e) => {
            log::warn!("sys_ipc_recv failed: {:?}", e);
            1 // Error code
        }
    }
}

/// sys_get_pid - Get the current process ID
///
/// # Returns
/// * Current process ID
pub fn sys_get_pid(_args: &SyscallArgs) -> u64 {
    match process::current_pid() {
        Some(pid) => pid as u64,
        None => {
            log::warn!("sys_get_pid: no current process");
            0
        }
    }
}

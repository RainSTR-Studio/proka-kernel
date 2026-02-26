//! System Call Handlers
//!
//! This module implements the actual system call handlers.
//! Each handler receives the syscall arguments and returns a value.

use super::mem;
use super::SyscallArgs;
use crate::ipc::{self, Message};
use crate::memory::paging::vmm::USER_SPACE_END;
use crate::process::{self, scheduler};
use alloc::sync::Arc;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

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

// ============================================================================
// Memory Management System Calls
// ============================================================================

/// mmap protection flags (matches Linux values)
pub const PROT_NONE: u64 = 0x0;
pub const PROT_READ: u64 = 0x1;
pub const PROT_WRITE: u64 = 0x2;
pub const PROT_EXEC: u64 = 0x4;

/// mmap flags (matches Linux values)
pub const MAP_SHARED: u64 = 0x01;
pub const MAP_PRIVATE: u64 = 0x02;
pub const MAP_FIXED: u64 = 0x10;
pub const MAP_ANONYMOUS: u64 = 0x20;

/// sys_mmap - Map memory into user address space
///
/// # Arguments
/// * `args.arg1` (RDI) - Preferred address (can be 0 for any)
/// * `args.arg2` (RSI) - Size in bytes
/// * `args.arg3` (RDX) - Protection flags (PROT_*)
/// * `args.arg4` (R10) - Mapping flags (MAP_*)
/// * `args.arg5` (R8)  - File descriptor (unused for anonymous)
/// * `args.arg6` (R9)  - Offset (unused for anonymous)
///
/// # Returns
/// * Address of mapped region on success
/// * -1 on failure (wrapped as u64)
pub fn sys_mmap(args: &SyscallArgs) -> u64 {
    let addr = args.arg1;
    let size = args.arg2 as usize;
    let prot = args.arg3;
    let flags = args.arg4;

    // Validate size
    if size == 0 {
        log::warn!("sys_mmap: size is 0");
        return !0u64; // -1
    }

    // We only support anonymous mappings for now
    if flags & MAP_ANONYMOUS == 0 {
        log::warn!("sys_mmap: only anonymous mappings supported");
        return !0u64;
    }

    // Validate address if MAP_FIXED is specified
    let preferred_addr = if addr != 0 {
        if addr >= USER_SPACE_END {
            log::warn!("sys_mmap: address {:#x} out of user space", addr);
            return !0u64;
        }
        if flags & MAP_FIXED != 0 {
            // MAP_FIXED: must use exactly this address
            Some(VirtAddr::new(addr))
        } else {
            // Hint address
            Some(VirtAddr::new(addr))
        }
    } else {
        None
    };

    // Convert protection flags to PageTableFlags
    let mut pt_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    if prot & PROT_WRITE != 0 {
        pt_flags |= PageTableFlags::WRITABLE;
    }
    if prot & PROT_EXEC == 0 {
        pt_flags |= PageTableFlags::NO_EXECUTE;
    }

    // Get current process's memory set
    let pcb = match process::current_process() {
        Some(pcb) => pcb,
        None => {
            log::warn!("sys_mmap: no current process");
            return !0u64;
        }
    };

    let pcb_lock = pcb.lock();
    let memory_set = Arc::clone(&pcb_lock.memory_set);
    drop(pcb_lock);

    let mut ms = memory_set.lock();

    match ms.mmap_anon(preferred_addr, size, pt_flags) {
        Ok(mapped_addr) => {
            log::debug!(
                "sys_mmap: mapped {} bytes at {:#x}",
                size,
                mapped_addr.as_u64()
            );
            mapped_addr.as_u64()
        }
        Err(e) => {
            log::warn!("sys_mmap failed: {:?}", e);
            !0u64 // -1
        }
    }
}

/// sys_munmap - Unmap memory from user address space
///
/// # Arguments
/// * `args.arg1` (RDI) - Address to unmap
/// * `args.arg2` (RSI) - Size in bytes
///
/// # Returns
/// * 0 on success
/// * -1 on failure
pub fn sys_munmap(args: &SyscallArgs) -> u64 {
    let addr = VirtAddr::new(args.arg1);
    let size = args.arg2 as usize;

    // Validate address is in user space
    if args.arg1 >= USER_SPACE_END {
        log::warn!("sys_munmap: address {:#x} out of user space", args.arg1);
        return !0u64;
    }

    // Validate size
    if size == 0 {
        log::warn!("sys_munmap: size is 0");
        return !0u64;
    }

    // Get current process's memory set
    let pcb = match process::current_process() {
        Some(pcb) => pcb,
        None => {
            log::warn!("sys_munmap: no current process");
            return !0u64;
        }
    };

    let pcb_lock = pcb.lock();
    let memory_set = Arc::clone(&pcb_lock.memory_set);
    drop(pcb_lock);

    let mut ms = memory_set.lock();

    match ms.munmap(addr, size) {
        Ok(()) => {
            log::debug!("sys_munmap: unmapped {} bytes at {:#x}", size, args.arg1);
            0
        }
        Err(e) => {
            log::warn!("sys_munmap failed: {:?}", e);
            !0u64
        }
    }
}

/// sys_brk - Change heap break
///
/// # Arguments
/// * `args.arg1` (RDI) - New program break (0 to query current)
///
/// # Returns
/// * New program break on success
/// * Current break if argument is 0
/// * -1 on failure
pub fn sys_brk(args: &SyscallArgs) -> u64 {
    let new_brk = args.arg1;

    // Get current process's memory set
    let pcb = match process::current_process() {
        Some(pcb) => pcb,
        None => {
            log::warn!("sys_brk: no current process");
            return !0u64;
        }
    };

    let pcb_lock = pcb.lock();
    let memory_set = Arc::clone(&pcb_lock.memory_set);
    drop(pcb_lock);

    let mut ms = memory_set.lock();

    let current_brk = ms.heap_break();

    // If new_brk is 0, just return current break
    if new_brk == 0 {
        return current_brk.as_u64();
    }

    let new_brk_addr = VirtAddr::new(new_brk);

    // Validate new break is in user space
    if new_brk >= USER_SPACE_END {
        log::warn!("sys_brk: address {:#x} out of user space", new_brk);
        return !0u64;
    }

    // Expand or shrink heap
    if new_brk_addr > current_brk {
        // Expand heap
        match ms.expand_user_heap(new_brk_addr) {
            Ok(()) => new_brk,
            Err(e) => {
                log::warn!("sys_brk expand failed: {:?}", e);
                !0u64
            }
        }
    } else {
        // Shrink heap
        match ms.shrink_user_heap(new_brk_addr) {
            Ok(()) => new_brk,
            Err(e) => {
                log::warn!("sys_brk shrink failed: {:?}", e);
                !0u64
            }
        }
    }
}

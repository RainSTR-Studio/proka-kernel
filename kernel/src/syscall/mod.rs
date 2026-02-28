//! IPC System Call for Proka Kernel
//!
//! This module provides the single syscall entry point for all IPC operations.
//! Instead of multiple syscalls, user programs use `ipc_call` to communicate
//! with kernel services.
//!
//! # Architecture
//!
//! ```text
//! User Program
//!     |
//!     | mov rax, 0  ; IPC_CALL
//!     | syscall
//!     v
//! +------------------+
//! | syscall_entry    |  (assembly)
//! +------------------+
//!     |
//!     v
//! +------------------+
//! | ipc_call_handler |  (Rust)
//! +------------------+
//!     |
//!     v
//! +------------------+
//! | service::dispatch|
//! +------------------+
//!     |
//!     v
//! +------------------+
//! | ProcessService   |
//! | MemoryService    |
//! | ConsoleService   |
//! +------------------+
//! ```

pub mod msr;

#[cfg(test)]
pub mod test;

use crate::process::scheduler;
use crate::service::{self, IpcRequest, IpcRequestHeader};
use core::arch::global_asm;

// Include the assembly entry point
global_asm!(
    r#"
.section .text

.extern ipc_call_handler

.global syscall_entry
syscall_entry:
    # 1. Save user RSP and switch to kernel stack
    mov [rip + ipc_user_rsp_scratch], rsp
    mov rsp, [rip + ipc_kernel_stack_top]

    # 2. Construct IpcCallArgs on kernel stack
    # Order: user_rsp, user_rflags, user_rip, payload_size, payload_ptr, msg_type, reserved, payload_ptr2, service_id, syscall_num
    
    push [rip + ipc_user_rsp_scratch]
    push r11
    push rcx
    push r9      # payload_size
    push r8      # payload_ptr
    push r10     # msg_type
    push rdx     # reserved
    push rsi     # payload_ptr2 (or arg2)
    push rdi     # service_id
    push rax     # syscall number (always 0 for ipc_call)

    # 3. Call Rust handler
    mov rdi, rsp
    call ipc_call_handler

    # 4. Restore registers and return
    add rsp, 8
    pop rdi
    pop rsi
    pop rdx
    pop r10
    pop r8
    pop r9
    pop rcx
    pop r11
    pop rsp

    sysretq

.section .bss
.align 4096
ipc_kernel_stack:
    .space 8192
ipc_user_rsp_scratch:
    .quad 0

.section .text
ipc_kernel_stack_top:
    .quad ipc_kernel_stack + 8192
"#
);

/// IPC call arguments structure
///
/// Matches the order of registers pushed in global_asm!
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IpcCallArgs {
    /// Syscall number (always 0 for ipc_call)
    pub syscall_num: u64,
    /// Target service ID (RDI)
    pub service_id: u64,
    /// Payload pointer (RSI)
    pub payload_ptr: u64,
    /// Reserved (RDX)
    pub _reserved: u64,
    /// Message type (R10)
    pub msg_type: u64,
    /// Payload pointer (R8)
    pub payload_ptr2: u64,
    /// Payload size (R9)
    pub payload_size: u64,
    /// User RIP (saved by hardware in RCX)
    pub user_rip: u64,
    /// User RFLAGS (saved by hardware in R11)
    pub user_rflags: u64,
    /// User stack pointer (RSP)
    pub user_rsp: u64,
}

// External symbol for the syscall entry point
extern "C" {
    fn syscall_entry();
}

/// Main IPC call handler called from assembly
///
/// # Arguments
/// * `args` - Pointer to saved register state on the kernel stack
///
/// # Returns
/// * Return value to be placed in RAX for the user program
#[no_mangle]
pub extern "C" fn ipc_call_handler(args: *const IpcCallArgs) -> u64 {
    let args = unsafe { &*args };

    // Get current thread ID for the request
    let sender = match scheduler::current_tid() {
        Some(tid) => tid,
        None => {
            log::warn!("ipc_call: no current thread");
            return u64::MAX;
        }
    };

    // Build the IPC request
    let request = build_request(args);

    // Dispatch to the appropriate service
    let response = service::dispatch(sender, &request);

    // Return the status code
    // Note: For now we return status in RAX. In the future, we may
    // support returning payload data via a buffer.
    if response.header.status == 0 {
        response.header.retval
    } else {
        // Encode error: high bit set + error code
        (1u64 << 63) | (response.header.status as u64 & 0x7FFFFFFF)
    }
}

/// Build an IPC request from the syscall arguments
fn build_request(args: &IpcCallArgs) -> IpcRequest {
    let header = IpcRequestHeader {
        service: args.service_id as u16,
        msg_type: args.msg_type as u16,
        flags: 0,
        payload_size: args.payload_size,
    };

    // Copy payload from user space if present
    let payload = if args.payload_size > 0 && args.payload_ptr != 0 {
        // Validate payload is in user space
        if args.payload_ptr >= crate::memory::paging::vmm::USER_SPACE_END {
            log::warn!("ipc_call: payload pointer out of user space");
            alloc::vec::Vec::new()
        } else {
            let size = args.payload_size as usize;
            let ptr = args.payload_ptr as *const u8;

            // SAFETY: We validated the pointer is in user space.
            // In a real kernel, we'd handle page faults during access.
            let mut vec = alloc::vec::Vec::with_capacity(size);
            unsafe {
                let slice = core::slice::from_raw_parts(ptr, size.min(1024));
                vec.extend_from_slice(slice);
            }
            vec
        }
    } else {
        alloc::vec::Vec::new()
    };

    IpcRequest { header, payload }
}

/// Initialize the IPC syscall subsystem
///
/// This function configures the MSRs and prepares the kernel
/// to handle IPC calls from user space.
pub fn init() {
    log::info!("Initializing IPC syscall subsystem...");

    // Get the address of the syscall entry point
    let entry_addr = syscall_entry as *const () as u64;

    // SAFETY: We're in kernel initialization, interrupts are disabled
    unsafe {
        msr::configure_syscall_msrs(entry_addr);
    }

    log::info!(
        "IPC syscall subsystem initialized (entry: {:#x})",
        entry_addr
    );
}

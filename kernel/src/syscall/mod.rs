//! System Call Support for Proka Kernel
//!
//! This module provides the x86_64 syscall/sysret mechanism for handling
//! system calls from user space (Ring 3) to kernel space (Ring 0).

pub mod handlers;
pub mod mem;
pub mod msr;
pub mod table;

#[cfg(test)]
pub mod test;

use core::arch::global_asm;

// Include the assembly entry point using global_asm!
// This uses LLVM/GNU assembler syntax.
global_asm!(
    r#"
.intel_syntax noprefix
.section .text

.extern syscall_handler

.global syscall_entry
syscall_entry:
    # 1. Save user RSP and switch to kernel stack
    mov [rip + syscall_user_rsp_scratch], rsp
    mov rsp, [rip + syscall_kernel_stack_top]

    # 2. Construct SyscallArgs on kernel stack
    # Order: user_rsp, user_rflags, user_rip, arg6, arg5, arg4, arg3, arg2, arg1, syscall_num
    
    push [rip + syscall_user_rsp_scratch]
    push r11
    push rcx
    push r9
    push r8
    push r10
    push rdx
    push rsi
    push rdi
    push rax

    # 3. Call Rust handler
    mov rdi, rsp
    call syscall_handler

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
syscall_kernel_stack:
    .space 8192
syscall_user_rsp_scratch:
    .quad 0

.section .text
syscall_kernel_stack_top:
    .quad syscall_kernel_stack + 8192
"#
);

/// System call arguments structure
///
/// Matches the order of registers pushed in global_asm!
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SyscallArgs {
    /// System call number (RAX)
    pub syscall_num: u64,
    /// First argument (RDI)
    pub arg1: u64,
    /// Second argument (RSI)
    pub arg2: u64,
    /// Third argument (RDX)
    pub arg3: u64,
    /// Fourth argument (R10)
    pub arg4: u64,
    /// Fifth argument (R8)
    pub arg5: u64,
    /// Sixth argument (R9)
    pub arg6: u64,
    /// User RIP (saved by hardware in RCX)
    pub user_rip: u64,
    /// User RFLAGS (saved by hardware in R11)
    pub user_rflags: u64,
    /// User stack pointer (RSP)
    pub user_rsp: u64,
}

/// External symbol for the syscall entry point
extern "C" {
    fn syscall_entry();
}

/// Main syscall handler called from assembly
///
/// # Arguments
/// * `args` - Pointer to saved register state on the kernel stack
///
/// # Returns
/// * Return value to be placed in RAX for the user program
#[no_mangle]
pub extern "C" fn syscall_handler(args: *const SyscallArgs) -> u64 {
    // SAFETY: args is valid as it's constructed by assembly
    let args = unsafe { &*args };

    // Dispatch to the appropriate handler
    table::dispatch(args.syscall_num, args)
}

/// Initialize the system call subsystem
///
/// This function configures the MSRs and prepares the kernel
/// to handle system calls from user space.
pub fn init() {
    log::info!("Initializing syscall subsystem...");

    // Get the address of the syscall entry point
    let entry_addr = syscall_entry as *const () as u64;

    // SAFETY: We're in kernel initialization, interrupts are disabled
    unsafe {
        msr::configure_syscall_msrs(entry_addr);
    }

    log::info!("Syscall subsystem initialized (entry: {:#x})", entry_addr);
}

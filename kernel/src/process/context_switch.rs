//! Context switching for Proka Kernel
//!
//! Based on Redox OS context switching implementation.
//! Uses ret instruction to jump to new thread via stack.

use super::thread::Context;
use core::mem::offset_of;

/// Initialize a new thread's context for its first run
///
/// Sets up the stack so that when context switch "returns",
/// it jumps to the entry point
pub fn init_context(
    ctx: &mut Context,
    entry_point: usize,
    stack_top: usize,
    is_kernel_thread: bool,
) {
    // Stack grows down.
    // System V ABI: stack must be 16-byte aligned before call.
    // Call pushes 8-byte return address.
    // So at function entry, (rsp + 8) is 16-byte aligned, i.e., rsp % 16 == 8.

    // We use stack_top - 16 for initial RSP.
    // We write entry_point at stack_top - 16.
    // When ret pops it, RSP becomes stack_top - 8.
    // Since stack_top is 4096-aligned, (stack_top - 8) % 16 == 8.
    let stack_ptr = (stack_top - 16) as *mut u64;

    // SAFETY: stack_ptr is valid (allocated by allocate_kernel_stack)
    unsafe {
        core::ptr::write(stack_ptr, entry_point as u64);
    }

    // Set up context
    ctx.rip = 0; // Not used - we use stack-based return
    ctx.rsp = (stack_top - 16) as u64; // Stack pointer points to entry_point
    ctx.rflags = 0x202; // IF flag set (interrupts enabled)

    if is_kernel_thread {
        ctx.cs = 0x08; // Kernel code segment
        ctx.ss = 0x10; // Kernel data segment
    } else {
        ctx.cs = 0x1B | 3;
        ctx.ss = 0x23 | 3;
    }

    // All registers start at zero
    ctx.rax = 0;
    ctx.rbx = 0;
    ctx.rcx = 0;
    ctx.rdx = 0;
    ctx.rsi = 0;
    ctx.rdi = 0;
    ctx.rbp = 0;
    ctx.r8 = 0;
    ctx.r9 = 0;
    ctx.r10 = 0;
    ctx.r11 = 0;
    ctx.r12 = 0;
    ctx.r13 = 0;
    ctx.r14 = 0;
    ctx.r15 = 0;
}

/// Context switch - save old context and restore new context
///
/// # Safety
/// Must be called with interrupts disabled.
///
/// This function switches to the new context by:
/// 1. Saving callee-saved registers to old context
/// 2. Loading callee-saved registers from new context
/// 3. Switching stack pointer
/// 4. Returning (which pops the new RIP from the new stack)
#[unsafe(naked)]
pub unsafe extern "C" fn switch_context(_old_ctx: *mut Context, _new_ctx: *const Context) {
    use Context as Cx;

    core::arch::naked_asm!(
        // System V AMD64 ABI:
        // - Parameters in rdi (old_ctx), rsi (new_ctx)
        // - Callee-saved: rbx, r12-r15, rbp, rsp

        // Save callee-saved registers to old context
        "mov [rdi + {off_rbx}], rbx",
        "mov [rdi + {off_r12}], r12",
        "mov [rdi + {off_r13}], r13",
        "mov [rdi + {off_r14}], r14",
        "mov [rdi + {off_r15}], r15",
        "mov [rdi + {off_rbp}], rbp",

        // Save RSP (current stack pointer)
        "mov [rdi + {off_rsp}], rsp",

        // Save RFLAGS
        "pushfq",
        "pop QWORD PTR [rdi + {off_rflags}]",

        // Load callee-saved registers from new context
        "mov rbx, [rsi + {off_rbx}]",
        "mov r12, [rsi + {off_r12}]",
        "mov r13, [rsi + {off_r13}]",
        "mov r14, [rsi + {off_r14}]",
        "mov r15, [rsi + {off_r15}]",
        "mov rbp, [rsi + {off_rbp}]",

        // Switch stack pointer FIRST so that if interrupts are enabled by popfq,
        // they use the new thread's stack.
        "mov rsp, [rsi + {off_rsp}]",

        // Load RFLAGS
        "push QWORD PTR [rsi + {off_rflags}]",
        "popfq",

        // Return - this pops the return address from the new stack
        "ret",


        off_rbx = const(offset_of!(Cx, rbx)),
        off_r12 = const(offset_of!(Cx, r12)),
        off_r13 = const(offset_of!(Cx, r13)),
        off_r14 = const(offset_of!(Cx, r14)),
        off_r15 = const(offset_of!(Cx, r15)),
        off_rbp = const(offset_of!(Cx, rbp)),
        off_rsp = const(offset_of!(Cx, rsp)),
        off_rflags = const(offset_of!(Cx, rflags)),
    );
}

/// First context switch from boot to the first thread
///
/// # Safety
/// This function does not return. It switches to the new context and
/// never comes back (until that thread yields).
#[unsafe(naked)]
pub unsafe extern "C" fn first_context_switch(_new_ctx: *const Context) -> ! {
    use Context as Cx;

    core::arch::naked_asm!(
        // Load callee-saved registers from new context
        "mov rbx, [rdi + {off_rbx}]",
        "mov r12, [rdi + {off_r12}]",
        "mov r13, [rdi + {off_r13}]",
        "mov r14, [rdi + {off_r14}]",
        "mov r15, [rdi + {off_r15}]",
        "mov rbp, [rdi + {off_rbp}]",

        // Switch stack pointer
        "mov rsp, [rdi + {off_rsp}]",

        // Load RFLAGS
        "push QWORD PTR [rdi + {off_rflags}]",
        "popfq",

        // Return to start the new thread
        "ret",


        off_rbx = const(offset_of!(Cx, rbx)),
        off_r12 = const(offset_of!(Cx, r12)),
        off_r13 = const(offset_of!(Cx, r13)),
        off_r14 = const(offset_of!(Cx, r14)),
        off_r15 = const(offset_of!(Cx, r15)),
        off_rbp = const(offset_of!(Cx, rbp)),
        off_rsp = const(offset_of!(Cx, rsp)),
        off_rflags = const(offset_of!(Cx, rflags)),
    );
}

//! MSR Configuration for System Calls
//!
//! This module configures the Model-Specific Registers required for
//! syscall/sysret instructions on x86_64.

use crate::interrupts::gdt::CS_KERNEL;
use crate::libs::msr::{rdmsr, wrmsr, EFER_SCE, IA32_EFER, IA32_FMASK, IA32_LSTAR, IA32_STAR};

/// RFLAGS bits to mask during syscall (disable interrupts and trap flag)
/// IF (Interrupt Flag) = bit 9 = 0x200
/// TF (Trap Flag) = bit 8 = 0x100
const SYSCALL_RFLAGS_MASK: u64 = 0x300;

/// Configure MSR registers for syscall/sysret
///
/// # Arguments
/// * `entry_point` - The address of the syscall_entry assembly routine
///
/// # Safety
/// This function modifies critical system registers and must only be called
/// once during kernel initialization.
pub unsafe fn configure_syscall_msrs(entry_point: u64) {
    // Enable syscall instruction via EFER.SCE
    let efer = rdmsr(IA32_EFER);
    wrmsr(IA32_EFER, efer | EFER_SCE);

    // Configure IA32_STAR:
    // [63:48] = User CS (sysret loads CS from here, DS = CS + 8)
    // [47:32] = Kernel CS (syscall loads CS from here, DS = CS + 8)
    // syscall: CS = STAR[47:32], SS = STAR[47:32] + 8
    // sysret:  CS = STAR[63:48], SS = STAR[63:48] + 8
    //
    // For sysret to work correctly:
    // - STAR[63:48] should point to user code segment (0x30)
    // - Then sysret loads CS=0x30, SS=0x30+8=0x38 (but we want 0x28 for data)
    //
    // Actually, sysret behavior:
    // - 64-bit sysret: CS = STAR[63:48] + 16, SS = STAR[63:48] + 8
    // - So if STAR[63:48] = 0x20, then CS = 0x30, SS = 0x28
    //
    // For syscall:
    // - CS = STAR[47:32], SS = STAR[47:32] + 8
    // - So if STAR[47:32] = 0x08, then CS = 0x08, SS = 0x10

    let star_value: u64 = ((CS_USER_32_FOR_STAR as u64) << 48) | ((CS_KERNEL as u64) << 32);
    wrmsr(IA32_STAR, star_value);

    // Configure IA32_LSTAR with the syscall entry point
    wrmsr(IA32_LSTAR, entry_point);

    // Configure IA32_FMASK to mask IF and TF during syscall
    wrmsr(IA32_FMASK, SYSCALL_RFLAGS_MASK);
}

/// User code segment base for STAR[63:48] (used by sysret)
/// sysret adds 16 to get 64-bit code segment, 8 to get data segment
/// If STAR[63:48] = 0x10 (Kernel Data index), then:
/// SS = 0x10 + 8 = 0x18 (User Data)
/// CS = 0x10 + 16 = 0x20 (User Code)
const CS_USER_32_FOR_STAR: u16 = 0x10;

/// Enable syscall instruction
///
/// # Safety
/// Must be called in Ring 0
pub unsafe fn enable_syscall() {
    let efer = rdmsr(IA32_EFER);
    wrmsr(IA32_EFER, efer | EFER_SCE);
}

/// Check if syscall is enabled
pub fn is_syscall_enabled() -> bool {
    unsafe { (rdmsr(IA32_EFER) & EFER_SCE) != 0 }
}

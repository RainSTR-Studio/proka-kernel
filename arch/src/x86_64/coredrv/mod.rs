//! The coredrv handler.
mod drvtype;
use crate::process::DRIVER_PROCESS;
pub use drvtype::*;
use proka_kernel_common::coredrv::Callnum;
use x86_64::structures::idt::InterruptStackFrame;

/// Common interrupt handler
pub extern "x86-interrupt" fn coredrv(_: InterruptStackFrame) {
    // At this time, we shall check up the interrupt
    let call_num: u64;
    let pml4: u64;
    let arg1: u64;
    let arg2: u64;

    unsafe {
        core::arch::asm!(
            "mov {}, cr3",
            "mov r15, 0x100000",
            "mov cr3, r15",
            out(reg) pml4,
            out("rax") call_num,
            out("rdi") arg1,
            out("rsi") arg2,
            out("r15") _,
        );
    }

    // Convert
    let call_num = Callnum::from(call_num);

    // Since we got PML4 address, we can do convert from PML4 to DID.
    let drvproc = &DRIVER_PROCESS.read().process;
    let did = if let Some(id) = drvproc
        .iter()
        .position(|process| process.table_addr == pml4)
    {
        id as u16 // id is always below 16384
    } else {
        unsafe { core::arch::asm!("mov cr3, {}", in(reg) pml4) } // Restore the original table
        return;
    };

    // After getting call_num and args, we shall match each...
    match call_num {
        // Driver type registing call
        Callnum::RegDriverType => driver_type_reg(arg1, arg2, did),

        // Invalid type
        Callnum::Invalid => (),
    }

    unsafe { core::arch::asm!("mov cr3, {}", in(reg) pml4) } // Restore the original table
}

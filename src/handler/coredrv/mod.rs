//! The coredrv handler.
mod drvtype;
use x86_64::structures::idt::InterruptStackFrame;
pub use drvtype::*;

/// The call_num enums of coredrv.
#[repr(u64)]
pub enum Callnum {
    /// Register the driver type.
    RegDriverType = 1,

    // TODO: Add more callnum for coredrv.

    /// Invalid call num.
    Invalid = u64::MAX,
}

impl Callnum {
    pub fn from_u64(num: u64) -> Self {
        match num {
            1 => Self::RegDriverType,
            _ => Self::Invalid,
        }
    }
}

/// Common interrupt handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn coredrv(_: InterruptStackFrame) {
    // At this time, we shall check up the interrupt
    let call_num: u64;
    let arg1: u64;
    let arg2: u64;

    unsafe {
        core::arch::asm!(
            "nop",
            out("rax") call_num,
            out("rdi") arg1,
            out("rsi") arg2,
        );
    }

    let call_num = Callnum::from_u64(call_num);

    // After getting call_num and args, we shall match each...
    match call_num {
        // Driver type registing call
        Callnum::RegDriverType => driver_type_reg(arg1, arg2),

        // Invalid type
        Callnum::Invalid => return,
    }
}


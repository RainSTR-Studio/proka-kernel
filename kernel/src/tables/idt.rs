//! The IDT table

use lazy_static::lazy_static;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        set_general_handler!(&mut idt, general_handler);
        idt
    };
}

pub fn general_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    unsafe {
        core::arch::asm!(
            "movzx r8, {0}",
            in(reg_byte) index,
            options(nomem, nostack),
        );

        if let Some(code) = error_code {
            core::arch::asm!(
                "mov r9, {0}",
                in(reg) code,
                options(nomem, nostack)
            );
        } else {
            core::arch::asm!(
                "mov r9, 0xFFFF",
                options(nomem, nostack)
            );
        }
    }
    loop {}
}

/// Init IDT
pub fn init() {
    IDT.load();
}

//! The IDT table
use crate::println;
use lazy_static::lazy_static;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        set_general_handler!(&mut idt, general_handler, 0..31);
        idt[0x30].set_handler_fn(time_interrupt);
        idt[0xFF].set_handler_fn(spurious);
        idt
    };
}

fn general_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    let errcode = if let Some(code) = error_code { code } else { 0xFFFF };
    println!(
        "[ERROR] CPU Exception! index: {},\n\
        stack: {:#?}, \nerrcode: {}",
        index, stack_frame, errcode
    );
    loop {}
}

extern "x86-interrupt" fn time_interrupt(_: InterruptStackFrame) {
    // Switch to kernel page table first
    unsafe {
        core::arch::asm!(
            "mov rax, 0x100000", // Fixed addr
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    // Invoke tadk switcher
    crate::scheduler::switch_task();

    // Send EOI
    crate::apic::eoi();
}

extern "x86-interrupt" fn spurious(_: InterruptStackFrame) {}

/// Init IDT
pub fn init() {
    IDT.load();
}

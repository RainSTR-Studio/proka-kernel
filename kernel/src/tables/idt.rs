//! The IDT table
use crate::println;
use crate::scheduler::switch_task;
use lazy_static::lazy_static;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        set_general_handler!(&mut idt, general_handler, 0..31);
        idt[0x20].set_handler_fn(switch_task);
        idt[0xF0].set_handler_fn(spurious);
        idt[0xF1].set_handler_fn(error);
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

// Spurious interrupt
extern "x86-interrupt" fn spurious(_: InterruptStackFrame) {}

// Error interrupt
extern "x86-interrupt" fn error(_: InterruptStackFrame) {
    crate::apic::eoi();
}

/// Init IDT
pub fn init() {
    IDT.load();
}

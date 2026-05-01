//! The IDT table
use crate::println;
use crate::scheduler::switch_task;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

#[unsafe(link_section = ".gdata")]
pub static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

#[unsafe(link_section = ".gdata")]
fn general_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    let errcode = if let Some(code) = error_code {
        code
    } else {
        0xFFFF
    };
    println!(
        "[ERROR] CPU Exception! index: {},\n\
        stack: {:#?}, \nerrcode: {}",
        index, stack_frame, errcode
    );
    loop {}
}

// Spurious interrupt
#[unsafe(link_section = ".gdata")]
extern "x86-interrupt" fn spurious(_: InterruptStackFrame) {}

// Error interrupt
#[unsafe(link_section = ".gdata")]
extern "x86-interrupt" fn error(_: InterruptStackFrame) {
    crate::apic::eoi();
}

/// Init IDT
pub fn init() {
    // SAFETY: Update IDT won't destroy data
    unsafe {
        let mut idt = &mut *(&raw mut IDT);
        set_general_handler!(&mut idt, general_handler, 0..31);
        idt[0x20].set_handler_fn(switch_task);
        idt[0xF0].set_handler_fn(spurious);
        idt[0xF1].set_handler_fn(error);
        idt.load();
    }
}

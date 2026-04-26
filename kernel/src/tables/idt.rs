//! The IDT table
use crate::println;
use lazy_static::lazy_static;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        set_general_handler!(&mut idt, general_handler, 0..31);
        idt
    };
}

pub fn general_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    let errcode = if let Some(code) = error_code { code } else { 0xFFFF };
    println!(
        "[ERROR] CPU Exception! index: {},\n\
        stack: {:#?}, \nerrcode: {}",
        index, stack_frame, errcode
    );
    loop {}
}

/// Init IDT
pub fn init() {
    IDT.load();
}

//! The IDT table
use crate::println;
use crate::scheduler::switch_task;
use spin::Lazy;
use x86_64::set_general_handler;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Place IDT in .gdata section, initialize lazily
#[unsafe(link_section = ".gdata")]
pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    set_general_handler!(&mut idt, general_handler);
    idt[0x20].set_handler_fn(apic_calibrator);
    idt[0x30].set_handler_fn(switch_task);
    idt[0xF0].set_handler_fn(spurious);
    idt[0xF1].set_handler_fn(error);
    idt
});

// General CPU exception handler for vector 0~31
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

// The APIC calibrator
extern "x86-interrupt" fn apic_calibrator(_: InterruptStackFrame) {
    use crate::apic::{PIC, COUNT};
    use core::sync::atomic::Ordering;
    
    // Add the count in each interrupts
    COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe { PIC.lock().notify_end_of_interrupt(0x20) }
}

// Spurious interrupt handler for LAPIC
#[unsafe(link_section = ".gdata")]
extern "x86-interrupt" fn spurious(_: InterruptStackFrame) {}

// Common interrupt handler with EOI acknowledge
#[unsafe(link_section = ".gdata")]
extern "x86-interrupt" fn error(_: InterruptStackFrame) {
    crate::apic::eoi();
}

/// Initialize and load IDT
pub fn init() {
    IDT.load();
}

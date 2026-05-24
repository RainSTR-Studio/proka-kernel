//! The IDT table
use crate::handler::*;
use crate::scheduler::switch_task;
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Place IDT in .gdata section, initialize lazily
// All exception handler are in `crate::handler`.
#[unsafe(link_section = ".gdata")]
pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    // New table
    let mut idt = InterruptDescriptorTable::new();
    
    // CPU exception handler
    idt.divide_error.set_handler_fn(divide_error);
    idt.debug.set_handler_fn(debug);
    idt.non_maskable_interrupt.set_handler_fn(nmi);
    idt.overflow.set_handler_fn(overflow);
    idt.bound_range_exceeded.set_handler_fn(bound_range);
    idt.invalid_opcode.set_handler_fn(invalid_opcode);
    idt.device_not_available.set_handler_fn(device_not_available);
    idt.double_fault.set_handler_fn(double_fault);
    idt.invalid_tss.set_handler_fn(invalid_tss);
    idt.segment_not_present.set_handler_fn(segment_not_present);
    idt.stack_segment_fault.set_handler_fn(stack_segment);
    idt.general_protection_fault.set_handler_fn(general_protection);
    idt.page_fault.set_handler_fn(pagefault);
    idt.x87_floating_point.set_handler_fn(x87_floating_point);
    idt.alignment_check.set_handler_fn(alignment_check);
    idt.machine_check.set_handler_fn(machine_check);
    idt.cp_protection_exception.set_handler_fn(control_protection);

    // LAPIC interrupts
    idt[0x20].set_handler_fn(apic_calibrator);
    idt[0x30].set_handler_fn(switch_task);
    idt[0xF0].set_handler_fn(spurious);
    idt[0xF1].set_handler_fn(error);
    idt
});


// The APIC calibrator
extern "x86-interrupt" fn apic_calibrator(_: InterruptStackFrame) {
    use crate::apic::COUNT;
    use core::sync::atomic::Ordering;

    // Add the count in each interrupts
    COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Initialize and load IDT
pub fn init() {
    IDT.load();
}

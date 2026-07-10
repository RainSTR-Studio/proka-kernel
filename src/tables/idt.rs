//! The IDT table
use crate::handler::*;
use crate::scheduler::switch_task;
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Place IDT in .gdata section, initialize lazily
// All exception handler are in `crate::handler`.
pub static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| unsafe {
    // New table
    let mut idt = InterruptDescriptorTable::new();

    // CPU exception handler
    idt.divide_error
        .set_handler_fn(divide_error)
        .set_stack_index(0);
    idt.debug.set_handler_fn(debug).set_stack_index(0);
    idt.non_maskable_interrupt
        .set_handler_fn(nmi)
        .set_stack_index(1);
    idt.overflow.set_handler_fn(overflow).set_stack_index(0);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range)
        .set_stack_index(0);
    idt.invalid_opcode
        .set_handler_fn(invalid_opcode)
        .set_stack_index(0);
    idt.device_not_available
        .set_handler_fn(device_not_available)
        .set_stack_index(0);
    idt.double_fault
        .set_handler_fn(double_fault)
        .set_stack_index(1);
    idt.invalid_tss
        .set_handler_fn(invalid_tss)
        .set_stack_index(0);
    idt.segment_not_present
        .set_handler_fn(segment_not_present)
        .set_stack_index(0);
    idt.stack_segment_fault
        .set_handler_fn(stack_segment)
        .set_stack_index(0);
    idt.general_protection_fault
        .set_handler_fn(general_protection)
        .set_stack_index(0);
    idt.page_fault.set_handler_fn(pagefault).set_stack_index(0);
    idt.x87_floating_point
        .set_handler_fn(x87_floating_point)
        .set_stack_index(0);
    idt.alignment_check
        .set_handler_fn(alignment_check)
        .set_stack_index(0);
    idt.machine_check
        .set_handler_fn(machine_check)
        .set_stack_index(1);
    idt.cp_protection_exception
        .set_handler_fn(control_protection)
        .set_stack_index(0);

    // LAPIC interrupts
    idt[0x20].set_handler_fn(apic_calibrator);
    idt[0x30].set_handler_fn(switch_task).set_stack_index(0);
    idt[0xF0].set_handler_fn(spurious);
    idt[0xF1].set_handler_fn(error);

    // Self-specified interrupts
    idt[0x40].set_handler_fn(coredrv).set_stack_index(0);

    idt
});

/// The empty IDT
pub static IDT_EMPTY: InterruptDescriptorTable = InterruptDescriptorTable::new();

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

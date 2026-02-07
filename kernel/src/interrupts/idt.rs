use crate::interrupts::gdt;
use crate::interrupts::handler;
use crate::interrupts::pic::{PIC_1_OFFSET, PIC_2_OFFSET}; // Import PIC consts and global PICS
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;
// Define the count of PIC interrupt vectors
#[allow(dead_code)]
pub const PICS_EVT_COUNT: u8 = 16; // IRQ0 to IRQ15, total 16 interrupts
pub const SPURIOUS_APIC_VECTOR: u8 = 0xFF; // APIC spurious interrupt vector

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Spurious Interrupt Handler
        idt[SPURIOUS_APIC_VECTOR].set_handler_fn(handler::spurious_interrupt_handler);

        // Non-error-code exception handlers
        idt.divide_error.set_handler_fn(handler::divide_error_handler);
        idt.debug.set_handler_fn(handler::debug_handler);
        idt.non_maskable_interrupt.set_handler_fn(handler::nmi_handler);
        idt.breakpoint.set_handler_fn(handler::breakpoint_handler);
        idt.overflow.set_handler_fn(handler::overflow_handler);
        idt.bound_range_exceeded.set_handler_fn(handler::bound_range_handler);
        idt.invalid_opcode.set_handler_fn(handler::invalid_opcode_handler);
        idt.device_not_available.set_handler_fn(handler::device_not_available_handler);
        idt.x87_floating_point.set_handler_fn(handler::x87_floating_point_handler);
        // Error code exception handlers
        idt.invalid_tss.set_handler_fn(handler::invalid_tss_handler);
        idt.segment_not_present.set_handler_fn(handler::segment_not_present_handler);
        idt.stack_segment_fault.set_handler_fn(handler::stack_segment_handler);
        idt.general_protection_fault.set_handler_fn(handler::general_protection_handler);
        idt.alignment_check.set_handler_fn(handler::alignment_check_handler);
        idt.cp_protection_exception.set_handler_fn(handler::control_protection_handler);
        // Special exception handlers
        idt.page_fault.set_handler_fn(handler::pagefault_handler);
        idt.machine_check.set_handler_fn(handler::machine_check_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(handler::double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        // PIC interrupt handlers
        // IRQ0 - Clock Interrupt, IRQ1 - Keyboard Interrupt
        idt[PIC_1_OFFSET].set_handler_fn(handler::pic_interrupt_handler_0); // IRQ0
        idt[PIC_1_OFFSET + 1].set_handler_fn(handler::pic_interrupt_handler_1); // IRQ1
        idt[PIC_1_OFFSET + 2].set_handler_fn(handler::pic_interrupt_handler_2); // IRQ2
        idt[PIC_1_OFFSET + 3].set_handler_fn(handler::pic_interrupt_handler_3); // IRQ3
        idt[PIC_1_OFFSET + 4].set_handler_fn(handler::pic_interrupt_handler_4); // IRQ4
        idt[PIC_1_OFFSET + 5].set_handler_fn(handler::pic_interrupt_handler_5); // IRQ5
        idt[PIC_1_OFFSET + 6].set_handler_fn(handler::pic_interrupt_handler_6); // IRQ6
        idt[PIC_1_OFFSET + 7].set_handler_fn(handler::pic_interrupt_handler_7); // IRQ7
        idt[PIC_2_OFFSET].set_handler_fn(handler::pic_interrupt_handler_8);   // IRQ8
        idt[PIC_2_OFFSET + 1].set_handler_fn(handler::pic_interrupt_handler_9);   // IRQ9
        idt[PIC_2_OFFSET + 2].set_handler_fn(handler::pic_interrupt_handler_10);  // IRQ10
        idt[PIC_2_OFFSET + 3].set_handler_fn(handler::pic_interrupt_handler_11);  // IRQ11
        idt[PIC_2_OFFSET + 4].set_handler_fn(handler::pic_interrupt_handler_12);  // IRQ12
        idt[PIC_2_OFFSET + 5].set_handler_fn(handler::pic_interrupt_handler_13);  // IRQ13
        idt[PIC_2_OFFSET + 6].set_handler_fn(handler::pic_interrupt_handler_14);  // IRQ14
        idt[PIC_2_OFFSET + 7].set_handler_fn(handler::pic_interrupt_handler_15);  // IRQ15
        idt
    };
}
pub fn init_idt() {
    IDT.load();
}

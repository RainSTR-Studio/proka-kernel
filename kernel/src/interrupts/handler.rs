#[allow(unused)]
use crate::interrupts::apic;
use crate::interrupts::apic::registry::{IrqContext, IRQ_REGISTRY};
use crate::interrupts::idt::IRQ_BASE;
use crate::panic::{ExceptionInfo, EXCEPTION_INFO};
use crate::serial_println;
use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
    VirtAddr,
};

macro_rules! exception_handler {
    ($name:ident, $msg:expr) => {
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            {
                let mut info = EXCEPTION_INFO.write();
                *info = Some(ExceptionInfo {
                    name: $msg,
                    rip: stack_frame.instruction_pointer.as_u64(),
                    cs: stack_frame.code_segment.0 as u64,
                    rflags: stack_frame.cpu_flags.bits(),
                    rsp: stack_frame.stack_pointer.as_u64(),
                    ss: stack_frame.stack_segment.0 as u64,
                    error_code: None,
                });
            }
            panic!("EXCEPTION: {}", $msg);
        }
    };
}

macro_rules! exception_handler_with_error_code {
    ($name:ident, $msg:expr) => {
        pub extern "x86-interrupt" fn $name(
            stack_frame: InterruptStackFrame,
            error_code: u64, // Uses u64 as error code
        ) {
            {
                let mut info = EXCEPTION_INFO.write();
                *info = Some(ExceptionInfo {
                    name: $msg,
                    rip: stack_frame.instruction_pointer.as_u64(),
                    cs: stack_frame.code_segment.0 as u64,
                    rflags: stack_frame.cpu_flags.bits(),
                    rsp: stack_frame.stack_pointer.as_u64(),
                    ss: stack_frame.stack_segment.0 as u64,
                    error_code: Some(error_code),
                });
            }
            panic!("EXCEPTION: {} [ERR: {:#x}]", $msg, error_code);
        }
    };
}

// Non-error-code exception -------------------------------------------------
exception_handler!(divide_error_handler, "DIVIDE ERROR");
exception_handler!(debug_handler, "DEBUG");
exception_handler!(nmi_handler, "NON-MASKABLE INTERRUPT");
exception_handler!(overflow_handler, "OVERFLOW");
exception_handler!(bound_range_handler, "BOUND RANGE EXCEEDED");
exception_handler!(invalid_opcode_handler, "INVALID OPCODE");
exception_handler!(device_not_available_handler, "DEVICE NOT AVAILABLE");
exception_handler!(x87_floating_point_handler, "x87 FLOATING POINT ERROR");

// Error-code exception -------------------------------------------------
exception_handler_with_error_code!(invalid_tss_handler, "INVALID TSS");
exception_handler_with_error_code!(segment_not_present_handler, "SEGMENT NOT PRESENT");
exception_handler_with_error_code!(stack_segment_handler, "STACK-SEGMENT FAULT");
exception_handler_with_error_code!(general_protection_handler, "GENERAL PROTECTION FAULT");
exception_handler_with_error_code!(alignment_check_handler, "ALIGNMENT CHECK");
exception_handler_with_error_code!(control_protection_handler, "CONTROL PROTECTION EXCEPTION");

// Special handler -------------------------------------------------
pub extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Fake interrupt doesn't need to send EIO
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    {
        let mut info = EXCEPTION_INFO.write();
        *info = Some(ExceptionInfo {
            name: "DOUBLE FAULT",
            rip: stack_frame.instruction_pointer.as_u64(),
            cs: stack_frame.code_segment.0 as u64,
            rflags: stack_frame.cpu_flags.bits(),
            rsp: stack_frame.stack_pointer.as_u64(),
            ss: stack_frame.stack_segment.0 as u64,
            error_code: Some(error_code),
        });
    }
    panic!("CRITICAL: DOUBLE FAULT [ERR: {:#x}]", error_code);
}

pub extern "x86-interrupt" fn pagefault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_address = match Cr2::read() {
        Ok(addr) => addr,
        Err(_) => VirtAddr::zero(),
    };

    {
        let mut ms_lock = crate::memory::paging::vmm::KERNEL_MEMORY_SET.lock();
        if let Some(ms) = ms_lock.as_mut() {
            if ms.handle_page_fault(fault_address).is_ok() {
                return;
            }
        }
    }

    {
        let mut info = EXCEPTION_INFO.write();
        *info = Some(ExceptionInfo {
            name: "PAGE FAULT",
            rip: stack_frame.instruction_pointer.as_u64(),
            cs: stack_frame.code_segment.0 as u64,
            rflags: stack_frame.cpu_flags.bits(),
            rsp: stack_frame.stack_pointer.as_u64(),
            ss: stack_frame.stack_segment.0 as u64,
            error_code: Some(error_code.bits()),
        });
    }

    panic!(
        "EXCEPTION: PAGE FAULT at {:#x}\nCause: {:?}",
        fault_address, error_code
    );
}

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    {
        let mut info = EXCEPTION_INFO.write();
        *info = Some(ExceptionInfo {
            name: "MACHINE CHECK",
            rip: stack_frame.instruction_pointer.as_u64(),
            cs: stack_frame.code_segment.0 as u64,
            rflags: stack_frame.cpu_flags.bits(),
            rsp: stack_frame.stack_pointer.as_u64(),
            ss: stack_frame.stack_segment.0 as u64,
            error_code: None,
        });
    }
    panic!("CRITICAL: MACHINE CHECK");
}

pub extern "x86-interrupt" fn timer_interrupt_handler(stack_frame: InterruptStackFrame) {
    let context = IrqContext {
        vector: crate::interrupts::apic::TIMER_VECTOR,
        irq_number: None, // APIC timer is not a standard ISA IRQ
        stack_frame: &stack_frame,
        error_code: None,
    };

    if let Some(registry) = IRQ_REGISTRY.try_read() {
        registry.handle(context);
    }

    apic::end_of_interrupt();
}

macro_rules! ioapic_interrupt_handler {
    ($name:ident, $irq_number:expr) => {
        #[allow(unused_variables)]
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            let vector = IRQ_BASE + $irq_number;
            let context = IrqContext {
                vector,
                irq_number: Some($irq_number),
                stack_frame: &stack_frame,
                error_code: None,
            };

            let mut handled = false;
            if let Some(registry) = IRQ_REGISTRY.try_read() {
                if let crate::interrupts::apic::registry::IrqResult::Handled =
                    registry.handle(context)
                {
                    handled = true;
                }
            }

            if !handled {
                if $irq_number == 1 {
                    let mut port = x86_64::instructions::port::Port::<u8>::new(0x60);
                    let scancode = unsafe { port.read() };
                    crate::drivers::input::ps2::keyboard::KEYBOARD.handle_scancode(scancode);
                } else {
                    serial_println!("IRQ {} received!", $irq_number);
                }
            }

            apic::end_of_interrupt();
        }
    };
}
// Define interrupt handlers for all 16 IRQs
ioapic_interrupt_handler!(ioapic_interrupt_handler_0, 0); // Timer Interrupt
ioapic_interrupt_handler!(ioapic_interrupt_handler_1, 1); // Keyboard Interrupt
ioapic_interrupt_handler!(ioapic_interrupt_handler_2, 2); // Cascade to PIC2
ioapic_interrupt_handler!(ioapic_interrupt_handler_3, 3); // Serial COM2
ioapic_interrupt_handler!(ioapic_interrupt_handler_4, 4); // Serial COM1
ioapic_interrupt_handler!(ioapic_interrupt_handler_5, 5); // Parallel Port LPT2 / Sound Card
ioapic_interrupt_handler!(ioapic_interrupt_handler_6, 6); // Floppy Disk Controller
ioapic_interrupt_handler!(ioapic_interrupt_handler_7, 7); // Parallel Port LPT1 / Fake Interrupt
ioapic_interrupt_handler!(ioapic_interrupt_handler_8, 8); // RTC Real Time Clock
ioapic_interrupt_handler!(ioapic_interrupt_handler_9, 9); // Redirect IRQ2
ioapic_interrupt_handler!(ioapic_interrupt_handler_10, 10); // Freed / SCSI / Netcard
ioapic_interrupt_handler!(ioapic_interrupt_handler_11, 11); // Freed / SCSI / Netcard
ioapic_interrupt_handler!(ioapic_interrupt_handler_12, 12); // PS/2 mouse
ioapic_interrupt_handler!(ioapic_interrupt_handler_13, 13); // FPU / MPU
ioapic_interrupt_handler!(ioapic_interrupt_handler_14, 14); // Primary IDE
ioapic_interrupt_handler!(ioapic_interrupt_handler_15, 15); // Secondary IDE

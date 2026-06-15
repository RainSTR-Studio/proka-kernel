//! Exception handler.
use crate::println;
#[allow(unused)]
use core::arch::asm;
use x86_64::{
    VirtAddr,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

macro_rules! exception {
    ($name:ident, $msg:expr) => {
        #[unsafe(link_section = ".gdata")]
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            // Switch to kernel page table
            unsafe {
                asm!(
                    "mov rax, 0x100000",
                    "mov cr3, rax",
                    options(nomem, nostack, preserves_flags),
                )
            }

            // Do next...
            println!(
                "\x1b[31m[ERROR] CPU EXCEPTION: {}\n{:#?}\x1b[0m",
                $msg, stack_frame
            );
            hlt_loop() // TODO: Replace it to recovor logic
        }
    };
}

macro_rules! exception_with_error_code {
    ($name:ident, $msg:expr) => {
        #[unsafe(link_section = ".gdata")]
        pub extern "x86-interrupt" fn $name(
            stack_frame: InterruptStackFrame,
            error_code: u64, // Uses u64 as error code
        ) {
            // Switch to kernel page table
            unsafe {
                asm!(
                    "mov rax, 0x100000",
                    "mov cr3, rax",
                    options(nomem, nostack, preserves_flags)
                )
            }

            println!(
                "\x1b[31m[ERROR] CPU EXCEPTION! {} [ERR: {:#x}]\n{:#?}\x1b[0m",
                $msg, error_code, stack_frame
            );
            hlt_loop()
        }
    };
}

// Non-error-code exception -------------------------------------------------
exception!(divide_error, "DIVIDE ERROR");
exception!(debug, "DEBUG");
exception!(nmi, "NON-MASKABLE INTERRUPT");
exception!(overflow, "OVERFLOW");
exception!(bound_range, "BOUND RANGE EXCEEDED");
exception!(invalid_opcode, "INVALID OPCODE");
exception!(device_not_available, "DEVICE NOT AVAILABLE");
exception!(x87_floating_point, "x87 FLOATING POINT ERROR");

// Error-code exception -------------------------------------------------
exception_with_error_code!(invalid_tss, "INVALID TSS");
exception_with_error_code!(segment_not_present, "SEGMENT NOT PRESENT");
exception_with_error_code!(stack_segment, "STACK-SEGMENT FAULT");
exception_with_error_code!(general_protection, "GENERAL PROTECTION FAULT");
exception_with_error_code!(alignment_check, "ALIGNMENT CHECK");
exception_with_error_code!(control_protection, "CONTROL PROTECTION EXCEPTION");

// Special handler -------------------------------------------------
// #DF handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn double_fault(stack_frame: InterruptStackFrame, error_code: u64) -> ! {
    // Switch to kernel table
    unsafe {
        asm!(
            "mov rax, 0x100000",
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    // Must mark as never return
    println!(
        "[ERROR] CRITICAL: DOUBLE FAULT [ERR: {:#x}]\n{:#?}",
        error_code, stack_frame
    );
    panic!("SYSTEM PANIC"); // Stop system safely
}

// #PF handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn pagefault(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    unsafe {
        asm!(
            "mov rax, 0x100000",
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    let fault_address = match Cr2::read() {
        Ok(addr) => addr,
        Err(_) => VirtAddr::zero(),
    };

    println!(
        "\x1b[31m[ERROR] EXCEPTION: PAGE FAULT at {:#x}\nError Code: {:?}\nFrame: {:#?}\x1b[0m",
        fault_address, error_code, stack_frame
    );
    // TODO: Exception recovery logic
    hlt_loop()
}

// Breakpoint handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn breakpoint(stack_frame: InterruptStackFrame) {
    unsafe {
        asm!(
            "mov rax, 0x100000",
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

// Machine check handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn machine_check(stack_frame: InterruptStackFrame) -> ! {
    unsafe {
        asm!(
            "mov rax, 0x100000",
            "mov cr3, rax",
            options(nomem, nostack, preserves_flags)
        )
    }

    println!("CRITICAL: MACHINE CHECK\n{:#?}", stack_frame);
    panic!("SYSTEM HALT: MACHINE CHECK");
}

#[inline(always)]
fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

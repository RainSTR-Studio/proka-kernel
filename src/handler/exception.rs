//! Exception handler.
//!
//! Originally by moyanj <me@moyanjdc.top>
use crate::memory::IdentityPageTableMapper;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use crate::println;
use crate::process::{DRIVER_PROCESS, NORMAL_PROCESS, ProcType};
use crate::scheduler::{CURRENT_ID, IS_DRIVER};
use core::arch::asm;
use core::sync::atomic::Ordering;
use x86_64::structures::paging::{
    FrameAllocator, MappedPageTable, Mapper, Page, PageTable, PageTableFlags, Size4KiB,
};
use x86_64::{
    VirtAddr,
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

macro_rules! exception {
    ($name:ident, $msg:expr) => {
        pub extern "x86-interrupt" fn $name(stack_frame: InterruptStackFrame) {
            // Switch to kernel page table
            unsafe {
                asm!(
                    "mov rax, 0x100000",
                    "mov cr3, rax",
                    options(nomem, nostack, preserves_flags),
                )
            }

            // Query the current process ID.
            let pid = CURRENT_ID.load(Ordering::Relaxed);
            let proc_type = if IS_DRIVER.load(Ordering::Relaxed) {
                "DRIVER"
            } else {
                "USER"
            };

            // Do next...
            println!(
                "\x1b[31m[ERROR] CPU EXCEPTION: {} [ID: {}] [PROCESS TYPE: {}]\n{:#?}\x1b[0m",
                $msg, pid, proc_type, stack_frame
            );
            hlt_loop() // TODO: Replace it to recovor logic
        }
    };
}

macro_rules! exception_with_error_code {
    ($name:ident, $msg:expr) => {
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

            // Query the current process ID.
            let pid = CURRENT_ID.load(Ordering::Relaxed);
            let proc_type = if IS_DRIVER.load(Ordering::Relaxed) {
                "DRIVER"
            } else {
                "USER"
            };

            // Do next...
            println!(
                "\x1b[31m[ERROR] CPU EXCEPTION: {}, error code: {} [ID: {}] [PROCESS TYPE: {}]\n{:#?}\x1b[0m",
                $msg, error_code, pid, proc_type, stack_frame
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
pub extern "x86-interrupt" fn pagefault(
    _stack_frame: InterruptStackFrame,
    _error_code: PageFaultErrorCode,
) {
    let pml4: u64;
    unsafe {
        asm!(
            "mov rdx, cr3",
            "mov rax, 0x100000",
            "mov cr3, rax",
            out("rdx") pml4,
            options(nomem, nostack, preserves_flags)
        )
    }

    let fault_address = match Cr2::read() {
        Ok(addr) => addr,
        Err(_) => VirtAddr::zero(),
    };

    // Time to query the process...
    if IS_DRIVER.load(Ordering::Relaxed) {
        let binding = DRIVER_PROCESS.read();
        let (index, process) = binding
            .process
            .iter()
            .enumerate()
            .find(|item| pml4 == item.1.table_addr)
            .expect("Process (driver) in the page table is mismatched...");

        // Check: is the #PF place in stack range?
        if (process.stack_bottom..0x7ffffffff000).contains(&fault_address.as_u64()) {
            // We should map the missing place...
            let mut mapper = unsafe {
                let table_wrapped = &mut *(pml4 as *mut PageTable);
                MappedPageTable::new(table_wrapped, IdentityPageTableMapper)
            };

            // Map 1 4KiB page...
            let page = Page::<Size4KiB>::containing_address(fault_address);
            let Some(frame) = FRAME_ALLOCATOR.lock().allocate_frame() else {
                crate::process::remove(ProcType::Driver, index).unwrap();
                hlt_loop()
            };
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            unsafe {
                mapper
                    .map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                    .expect("Failed to map stack in #PF")
                    .ignore()
            }
        }

        crate::process::remove(ProcType::Driver, index).unwrap();
        hlt_loop()
    } else {
        let binding = NORMAL_PROCESS.read();
        let (index, process) = binding
            .process
            .iter()
            .enumerate()
            .find(|item| pml4 == item.1.table_addr || pml4 == item.1.current_table)
            .expect("Process (normal) om this page table is mismatched...");

        // Check: is #PF in stack range
        if (process.stack_bottom..0x7ffffffff000).contains(&fault_address.as_u64()) {
            // Create mapper
            let mut mapper = unsafe {
                let table_wrapped = &mut *(pml4 as *mut PageTable);
                MappedPageTable::new(table_wrapped, IdentityPageTableMapper)
            };

            // Map 1 4KiB page only...
            let page = Page::<Size4KiB>::containing_address(fault_address);
            let Some(frame) = FRAME_ALLOCATOR.lock().allocate_frame() else {
                crate::process::remove(ProcType::Normal, index).unwrap();
                hlt_loop()
            };
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            unsafe {
                mapper
                    .map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                    .expect("Failed to map stack in #PF")
                    .ignore()
            }
        }

        crate::process::remove(ProcType::Normal, index).unwrap();
        hlt_loop()
    }
}

// Breakpoint handler

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

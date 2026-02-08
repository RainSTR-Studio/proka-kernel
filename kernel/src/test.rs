//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the test trait and runner.
//! Code example from: Writing an OS in Rust (blog)

use crate::{serial_print, serial_println};
use core::arch::asm;
use spin::Mutex;

/// The Jump Buffer to save the CPU state.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct JumpBuffer {
    rbx: u64,
    rsp: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
}

static TEST_JMP_BUF: Mutex<Option<JumpBuffer>> = Mutex::new(None);
static FAIL_COUNT: Mutex<usize> = Mutex::new(0);

/// Save the current context into the jump buffer.
/// Returns 0 when saving, and 1 when returning from long_jmp.
///
/// # Safety
/// This function is unsafe because it manipulates CPU registers directly
/// and relies on the caller to manage the jump buffer correctly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_jmp() -> u64 {
    let mut jmp_buf = JumpBuffer::default();
    let res: u64;

    asm!(
        // Save the current context into the jump buffer.
        "mov [rcx + 0], rbx",
        "mov [rcx + 8], rsp",
        "mov [rcx + 16], rbp",
        "mov [rcx + 24], r12",
        "mov [rcx + 32], r13",
        "mov [rcx + 40], r14",
        "mov [rcx + 48], r15",
        // Save the return address into the jump buffer.
        "lea rdx, [rip + 2f]",
        "mov [rcx + 56], rdx",
        // First call returns 0
        "mov rax, 0",
        "jmp 3f",
        // longjmp return point
        "2:",
        // longjmp returns 1
        "mov rax, 1",
        "3:",
        in("rcx") &mut jmp_buf,
        out("rax") res,
        out("rdx") _,
        clobber_abi("sysv64")
    );

    if res == 0 {
        *TEST_JMP_BUF.lock() = Some(jmp_buf);
    }
    res
}

/// Restore the context and jump back to the set_jmp location.
pub fn long_jmp() -> ! {
    let jmp_buf = TEST_JMP_BUF.lock().expect("No jump buffer set!");

    // Add error count before recover register
    // So that the counter won't be overwrite
    let mut fail_count = FAIL_COUNT.lock();
    *fail_count += 1;
    drop(fail_count); // Free lock

    unsafe {
        asm!(
            // Recovor register status
            "mov rbx, [rcx + 0]",
            "mov rsp, [rcx + 8]",
            "mov rbp, [rcx + 16]",
            "mov r12, [rcx + 24]",
            "mov r13, [rcx + 32]",
            "mov r14, [rcx + 40]",
            "mov r15, [rcx + 48]",
            // Jump back to the saved address
            "jmp [rcx + 56]",
            in("rcx") &jmp_buf,
            options(noreturn)
        );
    }
}

/// The trait that assign the function is testable.
pub trait Testable {
    /// The things will run
    fn run(&self) -> ();
}

// This is the default implementation of this trait
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("Testing {}... ", core::any::type_name::<T>());

        // Use setjmp/longjmp to test the function
        unsafe {
            if set_jmp() == 0 {
                // First call run the test function
                self();
                serial_println!("[OK]");
            } else {
                // longjmp returns, test failed
                serial_println!("[FAILED]");
            }
        }
    }
}

/// This is the test runner, which will run if the test begins.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    let final_fail_count = *FAIL_COUNT.lock();
    serial_println!("Total failures: {}", final_fail_count);

    if final_fail_count > 0 {
        serial_println!("\n[DONE] FAILED: {} tests failed", final_fail_count);
        exit_qemu(QemuExitCode::Failed);
    } else {
        serial_println!("\n[DONE] SUCCESS: All tests passed!");
        exit_qemu(QemuExitCode::Success);
    }
}

/// This is the QEMU exit code
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// The function to quit the QEMU
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

// The kernel entry, which will start up the test
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    crate::interrupts::gdt::init(); // Initialize GDT
    crate::interrupts::idt::init_idt(); // Initialize IDT
    crate::interrupts::pic::init(); // Initialize PI
    crate::memory::init(); // Initialize memory management
    crate::drivers::init_devices(); // Initialize devices
    crate::libs::time::init(); // Init time system
    crate::libs::logger::init_logger(); // Init log system
    crate::libs::initrd::load_initrd(); // Load initrd
    x86_64::instructions::interrupts::enable(); // Enable interrupts
    crate::test_main();
    loop {
        x86_64::instructions::hlt();
    }
}

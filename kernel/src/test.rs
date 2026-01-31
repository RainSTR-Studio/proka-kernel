//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the test trait and runner.

use crate::{serial_print, serial_println};

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
        serial_print!("Testing {}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

/// This is the test runner, which will run if the test begins.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
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
    crate::memory::init(); // Initialize memory management
    crate::drivers::init_devices(); // Initialize devices
    crate::libs::time::init(); // Init time system
    crate::libs::logger::init_logger(); // Init log system
    crate::libs::initrd::load_initrd(); // Load initrd
    crate::interrupts::gdt::init(); // Initialize GDT
    crate::interrupts::idt::init_idt(); // Initialize IDT
    crate::interrupts::pic::init(); // Initialize PI
    x86_64::instructions::interrupts::enable(); // Enable interrupts
    crate::test_main();
    loop {}
}

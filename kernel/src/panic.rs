//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the panic handler with tests and normal.

use crate::serial_println;
use core::panic::PanicInfo;

// This is the default panic handler
#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    serial_println!("{}", info);
    loop {}
}

// And this is for test
#[cfg(test)]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    // Call the default test panic handler
    panic_for_test(info)
}

// This is the panic handler for all testing function
#[cfg(test)]
pub fn panic_for_test(info: &PanicInfo) -> ! {
    serial_println!("[FAILED]");
    serial_println!("Caused by:\n\t{}", info);
    crate::test::long_jmp();
}

//! The panic handler.
use crate::println;
use core::panic::PanicInfo;

/// The panic handler.
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("\x1b[31m[PANIC] {}\x1b[0m", info);
    loop {}
}

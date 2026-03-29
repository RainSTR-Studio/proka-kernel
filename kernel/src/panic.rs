//! The panic handler.

use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_: &PanicInfo) -> ! {
    loop {}
}

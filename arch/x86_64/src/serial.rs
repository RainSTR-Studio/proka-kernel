//! The serial output module (x86_64 IO port based).
use spin::{LazyLock, Mutex};
use uart_16550::SerialPort;

pub static SERIAL1: LazyLock<Mutex<SerialPort>> = LazyLock::new(|| {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    Mutex::new(serial_port)
});

/* The functions and macros in debug mode */
#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

/// Register the serial print hook into the architecture-independent
/// output layer, so the public `println!` macro can reach the serial port.
pub fn register_hook() {
    proka_common::output::SERIAL_PRINT.call_once(|| _print);
}

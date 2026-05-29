//! The serial output module.
use spin::{Lazy, Mutex};
use uart_16550::SerialPort;

pub static SERIAL1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();
    Mutex::new(serial_port)
});

/* The functions and macros in debug mode */
#[doc(hidden)]
#[cfg(debug_assertions)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::output::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
#[cfg(debug_assertions)]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

/* The macros and function not in debug mode (empty) */
#[doc(hidden)]
#[cfg(not(debug_assertions))]
pub fn _print(_args: ::core::fmt::Arguments) {}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! serial_print {
    ($($arg:tt)*) => {};
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! serial_println {
    () => {};
    ($fmt:expr) => {};
    ($fmt:expr, $($arg:tt)*) => {};
}

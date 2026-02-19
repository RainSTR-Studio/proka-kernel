extern crate alloc;
use crate::config::SERIAL_LOG_PORT;
use crate::drivers::DEVICE_MANAGER;
use uart_16550::SerialPort;

pub fn serial_fallback(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    let mut serial_port = unsafe { SerialPort::new(SERIAL_LOG_PORT as u16) };
    serial_port.init();
    serial_port
        .write_fmt(args)
        .expect("Printing to serial failed");
}

/* The functions and macros in debug mode */
#[doc(hidden)]
#[cfg(debug_assertions)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;

    x86_64::instructions::interrupts::without_interrupts(|| {
        // Get device manager's lock
        let device_manager = DEVICE_MANAGER.read();

        // Try to get the device numbered (1,0)
        match device_manager.get_device_by_major_minor(1, 0) {
            Some(device) => {
                // Try to convert the device to a character device
                if let Some(char_device_arc) = device.as_char_device() {
                    let mut buffer = alloc::string::String::new();
                    buffer.write_fmt(args).expect("Failed to format string");

                    char_device_arc
                        .write(buffer.as_bytes())
                        .expect("Printing to serial failed");
                } else {
                    serial_fallback(args);
                }
            }
            None => {
                // Device (1, 0) not found
                serial_fallback(args);
            }
        }
    });
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
pub fn _print(args: ::core::fmt::Arguments) {}

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

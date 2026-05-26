use crate::output::console::_print as console_print;
use crate::output::serial::_print as serial_print;

/// Double println macro
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            // For release mode, serial is not needed
            // Instead, it will slow down the print speed.
            #[cfg(debug_assertions)]
            $crate::output::dual::_dual_print_serial(format_args!($($arg)*));

            // This will always print to console
            $crate::output::dual::_dual_print_console(format_args!($($arg)*));
        }
    };
}

/// Double println macro, but can switch line.
#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*))
    };
}

// Inner function: print to console
#[doc(hidden)]
pub fn _dual_print_console(args: core::fmt::Arguments) {
    console_print(args);
}

// Inner function: print to serial port
#[doc(hidden)]
#[cfg(debug_assertions)]
pub fn _dual_print_serial(args: core::fmt::Arguments) {
    serial_print(args);
}

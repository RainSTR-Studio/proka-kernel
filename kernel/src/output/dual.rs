#[cfg(ENABLE_GRAPHICS)]
use crate::output::console::_print as console_print;
use crate::output::serial::_print as serial_print;

/// Double println macro
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
            $crate::output::dual::_dual_print_serial(format_args!($($arg)*));
            // Always print to console
            $crate::output::dual::_dual_print_console(format_args!($($arg)*))
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
#[allow(unused_variables)]
pub fn _dual_print_console(args: core::fmt::Arguments) {
    #[cfg(ENABLE_GRAPHICS)]
    console_print(args);
}

// Inner function: print to serial port
#[doc(hidden)]
pub fn _dual_print_serial(args: core::fmt::Arguments) {
    serial_print(args);
}

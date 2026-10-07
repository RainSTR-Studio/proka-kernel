//! The output module.
#[cfg(feature = "output_fb")]
pub mod color;
#[cfg(feature = "output_fb")]
pub mod console;
#[cfg(feature = "output_fb")]
pub mod font8x16;

#[cfg(feature = "output_fb")]
use crate::output::console::_print as console_print;

/// Serial output hook.
///
/// The actual serial driver is architecture-specific and lives in
/// `proka-kernel-x86_64`. The kernel registers its `_print` function
/// here during boot, so the public `println!` macro can stay
/// architecture-independent.
#[cfg(feature = "output_serial")]
pub static SERIAL_PRINT: spin::Once<fn(core::fmt::Arguments)> = spin::Once::new();

/// Double println macro
#[macro_export]
macro_rules! print {
    () => {};
    ($($arg:tt)*) => {
        {
            #[cfg(feature = "output_serial")]
            $crate::output::_dual_print_serial(format_args!($($arg)*));

            #[cfg(feature = "output_fb")]
            $crate::output::_dual_print_console(format_args!($($arg)*));
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
#[cfg(feature = "output_fb")]
pub fn _dual_print_console(args: core::fmt::Arguments) {
    console_print(args);
}

// Inner function: print to serial port
#[doc(hidden)]
#[cfg(feature = "output_serial")]
pub fn _dual_print_serial(args: core::fmt::Arguments) {
    if let Some(f) = SERIAL_PRINT.get() {
        f(args)
    }
}

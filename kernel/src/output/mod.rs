pub mod console;
#[cfg(ENABLE_BITFONT_CONSOLE)]
pub use console::console_bitfont;
pub mod font8x16;
#[cfg(ENABLE_TTF_CONSOLE)]
pub use console::console_ttf;

pub mod dual;
pub mod serial;

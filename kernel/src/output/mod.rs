#[cfg(ENABLE_GRAPHICS)]
pub mod console;
#[cfg(ENABLE_BITFONT_CONSOLE)]
pub use console::console_bitfont;
#[cfg(ENABLE_GRAPHICS)]
pub mod font8x16;
#[cfg(ENABLE_TTF_CONSOLE)]
pub use console::console_ttf;

pub mod dual;
pub mod serial;

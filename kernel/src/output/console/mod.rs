use core::fmt::{self, Write};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::graphics::Color;

pub mod console_bitfont;
pub mod console_ttf;

pub use console_bitfont::BitfontConsole;
pub use console_ttf::TtfConsole;

/// General [`Console`] trait, which defined generic APIs.
pub trait Console: Write {
    /// Clean screen
    fn clear(&mut self);

    /// Set up foreground color
    fn set_fg_color(&mut self, color: Color);

    /// Set up background color
    fn set_bg_color(&mut self, color: Color);

    /// Get current foreground color
    fn get_fg_color(&self) -> Color;

    /// Get current background color
    fn get_bg_color(&self) -> Color;

    /// Print single char
    fn put_char(&mut self, ch: char);

    /// Move cursor up
    fn cursor_up(&mut self, lines: u32);

    /// Move cursor down
    fn cursor_down(&mut self, lines: u32);

    /// Move cursor left
    fn cursor_left(&mut self, cols: u32);

    /// Move cursor right
    fn cursor_right(&mut self, cols: u32);

    /// Set up cursor position
    fn set_cursor_pos(&mut self, x: u32, y: u32);

    /// Get current cursor posision
    fn get_cursor_pos(&self) -> (u32, u32);

    /// Hide cursor
    fn cursor_hide(&mut self);

    /// Show cursor
    fn cursor_show(&mut self);
}

pub type ConsoleImpl<'a> = alloc::boxed::Box<dyn Console + Send + 'a>;

lazy_static! {
    pub static ref CONSOLE: Mutex<ConsoleImpl<'static>> = {
        let console_type = crate::config::DEFAULT_CONSOLE_TYPE;
        if console_type == "ttf" {
            return Mutex::new(alloc::boxed::Box::new(TtfConsole::init()));
        } else if console_type == "bitfont" {
            return Mutex::new(alloc::boxed::Box::new(BitfontConsole::init()));
        } else {
            return Mutex::new(alloc::boxed::Box::new(BitfontConsole::init()));
        }
    };
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    CONSOLE
        .lock()
        .write_fmt(args)
        .expect("Failed to write to console");
}

pub enum ConsoleType {
    Bitfont,
    Ttf,
}

/// Set up the current using console
pub fn select_console(t: ConsoleType) {
    let mut console = CONSOLE.lock();

    // Must do clear before switching console, in order to 
    // avoid 2 console displaying problem.
    console.clear();
    *console = match t {
        ConsoleType::Bitfont => alloc::boxed::Box::new(BitfontConsole::init()),
        ConsoleType::Ttf => alloc::boxed::Box::new(TtfConsole::init()),
    };
}

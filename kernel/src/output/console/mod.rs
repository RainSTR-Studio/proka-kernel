use core::fmt::{self, Write};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::graphics::Color;

#[cfg(ENABLE_BITFONT_CONSOLE)]
pub mod console_bitfont;
#[cfg(ENABLE_TTF_CONSOLE)]
pub mod console_ttf;

#[cfg(ENABLE_BITFONT_CONSOLE)]
pub use console_bitfont::BitfontConsole;
#[cfg(ENABLE_TTF_CONSOLE)]
pub use console_ttf::TtfConsole;

/// 统一的 Console trait，定义所有 console 实现的通用接口
pub trait Console: Write {
    /// 清除屏幕
    fn clear(&mut self);

    /// 设置前景色
    fn set_fg_color(&mut self, color: Color);

    /// 设置背景色
    fn set_bg_color(&mut self, color: Color);

    /// 获取当前前景色
    fn get_fg_color(&self) -> Color;

    /// 获取当前背景色
    fn get_bg_color(&self) -> Color;

    /// 打印单个字符
    fn put_char(&mut self, ch: char);

    /// 光标上移
    fn cursor_up(&mut self, lines: u32);

    /// 光标下移
    fn cursor_down(&mut self, lines: u32);

    /// 光标左移
    fn cursor_left(&mut self, cols: u32);

    /// 光标右移
    fn cursor_right(&mut self, cols: u32);

    /// 设置光标位置
    fn set_cursor_pos(&mut self, x: u32, y: u32);

    /// 获取光标位置
    fn get_cursor_pos(&self) -> (u32, u32);

    /// 隐藏光标
    fn cursor_hide(&mut self);

    /// 显示光标
    fn cursor_show(&mut self);
}

pub type ConsoleImpl<'a> = alloc::boxed::Box<dyn Console + Send + 'a>;

lazy_static! {
    pub static ref CONSOLE: Mutex<ConsoleImpl<'static>> = {
        let console_type = crate::config::DEFAULT_CONSOLE_TYPE;
        #[cfg(ENABLE_TTF_CONSOLE)]
        if console_type == "ttf" {
            return Mutex::new(alloc::boxed::Box::new(TtfConsole::init()));
        }
        #[cfg(ENABLE_BITFONT_CONSOLE)]
        if console_type == "bitfont" {
            return Mutex::new(alloc::boxed::Box::new(BitfontConsole::init()));
        }

        // Fallback
        #[cfg(ENABLE_BITFONT_CONSOLE)]
        return Mutex::new(alloc::boxed::Box::new(BitfontConsole::init()));
        #[cfg(all(not(ENABLE_BITFONT_CONSOLE), ENABLE_TTF_CONSOLE))]
        return Mutex::new(alloc::boxed::Box::new(TtfConsole::init()));
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
    #[cfg(ENABLE_BITFONT_CONSOLE)]
    Bitfont,
    #[cfg(ENABLE_TTF_CONSOLE)]
    Ttf,
}

pub fn select_console(t: ConsoleType) {
    let mut console = CONSOLE.lock();
    *console = match t {
        #[cfg(ENABLE_BITFONT_CONSOLE)]
        ConsoleType::Bitfont => alloc::boxed::Box::new(BitfontConsole::init()),
        #[cfg(ENABLE_TTF_CONSOLE)]
        ConsoleType::Ttf => alloc::boxed::Box::new(TtfConsole::init()),
    };
}

#[cfg(not(any(ENABLE_BITFONT_CONSOLE, ENABLE_TTF_CONSOLE)))]
compile_error!("At least one console implementation must be enabled");

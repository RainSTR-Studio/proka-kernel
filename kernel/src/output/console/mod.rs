use core::fmt::{self, Write};
use lazy_static::lazy_static;
use spin::Mutex;

#[cfg(ENABLE_BITFONT_CONSOLE)]
pub mod console_bitfont;
#[cfg(ENABLE_TTF_CONSOLE)]
pub mod console_ttf;

#[cfg(ENABLE_BITFONT_CONSOLE)]
pub use console_bitfont::BitfontConsole;
#[cfg(ENABLE_TTF_CONSOLE)]
pub use console_ttf::TtfConsole;

pub enum ConsoleImpl<'a> {
    #[cfg(ENABLE_BITFONT_CONSOLE)]
    Bitfont(BitfontConsole, core::marker::PhantomData<&'a ()>),
    #[cfg(ENABLE_TTF_CONSOLE)]
    Ttf(TtfConsole<'a>),
    #[cfg(not(any(ENABLE_BITFONT_CONSOLE, ENABLE_TTF_CONSOLE)))]
    None(core::marker::PhantomData<&'a ()>),
}

impl Write for ConsoleImpl<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self {
            #[cfg(ENABLE_BITFONT_CONSOLE)]
            ConsoleImpl::Bitfont(c, _) => c.write_str(s),
            #[cfg(ENABLE_TTF_CONSOLE)]
            ConsoleImpl::Ttf(c) => c.write_str(s),
            #[cfg(not(any(ENABLE_BITFONT_CONSOLE, ENABLE_TTF_CONSOLE)))]
            ConsoleImpl::None(_) => Ok(()),
        }
    }
}

lazy_static! {
    pub static ref CONSOLE: Mutex<ConsoleImpl<'static>> = {
        let console_type = crate::config::DEFAULT_CONSOLE_TYPE;
        #[cfg(ENABLE_TTF_CONSOLE)]
        if console_type == "ttf" {
            return Mutex::new(ConsoleImpl::Ttf(TtfConsole::init()));
        }
        #[cfg(ENABLE_BITFONT_CONSOLE)]
        if console_type == "bitfont" {
            return Mutex::new(ConsoleImpl::Bitfont(BitfontConsole::init(), core::marker::PhantomData));
        }

        // Fallback
        #[cfg(ENABLE_BITFONT_CONSOLE)]
        return Mutex::new(ConsoleImpl::Bitfont(BitfontConsole::init(), core::marker::PhantomData));
        #[cfg(all(not(ENABLE_BITFONT_CONSOLE), ENABLE_TTF_CONSOLE))]
        return Mutex::new(ConsoleImpl::Ttf(TtfConsole::init()));
        #[cfg(not(any(ENABLE_BITFONT_CONSOLE, ENABLE_TTF_CONSOLE)))]
        return Mutex::new(ConsoleImpl::None(core::marker::PhantomData));
    };
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
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
        ConsoleType::Bitfont => {
            ConsoleImpl::Bitfont(BitfontConsole::init(), core::marker::PhantomData)
        }
        #[cfg(ENABLE_TTF_CONSOLE)]
        ConsoleType::Ttf => ConsoleImpl::Ttf(TtfConsole::init()),
    };
}

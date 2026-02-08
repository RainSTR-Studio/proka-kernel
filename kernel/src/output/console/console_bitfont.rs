extern crate alloc;

use crate::graphics::{color, Color};
use crate::output::font8x16::FONT8X16;
use crate::FRAMEBUFFER_REQUEST;
use alloc::{vec, vec::Vec};
use core::fmt::{self, Write};

// Constants
const FONT_W: u64 = 8;
const FONT_H: u64 = 16;

/// The ANSI parse status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    Normal,           // Normal mode
    Escape,           // Has read ESC char
    EscapeBracket,    // Already read ESC[
    CollectingParams, // Params for collecting
}

/// The object of console.
pub struct BitfontConsole {
    address: *mut u8,
    width: u64,
    height: u64,
    pitch: u64,
    position: (u64, u64), // (x, y)
    fg_color: Color,
    bg_color: Color,
    parse_state: ParseState,
    ansi_params: Vec<u16>,
    current_param: u16, // Current ANSI param
}

// We have to do it, so that it can be contained by Mutex.
unsafe impl Send for BitfontConsole {}
unsafe impl Sync for BitfontConsole {}

impl BitfontConsole {
    pub fn init() -> Self {
        let framebuffer_response = FRAMEBUFFER_REQUEST.get_response().unwrap();
        let framebuffer = framebuffer_response.framebuffers().next().unwrap();
        Self {
            address: framebuffer.addr(),
            width: framebuffer.width(),
            height: framebuffer.height(),
            pitch: framebuffer.pitch(),
            position: (0, 0),
            fg_color: color::WHITE,
            bg_color: color::BLACK,
            parse_state: ParseState::Normal,
            ansi_params: Vec::new(),
            current_param: 0,
        }
    }

    #[inline(always)]
    fn scroll_up(&mut self) {
        unsafe {
            let base_ptr = self.address;
            let scroll_bytes = (FONT_H as usize) * self.pitch as usize;
            let total_bytes = (self.height as usize) * self.pitch as usize;

            // Use memmove
            // Move all buffer upper scroll_bytes
            core::ptr::copy(
                base_ptr.add(scroll_bytes),
                base_ptr,
                total_bytes - scroll_bytes,
            );

            // Clear last scroll_bytes area
            core::ptr::write_bytes(base_ptr.add(total_bytes - scroll_bytes), 0, scroll_bytes);
        }
    }

    /// Print a char to framebuffer console.
    ///
    /// Note that the character must discovorable in ASCII, otherwise we don't know what
    /// unexpected thing is being happened.
    pub fn print_char(&mut self, c: u8) {
        // Parse ANSI symbol
        match self.parse_state {
            ParseState::Normal => {
                if c == 0x1B {
                    // ESC char
                    self.parse_state = ParseState::Escape;
                } else {
                    self.print_normal_char(c as usize);
                }
            }
            ParseState::Escape => {
                if c == b'[' {
                    self.parse_state = ParseState::EscapeBracket;
                    self.ansi_params.clear();
                    self.current_param = 0;
                } else {
                    // Invalid symbol, fall back
                    self.parse_state = ParseState::Normal;
                    self.print_normal_char(0x1B_usize);
                    self.print_normal_char(c as usize);
                }
            }
            ParseState::EscapeBracket => {
                if c.is_ascii_digit() {
                    self.current_param = self.current_param * 10 + (c - b'0') as u16;
                    self.parse_state = ParseState::CollectingParams;
                } else if c == b';' {
                    self.ansi_params.push(self.current_param);
                    self.current_param = 0;
                } else {
                    self.ansi_params.push(self.current_param);
                    self.parse_ansi_command(c);
                    self.parse_state = ParseState::Normal;
                }
            }
            ParseState::CollectingParams => {
                if c.is_ascii_digit() {
                    self.current_param = self.current_param * 10 + (c - b'0') as u16;
                } else if c == b';' {
                    self.ansi_params.push(self.current_param);
                    self.current_param = 0;
                    self.parse_state = ParseState::EscapeBracket;
                } else {
                    self.ansi_params.push(self.current_param);
                    self.parse_ansi_command(c);
                    self.parse_state = ParseState::Normal;
                }
            }
        }
    }

    /// Print a normal char to console.
    fn print_normal_char(&mut self, c: usize) {
        if (self.height - self.position.1) < FONT_H {
            self.scroll_up();
            self.position.1 = self.height - FONT_H;
        }

        // Backspace
        if c == 0x08 {
            if self.position.0 >= FONT_W {
                self.position.0 -= FONT_W;

                // Clear the character (draw background rect)
                let start_x = self.position.0;
                let start_y = self.position.1;

                // We use the same loop logic as print_normal_char but writing bg_color everywhere
                for line in 0..FONT_H {
                    for i in 0..FONT_W {
                        let x = start_x + i;
                        let y = start_y + line;
                        if x < self.width && y < self.height {
                            let pixel_offset = y * self.pitch + x * 4;
                            unsafe {
                                self.address
                                    .add(pixel_offset as usize)
                                    .cast::<u32>()
                                    .write(self.bg_color.to_u32(true));
                            }
                        }
                    }
                }
            }
            return;
        }

        // If character is "\n", just switch to next line.
        if c == ('\n' as usize) {
            self.position.0 = 0;
            self.position.1 += FONT_H;
            return;
        }

        // If over than self.width, switch to next line.
        if self.position.0 + FONT_W > self.width {
            self.position.0 = 0;
            self.position.1 += FONT_H;
        }

        // Compute the current position
        let start_x = self.position.0;
        let start_y = self.position.1;

        // Write pixel
        for line in 0..FONT_H {
            for i in 0..FONT_W {
                let mask = 0x80 >> i;

                // Write white pixel
                // Compute current pixel offset
                let x = start_x + i;
                let y = start_y + line;
                if x < self.width && y < self.height {
                    let pixel_offset = y * self.pitch + x * 4;
                    if FONT8X16[c][line as usize] & mask != 0 {
                        // Write
                        unsafe {
                            self.address
                                .add(pixel_offset as usize)
                                .cast::<u32>()
                                .write(self.fg_color.to_u32(true));
                        }
                    } else {
                        unsafe {
                            self.address
                                .add(pixel_offset as usize)
                                .cast::<u32>()
                                .write(self.bg_color.to_u32(true));
                        }
                    }
                }
            }
        }

        self.position.0 += FONT_W;
    }

    /// Handle ANSI commands
    fn parse_ansi_command(&mut self, cmd: u8) {
        match cmd {
            b'm' => self.handle_sgr(),
            b'J' => self.handle_erase_display(),
            b'A' => self.handle_cursor_up(),
            b'B' => self.handle_cursor_down(),
            b'C' => self.handle_cursor_right(),
            b'D' => self.handle_cursor_left(),
            _ => {} // Ignore non-impl command
        }
    }

    // SGR command handler
    fn handle_sgr(&mut self) {
        let params = if self.ansi_params.is_empty() {
            vec![0]
        } else {
            self.ansi_params.clone()
        };
        for param in params {
            match param {
                0 => {
                    // Reset
                    self.fg_color = color::WHITE;
                    self.bg_color = color::BLACK;
                }
                // Normal foreground color
                30 => self.fg_color = color::BLACK,
                31 => self.fg_color = color::RED,
                32 => self.fg_color = color::GREEN,
                33 => self.fg_color = color::YELLOW,
                34 => self.fg_color = color::BLUE,
                35 => self.fg_color = color::MAGENTA,
                36 => self.fg_color = color::CYAN,
                37 => self.fg_color = color::WHITE,
                // Normal background color
                40 => self.bg_color = color::BLACK,
                41 => self.bg_color = color::RED,
                42 => self.bg_color = color::GREEN,
                43 => self.bg_color = color::YELLOW,
                44 => self.bg_color = color::BLUE,
                45 => self.bg_color = color::MAGENTA,
                46 => self.bg_color = color::CYAN,
                47 => self.bg_color = color::WHITE,
                _ => {}
            }
        }
        self.ansi_params.clear();
        self.current_param = 0;
    }

    // Handle erase display ANSI command (ESC [ n J)
    // n=0: clear from cursor to end of screen
    // n=1: clear from beginning to cursor
    // n=2: clear entire screen and move cursor to top-left
    fn handle_erase_display(&mut self) {
        let param = if self.ansi_params.is_empty() {
            0
        } else {
            self.ansi_params[0]
        };

        match param {
            2 => {
                // Clear entire screen and reset cursor
                unsafe {
                    core::ptr::write_bytes(self.address, 0, (self.height * self.pitch) as usize);
                }
                self.position = (0, 0);
            }
            0 => {
                // Clear from cursor to end of screen
                let start_x = self.position.0;
                let start_y = self.position.1;
                let start_offset = start_y * self.pitch + start_x * 4;
                let total_bytes = (self.height * self.pitch) as usize;
                let clear_bytes = total_bytes - start_offset as usize;
                unsafe {
                    core::ptr::write_bytes(self.address.add(start_offset as usize), 0, clear_bytes);
                }
            }
            1 => {
                // Clear from beginning to cursor
                let end_offset = self.position.1 * self.pitch + self.position.0 * 4 + 4 * FONT_W;
                unsafe {
                    core::ptr::write_bytes(self.address, 0, end_offset as usize);
                }
            }
            _ => {}
        }
        self.ansi_params.clear();
        self.current_param = 0;
    }

    /// Print a string to console.
    pub fn print_string(&mut self, s: &str) {
        for c in s.bytes() {
            self.print_char(c);
        }
    }

    /* Settings methods */
    /// Set up the color of foreground character.
    pub fn set_fg_color(&mut self, color: Color) {
        self.fg_color = color
    }

    /// Set up the color of background character.
    pub fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color
    }

    /// Get the color of foreground character.
    pub fn get_fg_color(&self) -> Color {
        self.fg_color
    }

    /// Get the color of background character.
    pub fn get_bg_color(&self) -> Color {
        self.bg_color
    }

    /* Cursor handler */
    // Todo: Implement cursor
    pub fn handle_cursor_up(&mut self) {}
    pub fn handle_cursor_down(&mut self) {}
    pub fn handle_cursor_left(&mut self) {}
    pub fn handle_cursor_right(&mut self) {}
}

// Implement the [`Write`] trait to support formatting
impl Write for BitfontConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.print_string(s);
        Ok(())
    }
}

// Implement the unified [`Console`] trait
impl crate::output::console::Console for BitfontConsole {
    fn clear(&mut self) {
        self.position = (0, 0);
        unsafe {
            core::ptr::write_bytes(self.address, 0, (self.height * self.pitch) as usize);
        }
    }

    fn set_fg_color(&mut self, color: Color) {
        self.fg_color = color;
    }

    fn set_bg_color(&mut self, color: Color) {
        self.bg_color = color;
    }

    fn get_fg_color(&self) -> Color {
        self.fg_color
    }

    fn get_bg_color(&self) -> Color {
        self.bg_color
    }

    fn put_char(&mut self, ch: char) {
        if ch.is_ascii() {
            self.print_char(ch as u8);
        } else {
            // For non-ASCII characters, use "?" to replace.
            self.print_char(b'?');
        }
    }

    fn cursor_up(&mut self, _lines: u32) {
        // TODO: Implement cursor up
        let y = self.position.1.saturating_sub(FONT_H * _lines as u64);
        self.position.1 = y;
    }

    fn cursor_down(&mut self, _lines: u32) {
        // TODO: Implement cursor down
        let new_y = self.position.1 + FONT_H * _lines as u64;
        if new_y + FONT_H <= self.height {
            self.position.1 = new_y;
        }
    }

    fn cursor_left(&mut self, _cols: u32) {
        // TODO: Implement cursor left
        let x = self.position.0.saturating_sub(FONT_W * _cols as u64);
        self.position.0 = x;
    }

    fn cursor_right(&mut self, _cols: u32) {
        // TODO: Implement cursor right
        let new_x = self.position.0 + FONT_W * _cols as u64;
        if new_x + FONT_W <= self.width {
            self.position.0 = new_x;
        }
    }

    fn set_cursor_pos(&mut self, x: u32, y: u32) {
        self.position.0 = (x as u64 * FONT_W).min(self.width - FONT_W);
        self.position.1 = (y as u64 * FONT_H).min(self.height - FONT_H);
    }

    fn get_cursor_pos(&self) -> (u32, u32) {
        (
            (self.position.0 / FONT_W) as u32,
            (self.position.1 / FONT_H) as u32,
        )
    }

    fn cursor_hide(&mut self) {
        // TODO: Implement cursor hide
    }

    fn cursor_show(&mut self) {
        // TODO: Implement cursor show
    }
}

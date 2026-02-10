//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the panic handler with tests and normal.

use crate::color;
use crate::graphics::Color;
use crate::output::font8x16::FONT8X16;
use crate::serial_println;
use crate::FRAMEBUFFER_REQUEST;

use core::fmt::Write;
use core::panic::PanicInfo;

static BG_COLOR: Color = color!(0, 117, 210);

struct PanicConsole<'a> {
    framebuffer: limine::framebuffer::Framebuffer<'a>,
    x: u64,
    y: u64,
}

impl<'a> PanicConsole<'a> {
    fn new(framebuffer: limine::framebuffer::Framebuffer<'a>) -> Self {
        Self {
            framebuffer,
            x: 20,
            y: 20,
        }
    }

    fn set_pixel(&self, x: u64, y: u64, color: u32) {
        if x >= self.framebuffer.width() || y >= self.framebuffer.height() {
            return;
        }
        let offset = y * self.framebuffer.pitch() + x * (self.framebuffer.bpp() as u64 / 8);
        unsafe {
            self.framebuffer
                .addr()
                .add(offset as usize)
                .cast::<u32>()
                .write_volatile(color);
        }
    }

    fn clear(&self, color: u32) {
        for y in 0..self.framebuffer.height() {
            for x in 0..self.framebuffer.width() {
                self.set_pixel(x, y, color);
            }
        }
    }

    fn write_char(&mut self, c: char, fg: u32, bg: u32) {
        if c == '\n' {
            self.x = 20;
            self.y += 16;
            return;
        }
        let glyph = FONT8X16[c as usize & 0x7F];
        for row in 0..16 {
            for col in 0..8 {
                let color = if glyph[row] & (0x80 >> col) != 0 {
                    fg
                } else {
                    bg
                };
                self.set_pixel(self.x + col as u64, self.y + row as u64, color);
            }
        }
        self.x += 8;
        if self.x + 8 > self.framebuffer.width() - 20 {
            self.x = 20;
            self.y += 16;
        }
    }
}

impl<'a> Write for PanicConsole<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let fb = &self.framebuffer;
        let fg = ((255 as u32) << fb.red_mask_shift())
            | ((255 as u32) << fb.green_mask_shift())
            | ((255 as u32) << fb.blue_mask_shift());
        let bg = ((BG_COLOR.r as u32) << fb.red_mask_shift())
            | ((BG_COLOR.g as u32) << fb.green_mask_shift())
            | ((BG_COLOR.b as u32) << fb.blue_mask_shift());

        for c in s.chars() {
            self.write_char(c, fg, bg);
        }
        Ok(())
    }
}

// This is the default panic handler
#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    serial_println!("{}", info);

    if let Some(response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = response.framebuffers().next() {
            let bg = ((BG_COLOR.r as u32) << framebuffer.red_mask_shift())
                | ((BG_COLOR.g as u32) << framebuffer.green_mask_shift())
                | ((BG_COLOR.b as u32) << framebuffer.blue_mask_shift());

            let mut console = PanicConsole::new(framebuffer);
            console.clear(bg);

            let _ = write!(console, "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\n");
            let _ = write!(console, "!!                                                                            !!\n");
            let _ = write!(console, "!!                            PROKA KERNEL PANIC                              !!\n");
            let _ = write!(console, "!!                                                                            !!\n");
            let _ = write!(console, "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!\n");
            let _ = write!(console, "\n");
            let _ = write!(
                console,
                "A problem has been detected and the system has been halted to prevent damage.\n"
            );
            let _ = write!(console, "\n");
            let _ = write!(console, "Reason: \n{}\n", info.message());
            let _ = write!(console, "\n");
            if let Some(location) = info.location() {
                let _ = write!(
                    console,
                    "Location: {}, line {}\n",
                    location.file(),
                    location.line()
                );
            }
            let _ = write!(console, "\n\n");
            let _ = write!(console, "Please restart your computer.\n");
        }
    }

    loop {}
}

// And this is for test
#[cfg(test)]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    // Call the default test panic handler
    panic_for_test(info)
}

// This is the panic handler for all testing function
#[cfg(test)]
pub fn panic_for_test(info: &PanicInfo) -> ! {
    serial_println!("[FAILED]");
    serial_println!("Caused by:\n\t{}", info);
    crate::test::long_jmp();
}

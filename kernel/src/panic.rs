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

/// Information about a fatal CPU exception or kernel error
pub struct ExceptionInfo {
    pub name: &'static str,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
    pub error_code: Option<u64>,
}

pub static EXCEPTION_INFO: spin::RwLock<Option<ExceptionInfo>> = spin::RwLock::new(None);

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
        let fg = ((255) << fb.red_mask_shift())
            | ((255) << fb.green_mask_shift())
            | ((255) << fb.blue_mask_shift());
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
#[warn(unused_must_use)]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    let boot_time = crate::libs::time::time_since_boot();

    let mut rax: u64;
    let mut rbx: u64;
    let mut rcx: u64;
    let mut rdx: u64;
    let mut rsi: u64;
    let mut rdi: u64;
    let mut rbp: u64;
    let mut rsp: u64;
    let mut r8: u64;
    let mut r9: u64;
    let mut r10: u64;
    let mut r11: u64;
    let mut r12: u64;
    let mut r13: u64;
    let mut r14: u64;
    let mut r15: u64;
    let mut rip: u64;

    unsafe {
        core::arch::asm!("mov {}, rax", out(reg) rax);
        core::arch::asm!("mov {}, rbx", out(reg) rbx);
        core::arch::asm!("mov {}, rcx", out(reg) rcx);
        core::arch::asm!("mov {}, rdx", out(reg) rdx);
        core::arch::asm!("mov {}, rsi", out(reg) rsi);
        core::arch::asm!("mov {}, rdi", out(reg) rdi);
        core::arch::asm!("mov {}, rbp", out(reg) rbp);
        core::arch::asm!("mov {}, rsp", out(reg) rsp);
        core::arch::asm!("mov {}, r8", out(reg) r8);
        core::arch::asm!("mov {}, r9", out(reg) r9);
        core::arch::asm!("mov {}, r10", out(reg) r10);
        core::arch::asm!("mov {}, r11", out(reg) r11);
        core::arch::asm!("mov {}, r12", out(reg) r12);
        core::arch::asm!("mov {}, r13", out(reg) r13);
        core::arch::asm!("mov {}, r14", out(reg) r14);
        core::arch::asm!("mov {}, r15", out(reg) r15);
        core::arch::asm!("lea {}, [rip]", out(reg) rip);
    }

    let rflags_gathered = x86_64::registers::rflags::read_raw();

    let mut rflags = rflags_gathered;
    let mut is_exception = false;
    let mut exc_name = "";
    let mut exc_error = None;

    if let Some(info) = EXCEPTION_INFO.try_read() {
        if let Some(exc) = info.as_ref() {
            rip = exc.rip;
            rflags = exc.rflags;
            rsp = exc.rsp;
            is_exception = true;
            exc_name = exc.name;
            exc_error = exc.error_code;
        }
    }

    serial_println!("{}", info);

    if let Some(response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = response.framebuffers().next() {
            let bg = ((BG_COLOR.r as u32) << framebuffer.red_mask_shift())
                | ((BG_COLOR.g as u32) << framebuffer.green_mask_shift())
                | ((BG_COLOR.b as u32) << framebuffer.blue_mask_shift());

            let mut console = PanicConsole::new(framebuffer);
            console.clear(bg);

            let _ = writeln!(
                console,
                "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
            );
            let _ = writeln!(
                console,
                "!!                            PROKA KERNEL PANIC                              !!"
            );
            let _ = writeln!(
                console,
                "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!"
            );
            let _ = writeln!(console);
            let _ = writeln!(
                console,
                "A problem has been detected and the system has been halted to prevent damage."
            );
            let _ = writeln!(console);
            let _ = writeln!(console, "--- ERROR INFO ---");
            let _ = writeln!(console, "Reason:   {}", info.message());
            if is_exception {
                let _ = writeln!(console, "Exception: {}", exc_name);
                if let Some(err) = exc_error {
                    let _ = writeln!(console, "Error Code: {:#x}", err);
                }
            }
            if let Some(location) = info.location() {
                let _ = writeln!(
                    console,
                    "Location: {}, line {}",
                    location.file(),
                    location.line()
                );
            }
            let _ = writeln!(console);
            let _ = writeln!(console, "--- SYSTEM STATE ---");
            let _ = writeln!(console, "Boot Time: {:.4}s", boot_time);
            let _ = writeln!(console, "RIP:       {:#018x}", rip);
            let _ = writeln!(console, "RFLAGS:    {:#018x}", rflags);
            let _ = writeln!(console);
            let _ = writeln!(console, "--- REGISTERS ---");
            let _ = writeln!(console, "RAX: {:#018x}  RBX: {:#018x}", rax, rbx);
            let _ = writeln!(console, "RCX: {:#018x}  RDX: {:#018x}", rcx, rdx);
            let _ = writeln!(console, "RSI: {:#018x}  RDI: {:#018x}", rsi, rdi);
            let _ = writeln!(console, "RBP: {:#018x}  RSP: {:#018x}", rbp, rsp);
            let _ = writeln!(console, "R8:  {:#018x}  R9:  {:#018x}", r8, r9);
            let _ = writeln!(console, "R10: {:#018x}  R11: {:#018x}", r10, r11);
            let _ = writeln!(console, "R12: {:#018x}  R13: {:#018x}", r12, r13);
            let _ = writeln!(console, "R14: {:#018x}  R15: {:#018x}", r14, r15);
            let _ = writeln!(console);
            let _ = writeln!(console, "Please restart your computer.");
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
    serial_println!("Caused by:\t{}", info);
    crate::test::long_jmp();
}

//! Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All Rights Reserved.
//!
//! Well, welcome to the main entry of Proka Kernel!!
//!
//! If you have jumped here successfully, that means your CPU
//! can satisfy our kernel's requirements.
//!
//! Now, let's enjoy the kernel written in Rust!!!!
//!
//! For more information, see https://github.com/RainSTR-Studio/proka-kernel

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(proka_kernel::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
extern crate proka_kernel;
use proka_bootloader::BootInfo;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text")]
pub extern "C" fn kernel_main(info: &BootInfo) -> ! {
    let info = *info;

    // Init IDT first
    proka_kernel::tables::idt::init();

    let framebuffer = info.framebuffer();
    unsafe {
        let ptr = framebuffer.address() as *mut u8;
        for i in 0..500 {
            let offset = framebuffer.pitch() * i + i * framebuffer.bpp();
            ptr.add(offset as usize).cast::<u32>().write(0x00FFFFFF);
        }
    }
    loop {}
}

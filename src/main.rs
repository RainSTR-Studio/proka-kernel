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
#[macro_use]
extern crate log;
use proka_bootloader::header::Header;

// Kernel header definition
#[unsafe(link_section = ".header")]
#[used]
static KERNEL_HEADER: Header = Header::default();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".main")]
pub extern "C" fn kernel_main() -> ! {
    // Init GDT
    proka_kernel::tables::gdt::init();
    // Init IDT
    proka_kernel::tables::idt::init();

    // Print messages
    println!("[INFO] Successfully loaded kernel.");

    // Copyrights
    println!("\x1b[36m[INFO] Proka Kernel v0.1.0");
    println!("[INFO] Copyright (C) RainSTR Studio 2026, All rights reserved.\x1b[0m");

    println!("[INFO] Begin to initialize kernel staff...");

    // Re-init the kernel page
    proka_kernel::memory::init();
    println!("[INFO] Initialized memory manager.");

    // Init logger for convenient
    proka_kernel::logger::init();
    info!("Initialized log system.");

    // Start do MMIO mapping
    info!("Starting the MMIO mapping process...");
    proka_kernel::mmio::init();
    success!("Completed MMIO mapping process.");

    // Init APIC
    info!("Initializing APIC...");
    proka_kernel::apic::init();
    success!("Completed APIC initialization.");

    // Enable interrupt
    x86_64::instructions::interrupts::enable();

    loop {}
}

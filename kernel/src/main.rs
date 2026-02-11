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

/* Module imports */
#[macro_use]
extern crate proka_kernel;
extern crate alloc;

use proka_kernel::{BASE_REVISION, output::console::CONSOLE};
/* The Kernel main code */
// The normal one
#[unsafe(no_mangle)]
pub extern "C" fn kernel_main() -> ! {
    // Check is limine version supported
    assert!(BASE_REVISION.is_supported(), "Limine version not supported");

    // Init interrupts early for Page Fault handling during memory init
    proka_kernel::interrupts::gdt::init(); // Initialize GDT
    proka_kernel::interrupts::idt::init_idt(); // Initialize IDT

    // Initialize memory management
    proka_kernel::memory::init(); // Initialize memory management
    proka_kernel::libs::logger::init_logger(); // Init log system

    // Initialize ACPI and APIC
    proka_kernel::libs::acpi::init();
    proka_kernel::interrupts::apic::init();
    proka_kernel::interrupts::apic::ioapic::init();

    // Register Keyboard Handler (Simplified)
    proka_kernel::interrupts::request_irq(1, "Keyboard", |_context| {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x60);
        let scancode = unsafe { port.read() };
        proka_kernel::drivers::input::keyboard::KEYBOARD.handle_scancode(scancode);
        proka_kernel::interrupts::apic::registry::IrqResult::Handled
    });

    // Register Timer Handler via Registry
    proka_kernel::interrupts::apic::registry::IRQ_REGISTRY
        .write()
        .register(
            proka_kernel::interrupts::apic::TIMER_VECTOR,
            "Timer",
            |_context| {
                use proka_kernel::output::console::BITFONT_CURSOR_VISIBLE;
                use core::sync::atomic::{AtomicU64, Ordering};
                static TICKS: AtomicU64 = AtomicU64::new(0);
                let t = TICKS.fetch_add(1, Ordering::Relaxed);
                if t > 0 && t % 20 == 0 {
                    unsafe {
                        let current = BITFONT_CURSOR_VISIBLE.load(Ordering::Relaxed);
                        BITFONT_CURSOR_VISIBLE.store(!current, Ordering::Relaxed);
                        CONSOLE.lock().show_cursor(!current);
                    }
                }
                proka_kernel::interrupts::apic::registry::IrqResult::Handled
            },
        )
        .expect("Failed to register timer handler");

    proka_kernel::drivers::init_devices(); // Initialize devices
    proka_kernel::libs::time::init(); // Init time system

    proka_kernel::libs::initrd::load_initrd(); // Load initrd
    x86_64::instructions::interrupts::enable(); // Enable interrupts

    #[allow(unused_parens)]
    if (proka_kernel::config::ADDITIONAL_VERSION.is_empty()) {
        println!(
            "Starting \x1b[36mProka Kernel v{}\x1b[0m",
            env!("CARGO_PKG_VERSION")
        );
    } else {
        println!(
            "Starting \x1b[36mProka Kernel v{}-{}\x1b[0m",
            env!("CARGO_PKG_VERSION"),
            proka_kernel::config::ADDITIONAL_VERSION
        );
    }

    println!("Device list:");
    for device in proka_kernel::drivers::DEVICE_MANAGER
        .read()
        .list_devices()
        .iter()
    {
        println!("{:?}", device);
    }

    let st = proka_kernel::libs::time::time_since_boot();
    println!("A");
    let et = proka_kernel::libs::time::time_since_boot();
    println!("Time elasped for println! is {} ms", (et - st) * 1000.0);

    let time = proka_kernel::libs::time::time_since_boot();
    println!("Time since boot: {time}");
    
    let shell = proka_kernel::libs::shell::Shell::new();
    shell.run("keyboard");

    loop {
        x86_64::instructions::hlt();
    }
}

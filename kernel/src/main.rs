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

use core::alloc::Layout;

use proka_kernel::{
    libs::time::rtc, memory::FRAME_ALLOCATOR, output::console::CONSOLE, BASE_REVISION,
};
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
    println!("Start time: {:?}", rtc::now_local().to_iso8601());

    // Register Keyboard Handler (Simplified)
    proka_kernel::interrupts::request_irq(1, "Keyboard", |_context| {
        let mut port = x86_64::instructions::port::Port::<u8>::new(0x60);
        let scancode = unsafe { port.read() };
        proka_kernel::drivers::input::ps2::keyboard::KEYBOARD.handle_scancode(scancode);
        proka_kernel::interrupts::apic::registry::IrqResult::Handled
    });

    // Register Timer Handler via Registry
    #[allow(static_mut_refs)]
    proka_kernel::interrupts::apic::registry::IRQ_REGISTRY
        .write()
        .register(
            proka_kernel::interrupts::apic::TIMER_VECTOR,
            "Cursor Blinker",
            |_context| {
                use core::sync::atomic::{AtomicU64, Ordering};
                use proka_kernel::output::console::BITFONT_CURSOR_VISIBLE;
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
            "Starting \x1b[36mProka Kernel v{}\x1b[0m...",
            env!("CARGO_PKG_VERSION")
        );
    } else {
        println!(
            "Starting \x1b[36mProka Kernel v{}-{}\x1b[0m...",
            env!("CARGO_PKG_VERSION"),
            proka_kernel::config::ADDITIONAL_VERSION
        );
    }
    unsafe {
        println!("=== Memory Management Verification ===");
        let layout = Layout::from_size_align(5 * 1024 * 1024, 8).unwrap();

        // 1. First Allocation
        let ptr1 = alloc::alloc::alloc(layout);
        if ptr1.is_null() {
            println!("\x1b[31m[FAIL] First 5MB allocation failed!\x1b[0m");
        } else {
            println!("1. Allocated 5MB at {:p}", ptr1);
            proka_kernel::memory::paging::print_memory_stats(&FRAME_ALLOCATOR);

            // 2. Deallocate
            alloc::alloc::dealloc(ptr1, layout);
            println!(
                "2. Deallocated 5MB at {:p} (Heap allocator should cache these pages)",
                ptr1
            );
            proka_kernel::memory::paging::print_memory_stats(&FRAME_ALLOCATOR);

            // 3. Second Allocation (Verify reuse)
            let ptr2 = alloc::alloc::alloc(layout);
            if ptr2 == ptr1 {
                println!("\x1b[32m3. [SUCCESS] Reallocated same memory at {:p}. Heap reuse confirmed.\x1b[0m", ptr2);
            } else if !ptr2.is_null() {
                println!(
                    "3. Reallocated memory at {:p} (different from first).",
                    ptr2
                );
            } else {
                println!("\x1b[31m3. [FAIL] Second 5MB allocation failed!\x1b[0m");
            }
            proka_kernel::memory::paging::print_memory_stats(&FRAME_ALLOCATOR);

            if !ptr2.is_null() {
                alloc::alloc::dealloc(ptr2, layout);
            }
        }

        println!("\n=== Direct Frame Allocator Verification ===");
        let stats_before = FRAME_ALLOCATOR.stats();
        println!("Used frames before: {}", stats_before.used_frames);

        // Allocate 100 frames directly from Buddy Allocator
        if let Some(frame) = FRAME_ALLOCATOR.allocate_contiguous(100) {
            let stats_alloc = FRAME_ALLOCATOR.stats();
            println!(
                "Allocated 100 frames at {:#x}. Used: {}",
                frame.start_address().as_u64(),
                stats_alloc.used_frames
            );

            // Deallocate immediately
            FRAME_ALLOCATOR.deallocate_contiguous(frame, 100);
            let stats_after = FRAME_ALLOCATOR.stats();
            println!("Deallocated 100 frames. Used: {}", stats_after.used_frames);

            if stats_after.used_frames == stats_before.used_frames {
                println!("\x1b[32m[SUCCESS] Physical frame deallocation verified.\x1b[0m");
            } else {
                println!(
                    "\x1b[31m[FAIL] Physical frame count mismatch! (Expected {}, got {})\x1b[0m",
                    stats_before.used_frames, stats_after.used_frames
                );
            }
        } else {
            println!("\x1b[31m[FAIL] Direct frame allocation (100 frames) failed!\x1b[0m");
        }
        println!("======================================\n");
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

    proka_kernel::libs::pci::print_all_pci_devices();

    let shell = proka_kernel::libs::shell::Shell::new();
    shell.run("keyboard");

    loop {
        x86_64::instructions::hlt();
    }
}

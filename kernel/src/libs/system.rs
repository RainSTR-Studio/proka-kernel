//! System control functions for Proka Kernel.
//! Copyright (C) RainSTR Studio 2026, All rights reserved.

use x86_64::instructions::port::Port;

/// Shutdown the system.
///
/// This function attempts to shutdown the system using various methods.
/// Currently supported: QEMU, Bochs, VirtualBox, and Cloud Hypervisor.
///
/// Note: Full ACPI S5 shutdown requires parsing AML to get SLP_TYP values,
/// which is complex in the current `acpi` 6.0.1 environment.
/// Magic ports are used as a reliable fallback for emulators.
pub fn shutdown() -> ! {
    unsafe {
        // QEMU/Bochs
        Port::<u16>::new(0x604).write(0x2000);
        // VirtualBox
        Port::<u16>::new(0x4004).write(0x3400);
        // QEMU (older versions)
        Port::<u16>::new(0xB004).write(0x2000);
        // Cloud Hypervisor
        Port::<u16>::new(0x3C).write(0x01);
    }

    // If we reach here, shutdown failed
    loop {
        x86_64::instructions::hlt();
    }
}

/// Reboot the system.
///
/// This function attempts to reboot the system by pulsing the reset line
/// via the PS/2 controller.
pub fn reboot() -> ! {
    unsafe {
        let mut port = Port::<u8>::new(0x64);
        // Pulse reset line
        port.write(0xFE);
    }

    // If we reach here, reboot failed
    loop {
        x86_64::instructions::hlt();
    }
}

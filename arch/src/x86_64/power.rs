//! Arch-specified power actions.
use crate::tables::idt::IDT_EMPTY;
use log::warn;
use x86_64::instructions::port::Port;

/// Hard reboot.
pub fn hard_reboot() -> ! {
    // For unexpected situations, it will use this.
    warn!(
        "Failed to use ACPI to perform reboot, will use old port method to trigger hard reboot..."
    );

    // Port consts
    const KBD_PORT: u16 = 0x64;
    const KBD_RESET: u8 = 0xFE;

    unsafe {
        Port::<u8>::new(KBD_PORT).write(KBD_RESET);
    }

    // Commonly, the PC has shut down.
    // But if CPU still at here, we shall cause triple fault...
    warn!("Port force reboot failed, have to use triple fault...");
    IDT_EMPTY.load();
    unsafe { core::arch::asm!("int3", options(noreturn)) }
}

pub fn hard_poweroff() -> ! {
    warn!(
        "Failed to use ACPI to perform poweroff, will use old port method to trigger hard poweroff..."
    );
    unsafe {
        core::arch::asm!("out dx, ax", in("dx")(0x604), in("ax")(0x2000), options(noreturn));
    }
}

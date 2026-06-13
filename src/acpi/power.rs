//! The power system which is based on ACPI.
use super::ACPI_PLATFORM;
use crate::tables::idt::IDT_EMPTY;
use acpi::address::AddressSpace;
use acpi::sdt::fadt::Fadt;
use log::{debug, warn};
use spin::Lazy;
use x86_64::instructions::port::Port;

/// The FADT table.
static FADT: Lazy<Fadt> = Lazy::new(|| {
    let fadt = ACPI_PLATFORM.tables.find_table::<Fadt>().unwrap();
    *fadt
});

/// Reboot function.
pub fn reboot() -> ! {
    // Hard reboot (Use port)...
    let hard_reboot = || -> ! {
        // For unexpected situations, it will use this.
        warn!(
            "Failed to use ACPI to perform reboot, will use old port method to trigger hard reboot..."
        );

        // Port consts
        const KBD_PORT: u16 = 0x64;
        const KBD_RESET: u8 = 0xFE;

        unsafe {
            let value = Port::<u8>::new(KBD_PORT).read();
            while (value & 0x02) != 0 {
                Port::<u8>::new(KBD_PORT).write(KBD_RESET);
            }
        }

        // Commonly, the PC has shut down.
        // But if CPU still at here, we shall cause triple fault...
        warn!("Port force reboot failed, have to use triple fault...");
        IDT_EMPTY.load();
        unsafe { core::arch::asm!("int3", options(noreturn)) }
    };

    // Use ACPI reboot method first...
    // Get FADT's reset value
    let reg = FADT.reset_register().map_err(|_| hard_reboot()).unwrap(); // Won't panic!
    let val = FADT.reset_value;

    // Check: Is value invalid
    if reg.address == 0 || reg.bit_width != 8 {
        hard_reboot();
    }

    match reg.address_space {
        AddressSpace::SystemIo => {
            debug!("Using port method...");

            // Convert and write port
            if let Ok(port) = u16::try_from(reg.address) {
                unsafe { Port::<u8>::new(port).write(val) };
                loop {}
            }
        }
        AddressSpace::SystemMemory => {
            debug!("Using MMIO method...");

            // Just write bytes
            let mmio_ptr = reg.address as *mut u8;
            unsafe { core::ptr::write(mmio_ptr, val) };
            loop {}
        }
        _ => (),
    }

    loop {}
}

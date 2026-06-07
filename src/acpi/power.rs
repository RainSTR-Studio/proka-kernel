//! The power system which is based on ACPI.
use super::ACPI_TABLE;
use acpi::sdt::fadt::Fadt;
use acpi::address::AddressSpace;
use log::{debug, warn};
use spin::Lazy;
use x86_64::instructions::port::Port;

/// The FADT table.
static FADT: Lazy<Fadt> = Lazy::new(|| {
    let fadt = ACPI_TABLE.find_table::<Fadt>().unwrap();
    *fadt
});

/// Power management initializator.
pub fn init() -> Result<(), &'static str> {
    let enable = FADT.acpi_enable;
    let cstate = FADT.smi_cmd_port;

    // Check: Is port zeroed
    if cstate == 0 {
        return Err("Port is zeroed");
    }

    // Convert to u16
    let smi_port = match u16::try_from(cstate) {
        Ok(port) => port,
        Err(_) => return Err("SMI port is larger than u16"),
    };

    unsafe {
        Port::<u8>::new(smi_port).write(enable);
    }

    // Spin loop...
    for _ in 0..100 {
        core::hint::spin_loop();
    }
    Ok(())
}

/// Reboot function.
pub fn reboot() -> ! {
    // Hard reboot (Use port)...
    let hard_reboot = || -> ! {
        // For unexpected situations, it will use this.
        warn!(
            "Failed to use ACPI to perform shutdown, will use old port method to trigger hard reboot..."
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

        loop {}
    };

    // Use ACPI reboot method first...
    // Get FADT's reset value
    let reg = FADT
        .reset_register()
        .map_err(|_| hard_reboot())
        .unwrap(); // Won't panic!
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

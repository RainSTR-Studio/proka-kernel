//! The power system which is based on ACPI.
mod reboot;
use super::ACPI_TABLE;
use acpi::sdt::fadt::Fadt;
pub use reboot::reboot;
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

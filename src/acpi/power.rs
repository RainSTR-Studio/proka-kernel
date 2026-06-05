//! The power system which is based on ACPI.
use acpi::sdt::fadt::Fadt;
use spin::Lazy;
use x86_64::instructions::port::Port;
use super::ACPI_TABLE;

/// The FADT table.
static FADT: Lazy<Fadt> = Lazy::new(|| {
    let fadt = ACPI_TABLE.find_table::<Fadt>().unwrap();
    *fadt
});

/// Power management initializator.
pub fn init() {
    // First, a mode set is required.
    let enable = FADT.acpi_enable;
    let cstate = FADT.smi_cmd_port;

    // Write port...
    unsafe { Port::<u8>::new(cstate as u16).write(enable) }
}
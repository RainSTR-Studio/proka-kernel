//! ACPI initialization (delegates to the x86_64 backend).
pub mod power;

/// ACPI initializator.
pub fn init() {
    proka_kernel_arch::acpi::init();
}

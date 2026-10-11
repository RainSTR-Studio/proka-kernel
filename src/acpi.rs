//! ACPI initialization (delegates to the x86_64 backend).
pub fn init() {
    proka_kernel_arch::acpi::init();
}

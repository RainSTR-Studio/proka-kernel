//! x86_64 architecture support.
pub mod acpi;
pub mod cpu;
pub mod devices;
pub mod handler;
pub mod lapic;
pub mod memory;
pub mod power;
pub mod process;
pub mod scheduler;
pub mod sdms;
#[cfg(feature = "output_serial")]
pub mod serial;
pub mod tables;

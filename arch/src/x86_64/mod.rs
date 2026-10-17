//! x86_64 architecture support.
pub mod acpi;
pub mod coredrv;
pub mod cpu;
pub mod devices;
pub mod handler;
pub mod lapic;
pub mod memory;
pub mod power;
pub mod process;
pub mod scheduler;
#[cfg(feature = "output_serial")]
pub mod serial;
pub mod syscall;
pub mod tables;

//! x86_64 architecture support.
//!
//! All architecture-specific code (x86_64 registers, inline asm,
//! IO ports, GDT/IDT/TSS, APIC, page tables, serial driver) lives here.
#![no_std]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![feature(abi_x86_interrupt)]
#![allow(clippy::empty_loop)]

pub mod acpi;
pub mod apic;
pub mod coredrv;
pub mod cpu;
pub mod devices;
pub mod handler;
pub mod memory;
pub mod pci;
pub mod power;
pub mod process;
pub mod scheduler;
#[cfg(feature = "output_serial")]
pub mod serial;
pub mod syscall;
pub mod tables;

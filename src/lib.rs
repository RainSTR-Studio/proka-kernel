//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025. Licensed under GNU GPLv3.
//!
//! This provides the public functions, and they will help you
//! to use the kernel functions easily.

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

/// The kernel version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub mod acpi;
pub mod apic;
pub mod handler;
pub mod initprt;
pub mod logger;
pub mod memory;
pub mod mmio;
pub mod output;
pub mod panic;
pub mod process;
pub mod scheduler;
pub mod tables;
pub mod test;

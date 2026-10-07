//! Proka Kernel - architecture-independent kernel core.
//!
//! This crate owns the generic kernel logic and boot-time `init`
//! orchestration. All x86_64-specific code is delegated to the
//! `proka-kernel-x86_64` crate (under `arch/x86_64`), and shared
//! definitions live in `proka-common`.

#![no_std]
#![feature(custom_test_frameworks)]
#![cfg_attr(test, no_main)]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![allow(clippy::empty_loop)]

// Re-export the common printing macros so that both the kernel binary
// (`#[macro_use] extern crate proka_kernel`) and this crate's own modules
// (`use crate::println`) can use them.
pub use proka_common::{print, println, success};

/// The kernel version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The build-time configuration, generated from Kconfig.toml.
pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub mod acpi;
pub mod devices;
pub mod initprt;
pub mod logger;
pub mod memory;
pub mod panic;
pub mod process;
pub mod syscall;
pub mod test;

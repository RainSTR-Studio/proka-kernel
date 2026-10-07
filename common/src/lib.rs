//! # Proka Common
//! Shared public definitions, types, traits and platform-independent
//! services of the Proka kernel.
#![no_std]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![allow(clippy::empty_loop)]

/// The kernel version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod logger;
pub mod output;
pub mod process;
pub mod syscall;

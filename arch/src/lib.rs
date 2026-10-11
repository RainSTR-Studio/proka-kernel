//! Arch-specified library.
#![no_std]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![allow(clippy::empty_loop)]

/* x86_64 specified exports */
#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

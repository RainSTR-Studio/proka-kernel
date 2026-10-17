/* This file is generated. Do not edit manually. */
//! Arch-specified library.
#![no_std]
#![cfg_attr(
    not(all(feature = "output_fb", feature = "output_serial")),
    allow(unused)
)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]
#![allow(clippy::empty_loop)]

#[doc = "x86_64 specified library."]
#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(not(all(target_arch = "x86_64",)))]
compile_error!("You are trying to compile this project by using an unsupported architecture.");

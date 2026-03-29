//! # Proka Kernel - A kernel for ProkaOS
//! Copyright (C) RainSTR Studio 2025, All rights reserved.
//!
//! This provides the public functions, and they will help you
//! to use the kernel functions easily.

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(abi_x86_interrupt)]
#![test_runner(crate::test::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod panic;
pub mod tables;

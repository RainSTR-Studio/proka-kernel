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

extern crate alloc;

pub mod drivers;
pub mod fs;
pub mod graphics;
pub mod interrupts;
pub mod libs;
pub mod memory;
pub mod output;
pub mod panic;
pub mod process;
pub mod test;

pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

// Re-export common memory management types and functions
pub use memory::frame::{format_bytes, FrameStats, LockedFrameAllocator};
pub use memory::paging::{
    get_hhdm_offset, get_memory_stats, init_frame_allocator, init_offset_page_table,
    print_memory_stats,
};
pub use memory::protection::{kernel_flags, user_flags, Protection};

use limine::{
    modules::InternalModule,
    request::{FramebufferRequest, HhdmRequest, MemoryMapRequest, ModuleRequest, RsdpRequest},
    BaseRevision,
};

/* The section data define area */
#[unsafe(link_section = ".requests")]
#[used]
/// The base revision of the kernel.
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

#[unsafe(link_section = ".requests")]
#[used]
/// The framebuffer request of the kernel.
pub static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[unsafe(link_section = ".requests")]
#[used]
pub static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[unsafe(link_section = ".requests")]
#[used]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[unsafe(link_section = ".requests")]
#[used]
pub static RSDP_REQUEST: RsdpRequest = RsdpRequest::new();

#[unsafe(link_section = ".requests")]
#[used]
pub static MODULE_REQUEST: ModuleRequest = ModuleRequest::new()
    .with_internal_modules(&[&InternalModule::new().with_path(config::INITRD_PATH)]);

/// This will extern the C function and make it to safe.
///
/// # Example
/// ```rust
/// use proka_kernel::extern_safe;
///
/// // Make sure that the C function was defined and linked currectly.
/// extern_safe! {
///     fn add(a: i32, b: i32) -> i32;
/// }
///
/// // Then use it, with the header "safe_".
/// let result = safe_add(1, 2);
/// assert_eq!(result, 3);
/// ```
#[macro_export]
macro_rules! extern_safe {
    (
        $(
            $(#[$meta:meta])*
            fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty;
        )*
    ) => {
        unsafe extern "C" {
            $(
                $(#[$meta])*
                fn $name($($arg: $ty),*) -> $ret;
            )*
        }

       $(
           paste::paste! {
                pub fn [<safe_ $name>]($($arg: $ty),*) -> $ret {
                    unsafe { $name($($arg),*) }
                }
            }
        )*
    };
}

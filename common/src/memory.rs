//! Architecture-independent memory management.
//!
//! The bitmap frame allocator and the frame-backed heap are pure memory
//! tracking logic: they know nothing about page-table formats, CPU
//! registers or address-space layout. Each architecture wraps them in a
//! thin adapter (e.g. `proka-kernel-arch::x86_64::memory`) that plugs in
//! its own physical-frame type and page-table machinery.

pub mod framealloc;
pub mod heap;

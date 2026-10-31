//! Log system (delegates to proka-kernel-core).
//!
//! The logger itself is architecture-independent and lives in
//! `proka-kernel-core::logger`; the level is supplied from kernel config.

pub fn init(level: &str) {
    proka_kernel_core::logger::init(level);
}

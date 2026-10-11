//! Log system (delegates to proka-kernel-common).
//!
//! The logger itself is architecture-independent and lives in
//! `proka-kernel-common::logger`; the level is supplied from kernel config.

pub fn init(level: &str) {
    proka_kernel_common::logger::init(level);
}

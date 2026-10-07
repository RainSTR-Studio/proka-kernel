//! Log system (delegates to proka-common).
//!
//! The logger itself is architecture-independent and lives in
//! `proka-common::logger`; the level is supplied from kernel config.

pub fn init(level: &str) {
    proka_common::logger::init(level);
}

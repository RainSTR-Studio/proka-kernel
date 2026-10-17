//! The log system.
use crate::println;
use log::{Log, Metadata, Record};

/// The kernel logger.
pub struct KernelLogger;

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level();

            let color = match record.level() {
                log::Level::Error => "\x1b[31m",
                log::Level::Warn => "\x1b[33m",
                log::Level::Info => "\x1b[37m",
                log::Level::Debug => "\x1b[34m",
                log::Level::Trace => "\x1b[35m",
            };

            println!("{}[{}] {}\x1b[0m", color, level, record.args());
        }
    }

    fn flush(&self) {}
}

#[macro_export]
macro_rules! success {
    ($($arg:tt)*) => {
         println!("\x1b[32m[SUCCESS] {}\x1b[0m", format_args!($($arg)*))
    };
}

/// Init logger system with the given level name.
pub fn init(level: &str) {
    static LOGGER: KernelLogger = KernelLogger;
    log::set_logger(&LOGGER).expect("Failed to set logger");

    match level {
        "trace" => log::set_max_level(log::LevelFilter::Trace),
        "debug" => log::set_max_level(log::LevelFilter::Debug),
        "info" => log::set_max_level(log::LevelFilter::Info),
        "warn" => log::set_max_level(log::LevelFilter::Warn),
        "error" => log::set_max_level(log::LevelFilter::Error),
        _ => log::set_max_level(log::LevelFilter::Info),
    }
}

use crate::println;
use log::{Log, Metadata, Record};

/// 自定义日志记录器
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

/// 初始化日志系统
pub fn init_logger() {
    static LOGGER: KernelLogger = KernelLogger;
    log::set_logger(&LOGGER).expect("Failed to set logger");
    match crate::config::LOG_LEVEL {
        "Trace" => log::set_max_level(log::LevelFilter::Trace),
        "Debug" => log::set_max_level(log::LevelFilter::Debug),
        "Info" => log::set_max_level(log::LevelFilter::Info),
        "Warn" => log::set_max_level(log::LevelFilter::Warn),
        "Error" => log::set_max_level(log::LevelFilter::Error),
        &_ => {}
    }
}

//! The process system.
pub mod driver;
pub mod normal;
pub use self::driver::DRIVER_PROCESS;
pub use self::normal::NORMAL_PROCESS;

/// The max process numbers
pub const MAX_PS: usize = 16384;

/// The status of the current process.
#[repr(u16)]
#[derive(Default, Debug, Clone, Copy)]
pub enum Status {
    /// Means the process is ready and being run.
    #[default]
    Ready = 0,

    /// Means the kernel is now running.
    Running = 1,
}

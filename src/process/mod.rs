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

/// The error of operations of process.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The memory is not enough to create process.
    MemoryNotEnough,

    /// The address is not aligned.
    AddressNotAligned,

    /// The process is not exist.
    ProcessNotExist,

    /// The index is invalid.
    InvalidIndex,
}

/// The type of processes.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcType {
    /// Normal process.
    Normal,

    /// Driver process.
    Driver,
}

/// The process register content.
#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct Context {
    // TODO: Extend more registers
    pub rsp: u64,
    pub rip: u64,
}

/// Create a process and push it into the process list.
///
/// # Note
/// Certain arguments are not required for specific process domains.
///
/// If an unsupported process domain is provided, it will be ignored, 
/// and process creation will continue normally.
pub fn create(proctype: ProcType, priority: u8) -> Result<(), Error> {
    match proctype {
        ProcType::Normal => create_normal(priority)?,
        ProcType::Driver => create_driver()?,
    }
    Ok(())
}

/// Create a normal process.
fn create_normal(priority: u8) -> Result<(), Error> {
    let process = match self::normal::NormalProcess::create(priority) {
        Ok(proc) => proc,
        Err(e) => return Err(e),
    };

    // Check which process is usable
    let mut table = NORMAL_PROCESS.lock();
    for i in 0..MAX_PS {
        if table.process[i].present != false {
            continue;
        }
        table.process[i] = process.clone();
    }

    // TODO: Update scheduler queue
    Ok(())
}

/// Create a driver process.
fn create_driver() -> Result<(), Error> {
    let process = match self::driver::DriverProcess::create() {
        Ok(proc) => proc,
        Err(e) => return Err(e),
    };

    // Check which process is usable
    let mut table = DRIVER_PROCESS.lock();
    for i in 0..MAX_PS {
        if table.process[i].present != false {
            continue;
        }
        table.process[i] = process.clone();
    }

    // TODO: Update scheduler queue
    Ok(())
}

/// Remove a process from the process list by type and index.
///
/// # Note
/// Certain arguments are not required for specific process domains.
/// If an unsupported process domain is provided, it will be ignored,
/// and process removal will continue normally.
pub fn remove(proctype: ProcType, index: usize) -> Result<(), Error> {
    match proctype {
        ProcType::Normal => remove_normal(index)?,
        ProcType::Driver => remove_driver(index)?,
    }
    Ok(())
}

/// Remove a normal process by index.
fn remove_normal(index: usize) -> Result<(), Error> {
    if index >= MAX_PS {
        return Err(Error::InvalidIndex);
    }

    let mut table = NORMAL_PROCESS.lock();
    let proc = &mut table.process[index];

    if !proc.present {
        return Err(Error::ProcessNotExist);
    }

    proc.remove();

    // TODO: Update scheduler queue
    Ok(())
}

/// Remove a driver process by index.
fn remove_driver(index: usize) -> Result<(), Error> {
    if index >= MAX_PS {
        return Err(Error::InvalidIndex);
    }

    let mut table = DRIVER_PROCESS.lock();
    let proc = &mut table.process[index];

    if !proc.present {
        return Err(Error::ProcessNotExist);
    }

    proc.remove();

    // TODO: Update scheduler queue
    Ok(())
}


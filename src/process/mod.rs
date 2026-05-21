//! The process system.
extern crate alloc;
use alloc::vec::Vec;
pub mod driver;
pub mod normal;
use crate::memory::MAPPER;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use log::trace;
use proka_exec::{Parser, header::ExecMode};
use x86_64::align_up;

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

    /// The PKE format is invalid.
    InvalidFormat,
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
#[derive(Debug, Clone)]
pub struct Context {
    // TODO: Extend more registers
    pub rsp: u64,
    pub rip: u64,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            rsp: 0x1FF000,
            rip: 0x200000,
        }
    }
}

/// Create a process and push it into the process list by passing a valid
/// PKE format data.
///
/// # Safety
/// Caller must ensure that the data is already mapped.
///
/// # Note
/// Certain arguments are not required for specific process domains.
///
/// If an unsupported process domain is provided, it will be ignored,
/// and process creation will continue normally.
pub unsafe fn create(data: &'static [u8], priority: u8) -> Result<(), Error> {
    // First, parse the current data
    // Check: is the current data a valid PKE format
    let proctype: ProcType;
    let mut section_info: Vec<(u64, u32)> = Vec::new(); // (addr, len)
    let mut allocator = FRAME_ALLOCATOR.lock();
    let _mapper = MAPPER.lock();

    // SAFETY: Caller has ensured that the slice is already mapped.
    unsafe {
        let parser = Parser::init(data).map_err(|_| Error::InvalidFormat)?;
        if !parser.validate() {
            return Err(Error::InvalidFormat);
        }
        trace!("Process: data validation passed");

        // Decide the process type through the header info
        proctype = match parser.header().mode {
            ExecMode::UserApp => ProcType::Normal,
            ExecMode::CoreDrv => ProcType::Driver,
        };

        // Todo: Complete PKE loading
        for section in parser.sections() {
            // Get the page needed
            let len = section.length;
            let pages = align_up(section.length as u64, 4096) / 4096;
            let frame = if let Some(frame) = allocator.allocate_contiguous(pages as usize) {
                trace!(
                    "Allocated {:?} for storing data with pages {} (actually {})",
                    frame, pages, len
                );
                frame
            } else {
                return Err(Error::MemoryNotEnough);
            };
            let addr = frame.start_address().as_u64();
            section_info.push((addr as u64, len));

            // Create up a slice that will copy into
            let slice = core::slice::from_raw_parts_mut(addr as *mut u8, len as usize);
            slice.copy_from_slice(&data[section.base as usize..(section.base + len) as usize]);
            trace!("Slice length: {}, content: {:?}", slice.len(), slice);
        }
    }

    match proctype {
        ProcType::Normal => create_normal(priority)?,
        ProcType::Driver => create_driver()?,
    }
    Ok(())
}

/// Create a normal process.
fn create_normal(priority: u8) -> Result<(), Error> {
    let process = self::normal::NormalProcess::create(priority)?;

    // Check which process is usable
    let mut table = NORMAL_PROCESS.lock();
    for i in 0..MAX_PS {
        if table.process[i].present {
            continue;
        }
        table.process[i] = process.clone();
    }

    // TODO: Update scheduler queue
    Ok(())
}

/// Create a driver process.
fn create_driver() -> Result<(), Error> {
    let process = self::driver::DriverProcess::create()?;

    // Check which process is usable
    let mut table = DRIVER_PROCESS.lock();
    for i in 0..MAX_PS {
        if table.process[i].present {
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

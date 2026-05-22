//! The process system.
extern crate alloc;
use alloc::vec::Vec;
use x86_64::structures::paging::{
    MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
pub mod driver;
pub mod normal;
use crate::memory::IdentityPageTableMapper;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use crate::scheduler::{DRIVER_QUEUE, NORMAL_QUEUE};
use log::{debug, error, trace, warn};
use proka_exec::{Parser, header::ExecMode};
use x86_64::{PhysAddr, VirtAddr, align_up};

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

    /// An error about page table.
    PageError,
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
            rsp: 0x3F000,
            rip: 0x200000,
        }
    }
}

/// Data about a section.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SectionData {
    pub addr: u64,
    pub pages: u64,
    pub executable: bool,
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
    let pml4: u64;
    let mut section_info: Vec<SectionData> = Vec::new(); // (addr, pages)
    let mut allocator = FRAME_ALLOCATOR.lock();

    // SAFETY: Caller has ensured that the slice is already mapped.
    unsafe {
        let parser = Parser::init(data).map_err(|_| Error::InvalidFormat)?;
        if !parser.validate() {
            warn!("Validation not pasded, abortinng process creation...");
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
            // Check is current section loadable
            if !section.is_loadable {
                trace!("This section is not loadable, passing...");
                continue;
            }

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
                error!("Memory not enough");
                return Err(Error::MemoryNotEnough);
            };
            let addr = frame.start_address().as_u64();

            // Construct and push into data
            let info = SectionData {
                addr,
                pages,
                executable: section.is_execable,
            };
            section_info.push(info);

            // Create up a slice that will copy into
            let slice = core::slice::from_raw_parts_mut(addr as *mut u8, len as usize);
            slice.copy_from_slice(&data[section.base as usize..(section.base + len) as usize]);
            trace!("Slice length: {}, content: {:?}", slice.len(), slice);
        }

        trace!("Section iteration has completed.");

        // After collecting info, its time to make up a page table.
        // But first, we need to make up an PML4
        pml4 = if let Some(frame) = allocator.allocate_contiguous(1) {
            trace!("Allocated frame {:?} for proc PML4", frame);
            frame.start_address().as_u64()
        } else {
            return Err(Error::MemoryNotEnough);
        };
        let pml4_table = &mut *(pml4 as *mut PageTable);
        let mut proc_mapper = MappedPageTable::new(pml4_table, IdentityPageTableMapper);

        // Time to allocate 2MiB for stack
        const STACK_PAGES: usize = 64; // Pages of stack needed
        let stack_base = if let Some(frame) = allocator.allocate_contiguous(STACK_PAGES) {
            trace!("Allocated frame {:?} for proc stack", frame);
            frame.start_address().as_u64()
        } else {
            return Err(Error::MemoryNotEnough);
        };

        for i in 0..STACK_PAGES as u64 {
            let virt_addr = VirtAddr::new(i * 0x1000);
            let page = Page::<Size4KiB>::containing_address(virt_addr);
            let phys_addr = PhysAddr::new(i * 0x1000 + stack_base);
            let frame = PhysFrame::<Size4KiB>::containing_address(phys_addr);
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            trace!("Mapping frame: {:?}, Page: {:?}...", frame, page);
            proc_mapper
                .map_to(page, frame, flags, &mut *allocator)
                .map_err(|e| {
                    warn!("Failed to map within error \"{:?}\"", e);
                    Error::PageError
                })?
                .ignore();
        }
        trace!("Stack mapping has been completed.");
    }

    match proctype {
        ProcType::Normal => create_normal(pml4, priority)?,
        ProcType::Driver => create_driver(pml4)?,
    }
    Ok(())
}

/// Create a normal process.
fn create_normal(frame: u64, priority: u8) -> Result<(), Error> {
    let process = self::normal::NormalProcess::create(frame, priority)?;

    // Check which process is usable
    let mut table = NORMAL_PROCESS.lock();
    let mut pid: usize = 0;
    for i in 0..MAX_PS {
        if !table.process[i].present {
            table.process[i] = process.clone();
            pid = i;
            break;
        }
        continue;
    }
    debug!("Allocated PID {} for this new process", pid);

    // Push into queue and return
    NORMAL_QUEUE.lock().push(pid);
    Ok(())
}

/// Create a driver process.
fn create_driver(frame: u64) -> Result<(), Error> {
    let process = self::driver::DriverProcess::create(frame)?;

    // Check which process is usable
    let mut table = DRIVER_PROCESS.lock();
    let mut did: usize = 0;
    for i in 0..MAX_PS {
        if !table.process[i].present {
            table.process[i] = process.clone();
            did = i;
            break;
        }
        continue;
    }
    debug!("Allocated DID {} for this new process", did);

    // Push into queue and return
    DRIVER_QUEUE.lock().push(did);
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

    NORMAL_QUEUE.lock().retain(|item| *item != index);
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

    DRIVER_QUEUE.lock().retain(|item| *item != index);
    Ok(())
}

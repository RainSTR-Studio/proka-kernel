//! The process system.
extern crate alloc;
use alloc::vec::Vec;
use x86_64::structures::paging::{
    MappedPageTable, Mapper, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
pub mod driver;
pub mod normal;
use crate::memory::IdentityPageTableMapper;
use crate::memory::PDPT_HPROC_ADDR;
use crate::memory::PML4_ADDR;
use crate::memory::framealloc::FRAME_ALLOCATOR;
use crate::memory::paging::PDPT_HIGH_ADDR;
use crate::scheduler::{DRIVER_QUEUE, NORMAL_QUEUE};
use crate::tables::gdt::GDT;
use log::{debug, error, trace, warn};
use proka_exec::{Parser, header::ExecMode};
use x86_64::registers::rflags::RFlags;
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
#[derive(Default, Debug, Clone)]
pub struct Context {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
    pub rip: u64,
    pub cs: u64,
    pub ss: u64,
}

impl Context {
    /// Create up a normal TCB.
    pub fn normal() -> Self {
        let sel = GDT.1;
        let rflags = RFlags::INTERRUPT_FLAG | RFlags::ID;
        Self {
            rsp: 0x7FFFFFFFF000,
            rbp: 0x7FFFFFFFF000,
            rip: 0x200000,
            rflags: rflags.bits(),
            cs: u64::from(sel.user_code.0),
            ss: u64::from(sel.user_data.0),
            ..Default::default()
        }
    }

    /// Create up a driver TCB.
    pub fn driver() -> Self {
        let sel = GDT.1;
        let rflags = RFlags::INTERRUPT_FLAG | RFlags::ID;
        Self {
            rsp: 0x7FFFFFFFF000,
            rbp: 0x7FFFFFFFF000,
            rip: 0x200000,
            rflags: rflags.bits(),
            cs: u64::from(sel.kernel_code.0),
            ss: u64::from(sel.kernel_data.0),
            ..Default::default()
        }
    }
}

/// Data about a section.
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
pub unsafe fn create(data: &[u8], priority: u8) -> Result<(), Error> {
    // First, parse the current data
    let mut section_info: Vec<SectionData> = Vec::new(); // (addr, pages)
    let parser = Parser::init(data).map_err(|_| Error::InvalidFormat)?;

    // Check: Is this is a valid PKE format
    if !parser.validate() {
        warn!("Validation not passed, aborting process creation...");
        return Err(Error::InvalidFormat);
    }
    trace!("Process: data validation passed");

    // Decide the process type through the header info
    let proctype: ProcType = match parser.header().mode {
        ExecMode::UserApp => ProcType::Normal,
        ExecMode::CoreDrv => ProcType::Driver,
    };

    // Do PKE loading
    for section in parser.sections() {
        // Check is current section loadable
        if !section.is_loadable {
            trace!("This section is not loadable, passing...");
            continue;
        }

        // Get the page needed
        let len = section.length;
        let pages = align_up(section.length as u64, 4096) / 4096;
        let frame = if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(pages as usize)
        {
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
        // SAFETY: This address is alreadt mapped and writable
        let slice = unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, len as usize) };
        slice.copy_from_slice(&data[section.base as usize..(section.base + len) as usize]);
        trace!("Slice length: {}, content: {:?}", slice.len(), slice);
    }

    trace!("Section iteration has completed.");

    // After collecting info, its time to make up a page table.
    // But first, we need to make up an PML4
    let pml4: u64 = if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(1) {
        trace!("Allocated frame {:?} for proc PML4", frame);
        frame.start_address().as_u64()
    } else {
        return Err(Error::MemoryNotEnough);
    };

    // Copy kernel's PML4 to do more handling
    // SAFETY: This address of PML4 is exist and target was allocated
    let pml4_table = unsafe {
        core::ptr::copy(PML4_ADDR as *const PageTable, pml4 as *mut PageTable, 1);
        &mut *(pml4 as *mut PageTable)
    };
    pml4_table.zero();
    if proctype == ProcType::Driver {
        // This defend driver write the kernel's page table to prevent
        // table was destoryed.
        pml4_table[256].set_addr(
            PhysAddr::new(PDPT_HPROC_ADDR),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
    } else {
        pml4_table[256].set_addr(
            PhysAddr::new(PDPT_HIGH_ADDR),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
    }
    let mut proc_mapper = unsafe { MappedPageTable::new(pml4_table, IdentityPageTableMapper) };

    // Time to allocate 2MiB for stack
    const STACK_PAGES: usize = 2; // Pages of stack needed
    let stack_base = if let Some(frame) = FRAME_ALLOCATOR.lock().allocate_contiguous(STACK_PAGES) {
        trace!("Allocated frame {:?} for proc stack", frame);
        frame.start_address().as_u64()
    } else {
        return Err(Error::MemoryNotEnough);
    };

    for i in 0..STACK_PAGES as u64 {
        let virt_addr = VirtAddr::new(i * 0x1000 + 0x7fffffffe000);
        let page = Page::<Size4KiB>::containing_address(virt_addr);
        let phys_addr = PhysAddr::new(i * 0x1000 + stack_base);
        let frame = PhysFrame::<Size4KiB>::containing_address(phys_addr);
        let flags = if proctype == ProcType::Driver {
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE
        } else {
            PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::NO_EXECUTE
                | PageTableFlags::USER_ACCESSIBLE
        };
        trace!("Mapping frame: {:?}, Page: {:?}...", frame, page);

        // SAFETY: All frame are allocated by allocator and it's currently not in use
        unsafe {
            proc_mapper
                .map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                .map_err(|e| {
                    warn!("Failed to map within error \"{:?}\"", e);
                    Error::PageError
                })?
                .ignore();
        }
    }
    trace!("Stack mapping has been completed.");

    // Then, let's put each sections by order...
    let mut current_page: u64 = 0;
    for info in section_info {
        for i in 0..info.pages {
            let virt_addr = VirtAddr::new(0x200000 + (current_page + i) * 0x1000);
            let page = Page::<Size4KiB>::containing_address(virt_addr);
            let phys_addr = PhysAddr::new(info.addr + i * 0x1000);
            let frame = PhysFrame::<Size4KiB>::containing_address(phys_addr);
            let flags = if info.executable {
                // For ring3/userapp, we still need add user accessable
                let basic = PageTableFlags::PRESENT;
                if proctype == ProcType::Normal {
                    basic | PageTableFlags::USER_ACCESSIBLE
                } else {
                    basic
                }
            } else {
                let basic =
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
                if proctype == ProcType::Normal {
                    basic | PageTableFlags::USER_ACCESSIBLE
                } else {
                    basic
                }
            };
            trace!("Mapping frame: {:?}, Page: {:?}", frame, page);

            // SAFETY: All frame are allocated and not in use
            unsafe {
                proc_mapper
                    .map_to(page, frame, flags, &mut *FRAME_ALLOCATOR.lock())
                    .map_err(|e| {
                        warn!("Failed to map with error \"{:?}\"", e);
                        Error::PageError
                    })?
                    .ignore();
            }

            // Add the current page counter...
            current_page += 1;
        }
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
    let mut table = NORMAL_PROCESS.write();
    let mut pid: usize = 0;
    for i in 0..MAX_PS {
        if !table.process[i].present {
            table.process[i] = process.clone();
            pid = i;
            break;
        }
        continue;
    }
    debug!("Allocated PID {} for this new normal process", pid);

    // Push into queue and return
    NORMAL_QUEUE.lock().push(pid);
    Ok(())
}

/// Create a driver process.
fn create_driver(frame: u64) -> Result<(), Error> {
    let process = self::driver::DriverProcess::create(frame)?;

    // Check which process is usable
    let mut table = DRIVER_PROCESS.write();
    let mut did: usize = 0;
    for i in 0..MAX_PS {
        if !table.process[i].present {
            table.process[i] = process.clone();
            did = i;
            break;
        }
        continue;
    }
    debug!("Allocated DID {} for this new driver process", did);

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

    let mut table = NORMAL_PROCESS.write();
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

    let mut table = DRIVER_PROCESS.write();
    let proc = &mut table.process[index];

    if !proc.present {
        return Err(Error::ProcessNotExist);
    }

    proc.remove();

    DRIVER_QUEUE.lock().retain(|item| *item != index);
    Ok(())
}

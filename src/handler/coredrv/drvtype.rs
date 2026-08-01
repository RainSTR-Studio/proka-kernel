//! The driver type call.
//!
//! For this module, we have the main function called [`driver_type_reg`], which will
//! done the registration of current's driver type.
//!
//! Also, the args of this call is required:
//!  - arg1: The main type of this driver. See [`DrvType`] for more info;
//!  - arg2: The subtype, which is the pointer of `&str` within 16 bytes length.
extern crate alloc;
use crate::{
    devices::{IS_PCIE, PCILIST, pci::PciCfgAccess, pcie::get_access},
    memory::{IdentityPageTableMapper, framealloc::FRAME_ALLOCATOR},
    process::DRIVER_PROCESS,
};
use alloc::vec::Vec;
use pci_types::{
    Bar::{self, Memory32, Memory64},
    ConfigRegionAccess, EndpointHeader, HeaderType, PciHeader,
};
use spin::{LazyLock, RwLock};
use x86_64::{
    PhysAddr, align_up,
    structures::paging::{
        MappedPageTable, Mapper, PageSize, PageTable, PageTableFlags, PhysFrame, Size2MiB,
        mapper::MapToError,
    },
};

/// The driver type index.
pub static DRVTYPE_INDEX: LazyLock<RwLock<Vec<DrvTypeTable>>> = LazyLock::new(|| {
    let table = Vec::new();
    RwLock::new(table)
});

/// The driver type table.
#[derive(Debug, Clone)]
pub struct DrvTypeTable {
    /// The ID of this driver.
    pub id: u16,

    /// The type of this driver.
    pub typ: DrvType,
}

/// The type of coredrv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(C)]
pub enum DrvType {
    /// Graphics driver.
    Graphics,

    /// Invalid driver type.
    Invalid,
}

/// The format of address.
#[repr(C, packed)]
pub struct AddrFormat {
    /// The base of address.
    pub base: u64,

    /// The size of address.
    pub size: u64,
}

impl DrvType {
    /// Create this enum from u64...
    pub fn from_u64(arg1: u64) -> Self {
        match arg1 {
            1 => Self::Graphics,
            _ => Self::Invalid,
        }
    }

    /// Create this from base class (u8).
    pub fn from_base_class(base_class: u8) -> Self {
        match base_class {
            0x03 => Self::Graphics,
            _ => Self::Invalid,
        }
    }
}

pub fn driver_type_reg(arg1: u64, _arg2: u64, did: u16) {
    let typ = DrvType::from_u64(arg1);

    // Check: is type invalid?
    if typ == DrvType::Invalid {
        return;
    }

    // Update index...
    let obj = DrvTypeTable { id: did, typ };
    DRVTYPE_INDEX.write().push(obj);

    // Create a page table mapper for this driver.
    let table_addr = DRIVER_PROCESS
        .read()
        .process
        .get(did as usize)
        .unwrap()
        .table_addr;
    let table = unsafe { &mut *(table_addr as *mut PageTable) };
    let mut mapper = unsafe { MappedPageTable::new(table, IdentityPageTableMapper) };

    /* Do MMIO mapping for driver. */
    // Create a list which records `Bar`
    let mut bar_list: Vec<AddrFormat> = Vec::new();

    // Let's iterate all of the PCI address (valid)...
    for addr in PCILIST.read().iter() {
        let header = PciHeader::new(*addr);
        if *IS_PCIE.get().unwrap() {
            // Use PCIe method
            let cfg_access = get_access(addr.segment()).unwrap();

            // Get base class, subclass (convert to `DeviceType`)
            let base_class = header.revision_and_class(cfg_access).1;

            // Check: Is this base class our wanted?
            let drv_type = DrvType::from_base_class(base_class);
            if drv_type != typ {
                continue;
            }

            // Check: Is this a endpoint device?
            // If yes, we can get the MMIO.
            if header.header_type(cfg_access) != HeaderType::Endpoint {
                continue;
            }

            // Do MMIO mapping...
            map_mmio(cfg_access, header, &mut mapper);
        } else {
            // For non-PCIe branch
            // Get config access
            let cfg_access = PciCfgAccess;

            // Get base class
            let base_class = header.revision_and_class(cfg_access).1;

            // Check: Is this base class our wanted?
            let drv_type = DrvType::from_base_class(base_class);
            if drv_type != typ {
                continue;
            }

            // Check: Is this a endpoint device?
            // If yes, we can get the MMIO.
            if header.header_type(cfg_access) != HeaderType::Endpoint {
                continue;
            }

            // Do MMIO mapping and get BAR...
            let bar = map_mmio(cfg_access, header, &mut mapper);

            // Check: Is bar none?
            if bar.is_none() {
                continue;
            }

            // Construct a [`AddrFormat`] and push into list...
            let bar = bar.expect("This message won't show");
            let addr_format = match bar {
                Memory32 {
                    address,
                    size,
                    prefetchable: _,
                } => AddrFormat {
                    base: address as u64,
                    size: size as u64,
                },
                Memory64 {
                    address,
                    size,
                    prefetchable: _,
                } => AddrFormat {
                    base: address,
                    size,
                },
                _ => continue,
            };
            bar_list.push(addr_format);
        }
    }

    // Once the iteration completed, write to driver-public area
    // Fixed at 0xffff800080000000.
    unsafe {
        core::ptr::copy_nonoverlapping(bar_list.as_ptr(), 0xffff800080000000 as *mut AddrFormat, 1)
    }
}

fn map_mmio<C>(
    cfg_access: C,
    header: PciHeader,
    mapper: &mut MappedPageTable<'_, IdentityPageTableMapper>,
) -> Option<Bar>
where
    C: ConfigRegionAccess + Copy,
{
    // Get MMIO
    let end_point = EndpointHeader::from_header(header, cfg_access).unwrap();
    // TODO: Adapt BAR0-BAR5
    let mmio = end_point.bar(0, cfg_access)?;

    // Match...
    let (addr, size) = match mmio {
        Memory32 {
            address,
            size,
            prefetchable: _,
        } => (address as u64, size as u64),
        Memory64 {
            address,
            size,
            prefetchable: _,
        } => (address, size),
        _ => return None,
    };

    // Do identity mapping...
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::WRITE_THROUGH
        | PageTableFlags::NO_CACHE
        | PageTableFlags::NO_EXECUTE;
    let pages = align_up(size, Size2MiB::SIZE) / Size2MiB::SIZE;
    for i in 0..pages {
        let frame =
            PhysFrame::<Size2MiB>::containing_address(PhysAddr::new(addr + i * Size2MiB::SIZE));

        unsafe {
            let mut frame_alloc = FRAME_ALLOCATOR.lock();
            match mapper.identity_map(frame, flags, &mut *frame_alloc) {
                Ok(flusher) => flusher.ignore(),
                Err(MapToError::PageAlreadyMapped(_)) => (),
                Err(e) => panic!("Cannot map that page: {:?}", e),
            }
        }
    }

    Some(mmio)
}

//! The MMIO Manager.
pub mod pci;
use lazy_static::lazy_static;
use log::{debug, trace};
use spin::Mutex;
use x86_64::structures::paging::{PageTable, PageTableFlags};
use x86_64::{PhysAddr, align_down};

// Constants
const PML4: u64 = 0x100000;
const BASE: u64 = 0xffffe00001000000;

/// Stores the table index offset.
pub static OFFSET_IDX: Mutex<u64> = Mutex::new(0);

// The MMIO table
lazy_static! {
    pub static ref MMIO: Mutex<MmioTable> = {
        let mut table = MmioTable::default();

        // Because the class 0x3 (display) is mapped for
        // 16MiB, so it's time to record it into table.
        table.entries[0].virt = 0xffffe00000000000;
        table.entries[0].length = 0x1000000;

        // The physical address will be filled in runtime.
        table.count += 1;

        Mutex::new(table)
    };
}

/// The main MMIO Table
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MmioTable {
    /// Entries of MMIO.
    pub entries: [MmioEntry; 64],

    /// Usable counts
    pub count: u8,
}

impl Default for MmioTable {
    fn default() -> Self {
        Self {
            entries: [MmioEntry::default(); 64],
            count: 0,
        }
    }
}

/// The MMIO table entry.
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct MmioEntry {
    /// The physical address.
    pub phys: u64,

    /// The mapped virtual address.
    pub virt: u64,

    /// The length that the MMIO used
    pub length: u64,

    /// The class of this MMIO
    pub class: u32,

    /// The subclass of this MMIO
    pub subclass: u32,
}

/// MMIO Initializator.
pub fn init() {
    // Scan PCI
    debug!("=====Begin of PCI device list=====");
    self::pci::pci_scan();
    debug!("=====End of PCI device list=====");

    // Now do MMIO mapping, also create a MMIO table.
    // By map the MMIO, we can just use Mapper, because
    // it's more convenient to do mapping.
    debug!("Doing MMIO mapping...");
    self::pci::pci_for_each(|dev| {
        // If base / size = 0, we don't map its MMIO.
        if dev.mmio_base == 0 || dev.mmio_size == 0 || dev.mmio_size > 0xff000000 {
            return;
        }

        // To make compatibility for framebuffer, the
        // display won't be mapped again.
        //
        // The other PCI address will start since the virt
        // 0xffffe00001000000.
        if dev.class == 0x03 {
            trace!("This class is display...");
            // If it is, just fill the Mmio Table
            let mut table = MMIO.lock();
            table.entries[0].phys = dev.mmio_base;
            trace!("(display) MMIO base: 0x{:08x}", dev.mmio_base);
        } else {
            // If not, just do standard mapping.
            trace!("Not in display...");
            let mut offset = 0u64;
            let mut offset_idx = OFFSET_IDX.lock();
            let base = BASE + *offset_idx * 0x200000;
            while offset <= dev.mmio_size {
                trace!("base: 0x{:16x}, offset_idx: 0x{:04x}", base, *offset_idx);
                align_down(dev.mmio_base, 4096);                
                mapper(dev.mmio_base, *offset_idx);
                offset += 0x200000;
            }

            // Get the offset in this mapped page
            let offset_in_page = dev.mmio_base & 0x1fffff;

            // Write to table
            let mut table = MMIO.lock();
            let idx = table.count as usize;
            table.entries[idx].phys = dev.mmio_base;
            table.entries[idx].virt = base + offset_in_page;
            table.entries[idx].length = dev.mmio_size;
            table.count += 1;

            // Update counter
            *offset_idx += offset / 0x200000;
            trace!("Offset index updated to {}", *offset_idx);
        }
    });
}

/// Global index to track current PDPT slot and PDT allocation
static MMIO_PDPT_IDX: Mutex<usize> = Mutex::new(0);
static MMIO_NEXT_PDT_PHY: Mutex<u64> = Mutex::new(0);

/// The mapper of other MMIO
fn mapper(phys: u64, offset_idx: u64) {
    // Now set up the basic things.
    let flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::HUGE_PAGE
        | PageTableFlags::NO_CACHE
        | PageTableFlags::WRITE_THROUGH; // UC

    // Create page table object
    let pml4 = unsafe { &mut *(PML4 as *mut PageTable) };
    let pdpt = unsafe { &mut *((pml4[448].addr().as_u64()) as *mut PageTable) };

    let mut pdpt_idx = MMIO_PDPT_IDX.lock();
    let mut next_pdt_phy = MMIO_NEXT_PDT_PHY.lock();

    let entry_idx = offset_idx + 8;

    // Out of PDT bounds: switch to next PDPT entry and allocate new PDT
    if entry_idx >= 512 {
        *pdpt_idx += 1;
        trace!("MMIO: PDT full, switch to PDPT[{}]", *pdpt_idx);

        // First new PDT starts at (pml4[448].addr() & 0xFFFF) + 0x8000
        if *next_pdt_phy == 0 {
            let base = pml4[448].addr().as_u64();
            *next_pdt_phy = (base & 0xFFFF) + 0x8000;
        }

        // Map new PDT into PDPT
        pdpt[*pdpt_idx].set_addr(PhysAddr::new(*next_pdt_phy), flags);
        *next_pdt_phy += 0x1000;
    }

    // Get current PDT from PDPT
    let pdt = unsafe { &mut *(pdpt[*pdpt_idx].addr().as_u64() as *mut PageTable) };
    let idx = (entry_idx % 512) as usize;

    trace!("Mapping phys {:08x} to pdt[{}]...", phys, idx);
    pdt[idx].set_addr(PhysAddr::new(phys), flags);
}

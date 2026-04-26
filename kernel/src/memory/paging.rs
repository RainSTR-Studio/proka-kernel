//! The paging module.
use super::TOTAL_RAM;
use x86_64::PhysAddr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::*;

const PML4_ADDR: u64 = 0x100000;
const PDPT_LOW_ADDR: u64 = 0x101000;
const PDPT_HIGH_ADDR: u64 = 0x102000;
const PDT_LOW_ADDR: u64 = 0x103000;
const PDT_HIGH_ADDR: u64 = 0x104000;
const PT_LOW_ADDR: u64 = 0x105000;
const PDT_LOW2_ADDR: u64 = 0x106000;

/// Initialize page tables for higher-half kernel
pub fn init() {
    // Init page table
    let pml4 = unsafe { &mut *(PML4_ADDR as *mut PageTable) };
    let pdpt_low = unsafe { &mut *(PDPT_LOW_ADDR as *mut PageTable) };
    let pdpt_high = unsafe { &mut *(PDPT_HIGH_ADDR as *mut PageTable) };
    let pdt_low = unsafe { &mut *(PDT_LOW_ADDR as *mut PageTable) };
    let pdt_high = unsafe { &mut *(PDT_HIGH_ADDR as *mut PageTable) };
    let pt_low = unsafe { &mut *(PT_LOW_ADDR as *mut PageTable) };

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    // Identity mapping 0x00000 ~ 0x200000 (2MiB)
    pml4[0].set_addr(PhysAddr::new(PDPT_LOW_ADDR), flags);
    pdpt_low[0].set_addr(PhysAddr::new(PDT_LOW_ADDR), flags);
    pdt_low[0].set_addr(PhysAddr::new(PT_LOW_ADDR), flags);

    // Low 2MiB PT mapping
    for i in 0..512 {
        pt_low[i].set_addr(PhysAddr::new((i * 0x1000) as u64), flags);
    }

    // Map the remaining space then
    let huge_flags = flags | PageTableFlags::HUGE_PAGE;
    for i in 1..512 {
        let phys = (i as u64) * 0x200000;
        pdt_low[i].set_addr(PhysAddr::new(phys), huge_flags);
    }

    // And the whole 256GiB
    let total_ram_bytes = TOTAL_RAM.get().unwrap().1;
    let mut pdt_current = PDT_LOW2_ADDR;
    for i_pdpt in 1..256 {
        let pdt = unsafe { &mut *(pdt_current as *mut PageTable) };
        let base_phys = 0x40000000 + ((i_pdpt - 1) as u64 * 0x40000000);
        for i_pdt in 0..512 {
            let phys = base_phys + (i_pdt as u64 * 0x200000);
            // Check: is current base addr over than phys
            if phys >= total_ram_bytes {
                break;
            }

            pdt[i_pdt].set_addr(PhysAddr::new(phys), huge_flags);
        }
        pdpt_low[i_pdpt].set_addr(PhysAddr::new(pdt_current), flags);
        pdt_current += 0x1000;

        // Check: is current base addr over than phys
        if base_phys >= total_ram_bytes {
            break;
        }
    }


    // Higher-half mapping (kernel):
    // Physical: 0x200000 ~ 0x2200000 (32MiB)
    // Virtual:  0xffff800000000000 ~
    pml4[256].set_addr(PhysAddr::new(PDPT_HIGH_ADDR), flags);
    pdpt_high[0].set_addr(PhysAddr::new(PDT_HIGH_ADDR), flags);

    // 32MiB = 16 PDTE
    for i in 0..16 {
        let phys = 0x200000 + (i as u64 * 0x200000);
        pdt_high[i].set_addr(PhysAddr::new(phys), huge_flags);
    }

    // Higher-half mapping (initrd):
    // Physical: 0x2200000 ~ 0x4200000 (32MiB)
    // Virtual: 0xffff800002000000

    // 32MiB = 16 PDTE
    let initrd_flags = PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE;
    for i in 16..32 {
        let phys = 0x2200000 + ((i - 16) as u64 * 0x200000);
        pdt_high[i].set_addr(PhysAddr::new(phys), initrd_flags);
    }

    // Map the framebuffer
    // At here, we just use the old page tables
    let (old_pml4_phys, _) = Cr3::read();
    let old_pml4 = unsafe { &*(old_pml4_phys.start_address().as_u64() as *const PageTable) };
    let fb_pdpt_phys = old_pml4[448].addr();

    pml4[448].set_addr(fb_pdpt_phys, flags);

    // And now, just write CR3
    unsafe {
        let pml4_addr = PhysAddr::new(PML4_ADDR);
        let pml4_frame = PhysFrame::containing_address(pml4_addr);
        Cr3::write(pml4_frame, Cr3Flags::empty());
    }
}

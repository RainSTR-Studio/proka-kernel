//! The paging module.
use core::ptr::addr_of;
use x86_64::PhysAddr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::*;
use x86_64::{align_down, align_up};

const PML4_ADDR: u64 = 0x100000;
const PDPT_LOW_ADDR: u64 = 0x101000;
const PDPT_HIGH_ADDR: u64 = 0x102000;
const PDT_LOW_ADDR: u64 = 0x103000;
const PDT_HIGH_ADDR: u64 = 0x104000;
const PT_LOW_ADDR: u64 = 0x105000;
const PDT_PROC_ADDR: u64 = 0x106000; // For process only
const PDT_GS_ADDR: u64 = 0x107000; // Global interrupt stack PDT (resolve conflict)
const PDT_GRW_ADDR: u64 = 0x108000; // Global Read-Write area
const PDT_LOW2_ADDR: u64 = 0x109000;

unsafe extern "C" {
    static __GDATA_START: u8;
    static __GDATA_END: u8;
}

/// Initialize page tables for higher-half kernel
pub fn init() {
    // Init page table
    let pml4 = unsafe { &mut *(PML4_ADDR as *mut PageTable) };
    let pdpt_low = unsafe { &mut *(PDPT_LOW_ADDR as *mut PageTable) };
    let pdpt_high = unsafe { &mut *(PDPT_HIGH_ADDR as *mut PageTable) };
    let pdt_low = unsafe { &mut *(PDT_LOW_ADDR as *mut PageTable) };
    let pdt_high = unsafe { &mut *(PDT_HIGH_ADDR as *mut PageTable) };
    let pt_low = unsafe { &mut *(PT_LOW_ADDR as *mut PageTable) };
    let pdt_proc = unsafe { &mut *(PDT_PROC_ADDR as *mut PageTable) };
    let pdt_gs = unsafe { &mut *(PDT_GS_ADDR as *mut PageTable) };
    let pdt_grw = unsafe { &mut *(PDT_GRW_ADDR as *mut PageTable) };

    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    let global_flags = flags | PageTableFlags::GLOBAL;
    let huge_flags = flags | PageTableFlags::HUGE_PAGE;

    // Identity mapping 0x00000 ~ 0x200000 (2MiB)
    pml4[0].set_addr(PhysAddr::new(PDPT_LOW_ADDR), flags);
    pdpt_low[0].set_addr(PhysAddr::new(PDT_LOW_ADDR), flags);
    pdt_low[0].set_addr(PhysAddr::new(PT_LOW_ADDR), flags);

    // Low 2MiB PT mapping
    for i in 0..512 {
        pt_low[i].set_addr(PhysAddr::new((i * 0x1000) as u64), flags);
    }

    // Map the remaining space with 2MiB huge page to 1GiB
    for i in 1..512 {
        let phys = (i as u64) * 0x200000;
        pdt_low[i].set_addr(PhysAddr::new(phys), huge_flags);
    }

    let mut current = PDT_LOW2_ADDR;

    // Higher-half mapping (kernel):
    // Physical: 0x200000 ~ 0x3200000 (48MiB)
    // Virtual:  0xffff800000000000 ~
    // Use 4KiB page granularity
    pml4[256].set_addr(PhysAddr::new(PDPT_HIGH_ADDR), flags);
    pdpt_high[0].set_addr(PhysAddr::new(PDT_HIGH_ADDR), flags);

    // Allocate PT pages sequentially with 'current'
    for i_pdt in 0..24 {
        let pt = unsafe { &mut *(current as *mut PageTable) };
        let base_phys = 0x200000 + (i_pdt as u64 * 0x200000);

        for i_pt in 0..512 {
            let phys = base_phys + (i_pt as u64 * 0x1000);
            pt[i_pt].set_addr(PhysAddr::new(phys), flags);
        }

        pdt_high[i_pdt].set_addr(PhysAddr::new(current), flags);
        current += 0x1000;
    }

    // Higher-half mapping (initrd):
    // Physical: 0x3200000 ~ 0x4200000 (16MiB)
    // Virtual: 0xffff800002000000
    // Use 2MiB huge page, no fine 4K control needed
    let initrd_flags = PageTableFlags::PRESENT | PageTableFlags::HUGE_PAGE;
    for i_pdt in 24..32 {
        let base_phys = 0x3200000 + ((i_pdt - 24) as u64 * 0x200000);
        pdt_high[i_pdt].set_addr(PhysAddr::new(base_phys), initrd_flags);
    }

    // Map global interrupt stack 0xFFFF800040000000 -> 0x4200000
    // Fill PDPT entry for global stack PDT
    pdpt_high[1].set_addr(PhysAddr::new(PDT_GS_ADDR), global_flags);
    // Map 2MiB huge page with Global flag
    pdt_gs[0].set_addr(
        PhysAddr::new(0x4200000),
        huge_flags | PageTableFlags::GLOBAL,
    );

    // Map global read-write data to 0xFFFF800080000000 -> 0x4400000~0x4600000
    // Fill PDPT[2] for global data
    pdpt_high[2].set_addr(PhysAddr::new(PDT_GRW_ADDR), global_flags);
    // Will divide into 2 forms
    // 0xFFFF800080000000~0xFFFF800080200000 is for both
    // kernel and drivers
    pdt_grw[0].set_addr(
        PhysAddr::new(0x4400000),
        huge_flags | PageTableFlags::GLOBAL | PageTableFlags::NO_EXECUTE,
    );
    // 0xFFFF800080200000~0xFFFF800080400000 is for among
    // kernel, drivers and user programs.
    pdt_grw[1].set_addr(
        PhysAddr::new(0x4600000),
        huge_flags | PageTableFlags::GLOBAL | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_EXECUTE,
    );

    // Higher half mapping (process-only)
    // SAFETY: Reading linker-defined symbols is safe
    let start = addr_of!(__GDATA_START) as u64;
    let end = addr_of!(__GDATA_END) as u64;

    // Align start address down to 4KiB boundary
    let start_aligned = align_down(start, 0x1000);
    // Align end address up to 4KiB boundary
    let end_aligned = align_up(end, 0x1000);

    // Higher half kernel virtual base
    let va_base = 0xFFFF800000000000;
    // Total aligned length of GDATA region
    let len = end_aligned - start_aligned;

    // Number of 2MiB and 4KiB times
    let pdt_times = (align_up(len, 0x200000) / 0x200000) as usize;
    let pt_times = (align_up(len, 0x1000) / 0x1000) as usize;

    // Starting PDT and PT index (from relative offset)
    let pdt_index: usize = ((start_aligned >> 21) & 0x1FF) as usize;
    let pt_index: usize = ((start_aligned >> 12) & 0x1FF) as usize;

    // Flags
    let proc_flags = flags | PageTableFlags::GLOBAL;
    // Convert virt to phys
    let gdata_phys = start_aligned - va_base + 0x200000;

    // Map process-only region with 4KiB granularity
    for i_pdt in pdt_index..pdt_index + pdt_times {
        let pt = unsafe { &mut *(current as *mut PageTable) };
        let mut count = 0;
        for i_pt in pt_index..pt_index + pt_times {
            let base_addr = gdata_phys + count * 0x1000;
            pt[i_pt].set_addr(PhysAddr::new(base_addr), proc_flags);
            count += 1;
        }

        pdt_proc[i_pdt].set_addr(PhysAddr::new(current), flags);
        current += 0x1000;
    }

    // Map the framebuffer
    // At here, we just use the old page tables
    let (old_pml4_phys, _) = Cr3::read();
    let old_pml4 = unsafe { &*(old_pml4_phys.start_address().as_u64() as *const PageTable) };
    let fb_pdpt_phys = old_pml4[448].addr();

    pml4[448].set_addr(fb_pdpt_phys, flags);

    // Reload CR3 with new page table
    unsafe {
        let pml4_addr = PhysAddr::new(PML4_ADDR);
        let pml4_frame = PhysFrame::containing_address(pml4_addr);
        Cr3::write(pml4_frame, Cr3Flags::empty());
    }
}

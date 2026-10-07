//! The GDT table
use super::tss::TSS;
use spin::LazyLock;
use x86_64::registers::segmentation::{CS, DS, ES, FS, GS, SS, Segment};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};

/// The GDT table
pub static GDT: LazyLock<(GlobalDescriptorTable, Selectors)> = LazyLock::new(|| {
    let mut gdt = GlobalDescriptorTable::new();

    let kcode = gdt.append(Descriptor::kernel_code_segment());
    let kdata = gdt.append(Descriptor::kernel_data_segment());
    let udata = gdt.append(Descriptor::user_data_segment());
    let ucode = gdt.append(Descriptor::user_code_segment());
    let tss = gdt.append(Descriptor::tss_segment(&TSS));

    (
        gdt,
        Selectors {
            kernel_code: kcode,
            kernel_data: kdata,
            user_code: ucode,
            user_data: udata,
            tss,
        },
    )
});

/// The segment selectors of GDT.
#[derive(Debug, Clone, Copy)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    pub tss: SegmentSelector,
}

/// Initialize and load GDT
pub fn init() {
    // Load GDT
    GDT.0.load();

    // Reload segment selectors
    let sel = GDT.1;

    // Safety: Valid GDT segment selectors for kernel ring 0.
    unsafe {
        // Update segment registers
        CS::set_reg(sel.kernel_code);
        DS::set_reg(sel.kernel_data);
        ES::set_reg(sel.kernel_data);
        FS::set_reg(sel.kernel_data);
        GS::set_reg(sel.kernel_data);
        SS::set_reg(sel.kernel_data);

        // Update TSS
        core::arch::asm!(
            "ltr {0:x}",
            in(reg) sel.tss.0,
            options(nostack, preserves_flags)
        );
    }
}

//! The GDT table
use spin::Lazy;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

#[unsafe(link_section = ".gdata")]
pub static GDT: Lazy<GlobalDescriptorTable> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    gdt.append(Descriptor::kernel_code_segment());
    gdt.append(Descriptor::kernel_data_segment());
    gdt.append(Descriptor::user_code_segment());
    gdt.append(Descriptor::user_data_segment());
    gdt
});

/// Initialize and load GDT
pub fn init() {
    GDT.load();
}

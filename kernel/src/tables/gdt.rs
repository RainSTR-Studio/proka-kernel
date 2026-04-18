//! The GDT table

use lazy_static::lazy_static;
use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor};

lazy_static! {
    pub static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());
        gdt.append(Descriptor::user_code_segment());
        gdt.append(Descriptor::user_data_segment());
        gdt
    };
}

pub fn init() {
    GDT.load();
}

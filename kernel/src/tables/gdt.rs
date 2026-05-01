//! The GDT table
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

#[unsafe(link_section = ".gdata")]
pub static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

pub fn init() {
    // SAFETY: Update the GDT won't destory data
    unsafe {
        let gdt = &mut *(&raw mut GDT);
        gdt.append(Descriptor::kernel_code_segment());
        gdt.append(Descriptor::kernel_data_segment());
        gdt.append(Descriptor::user_code_segment());
        gdt.append(Descriptor::user_data_segment());
        gdt.load();
    }
}

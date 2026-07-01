//! The syscall initializator.

use x86_64::{VirtAddr, registers::model_specific::{LStar, Star}};
use crate::{handler::syscall_entry, tables::gdt::GDT};

pub fn init() {
    // Firstly, update STAR registers
    let sel = GDT.1;
    Star::write(
        sel.user_code,
        sel.user_data,
        sel.kernel_code,
        sel.kernel_data,
    )
    .expect("Failed to do STAR register writing");

    // Then update LSTAR.
    let addr = syscall_entry as *const () as u64;
    LStar::write(VirtAddr::new(addr));
}

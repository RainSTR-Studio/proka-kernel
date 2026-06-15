//! The handler about IDT
//!
//! This code is originally by moyanj <me@moyanjdc.top>
mod exception;
mod apic;
use x86_64::structures::idt::InterruptStackFrame;
use crate::mmio::pci::PciDeviceTable;
pub use exception::*;
pub use apic::*;

const INFO_ADDR_DRIVER: u64 = 0xffff800080000000;

/// Common interrupt handler
#[unsafe(link_section = ".gdata")]
pub extern "x86-interrupt" fn coredrv(_: InterruptStackFrame) {
    // Switch table
    unsafe {
        core::arch::asm!("mov r8, 0x100000", "mov cr3, r8");
    }

    // At this time, we shall check up the interrupt
    let call_num: u64;
    let arg1: u64;
    let arg2: u64;

    unsafe {
        core::arch::asm!(
            "nop",
            out("rax") call_num,
            out("rdx") arg1,
            out("rsi") arg2,
        );
    }

    // After getting call_num and args, we shall match each...
    match call_num {
        // Get info call
        1 => get_info(arg1),

        // Driver type registing call
        2 => driver_type_reg(arg1, arg2),

        _ => return,
    }
}

/// Arg1: The info that you want to get
/// 
/// # Args
/// ## Arg1
///  - 0 => ACPI address
///  - 1 => PCI mapping table
/// 
/// # Returns
/// The info struct will put into 0xffff800080000000
fn get_info(arg1: u64) {
    match arg1 {
        0 => unsafe {
            // Get ACPI address
            let acpi = proka_bootloader::get_bootinfo().acpi();
            let addr = INFO_ADDR_DRIVER as *mut u64;
            *addr = acpi;
        },
        1 => unsafe {
            let pci_table = crate::mmio::pci::PCI_DEVICES.lock().clone();
            let addr = INFO_ADDR_DRIVER as *mut PciDeviceTable;
            *addr = pci_table;
        },
        _ => {
            // Invalid arg1
            return;
        },
    }
}

fn driver_type_reg(_arg1: u64, _arg2: u64) {}



//! The SDMS module (kernel-level orchestration).
//!
//! This is where the kernel decides *which* call numbers exist. The
//! architecture layer only provides the two mechanisms (`syscall` and
//! `int 0x40`) and the dispatch stubs; the table itself is populated here.
pub mod power;
use proka_kernel_core::sdms::{SDMSTABLE, SdmsEntry, dms::Callnum};

/// The kernel page table every call entry runs under.
const KERNEL_PAGE_TABLE: u64 = 0x100000;

/// SDMS initializator.
pub fn init() {
    // Architecture-specific half: prepare the `syscall` fast path.
    proka_kernel_arch::sdms::init_reg();

    let mut table = SDMSTABLE.write();

    // Generic half: register the kernel's own normal-process syscalls.
    // For syscall 0 (process management)
    table.sms_entry.push(SdmsEntry {
        sysnum: 0,
        page_table: KERNEL_PAGE_TABLE,
        entry: proka_kernel_arch::sdms::sms::process::process,
    });

    // For syscall 1 (power action)
    table.sms_entry.push(SdmsEntry {
        sysnum: 1,
        page_table: KERNEL_PAGE_TABLE,
        entry: self::power::power,
    });

    // For syscall 2 (memory allocation)
    table.sms_entry.push(SdmsEntry {
        sysnum: 2,
        page_table: KERNEL_PAGE_TABLE,
        entry: proka_kernel_arch::sdms::memory::memory,
    });

    // TODO: Add more types of syscall (0-16)

    // And the driver-process call numbers (`int 0x40`).
    // For driver call 1 (driver type registration)
    table.dms_entry.push(SdmsEntry {
        sysnum: Callnum::RegDriverType as u64,
        page_table: KERNEL_PAGE_TABLE,
        entry: proka_kernel_arch::sdms::dms::drvtype::driver_type_reg,
    });

    // TODO: Add more call numbers of the driver-call half.
    // `smsh_pfn` / `dmsh_pfn` stay empty until the shared pages are set up.
}

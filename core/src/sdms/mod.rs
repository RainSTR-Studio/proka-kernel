//! SDMS - the merged Syscall & Drvcall (SMS/DMS) subsystem.
extern crate alloc;
pub mod dms;
pub mod memory;
pub mod sms;
use alloc::vec::Vec;
use alloc::collections::BTreeSet;
use spin::RwLock;

/// The merged syscall / drvcall table.
pub static SDMSTABLE: RwLock<SdmsTable> = RwLock::new(SdmsTable {
    temp_stack: BTreeSet::new(),
    smsh_pfn: Vec::new(),
    sms_entry: Vec::new(),
    dms_entry: Vec::new(),
    dmsh_pfn: Vec::new(),
});

/// One call table, split by the class of the caller.
#[derive(Debug)]
pub struct SdmsTable {
    /// SMS/DMS temporaty stack used index set.
    ///
    /// Calc: 0xffff8008_00000000 + index * 0x40000
    pub temp_stack: BTreeSet<u64>,

    /// Page frame numbers shared with normal processes.
    pub smsh_pfn: Vec<u64>,

    /// Entry of each syscall...
    pub sms_entry: Vec<SdmsEntry>,

    /// Entry of each drvcall...
    pub dms_entry: Vec<SdmsEntry>,

    /// Page frame numbers shared with driver processes.
    pub dmsh_pfn: Vec<u64>,
}

/// A single call entry.
#[derive(Debug, Clone, Copy)]
pub struct SdmsEntry {
    /// The call number.
    pub sysnum: u64,

    /// The dest page table.
    ///
    /// The system will automatically switch into this table and pass the
    /// arguments through.
    pub page_table: u64,

    /// The dest entry point address after switching table.
    pub entry: extern "C" fn(u64, u64, u64, u64, u64, u64, u64) -> i64,
}

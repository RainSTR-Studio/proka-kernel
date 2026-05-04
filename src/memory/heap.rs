//! The heap allocator
use talc::{source::Claim, *};

// Heap size from 0xffff800001800000
const HEAP_BASE: u64 = 0xffff800001800000;
const HEAP_SIZE: usize = 0x800000; // 8MiB

#[global_allocator]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> =
    TalcLock::new(unsafe { Claim::new(HEAP_BASE as *mut u8, HEAP_SIZE) });

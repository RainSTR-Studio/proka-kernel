//! The heap allocator
use spinning_top::RawSpinlock;
use talc::{source::Claim, *};

/// The end to heap
const HEAP_END: u64 = 0xffff800003000000;

#[global_allocator]
static TALC: TalcLock<RawSpinlock, Claim> = TalcLock::new(unsafe {
    // Todo: Use linker script to tell its start
    let start = 0xffff800000400000u64;
    let size = (HEAP_END - start) as usize;
    Claim::new(start as *mut u8, size)
});

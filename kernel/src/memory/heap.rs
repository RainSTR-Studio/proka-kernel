//! The heap allocator
use talc::{*, source::Claim};

// Heap size from 0x101000~0x1fffff
const HEAP_BASE: u64 = 0x101000;
const HEAP_SIZE: usize = 0xff000;   // 0x101000-0x1fffff

#[global_allocator]
static TALC: TalcLock<spinning_top::RawSpinlock, Claim> = TalcLock::new(unsafe {
    Claim::new(HEAP_BASE as *mut u8, HEAP_SIZE)
});

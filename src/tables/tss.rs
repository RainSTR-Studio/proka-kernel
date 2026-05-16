//! The TSS module.
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;

// Constants
const IST1_STACK_TOP: u64 = 0xFFFF8000400FF000;
const RSP0_STACK_TOP: u64 = 0xFFFF8000401FF000;

#[unsafe(link_section = ".gdata")]
pub static TSS: TaskStateSegment = {
    let mut tss = TaskStateSegment::new();
    tss.privilege_stack_table[0] = VirtAddr::new(RSP0_STACK_TOP);
    tss.interrupt_stack_table[0] = VirtAddr::new(IST1_STACK_TOP);
    tss
};

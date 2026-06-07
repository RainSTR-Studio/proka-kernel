//! The TSS module.
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;

// Constants
const IST1_COMMON: u64 = 0xFFFF8000400FF000;
const IST2_CRIT: u64 = 0xFFFF8000401FF000;

#[unsafe(link_section = ".gdata")]
pub static TSS: TaskStateSegment = {
    let mut tss = TaskStateSegment::new();
    tss.privilege_stack_table[0] = VirtAddr::new(IST1_COMMON);
    tss.interrupt_stack_table[0] = VirtAddr::new(IST1_COMMON);
    tss.interrupt_stack_table[1] = VirtAddr::new(IST2_CRIT);
    tss
};

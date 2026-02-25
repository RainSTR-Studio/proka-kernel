use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

// ============================================
// Segment Selectors for syscall/sysret
// ============================================
// GDT Layout for syscall/sysret compatibility:
// Index  | Selector | 用途
// -------|----------|------------------
// 0      | 0x00     | Null
// 1      | 0x08     | Kernel Code (64-bit) - CS_KERNEL
// 2      | 0x10     | Kernel Data - DS_KERNEL
// 3      | 0x18     | User Data (32-bit compat) - DS_USER_32
// 4      | 0x20     | User Code (32-bit compat) - CS_USER_32
// 5      | 0x28     | User Data (64-bit) - DS_USER
// 6      | 0x30     | User Code (64-bit) - CS_USER
// 7      | 0x38     | TSS

/// Kernel code segment selector (Ring 0)
pub const CS_KERNEL: u16 = 0x08;
/// Kernel data segment selector (Ring 0)
pub const DS_KERNEL: u16 = 0x10;
/// User data segment selector (Ring 3, 64-bit)
pub const DS_USER: u16 = 0x28 | 3; // Ring 3
/// User code segment selector (Ring 3, 64-bit)
pub const CS_USER: u16 = 0x30 | 3; // Ring 3
/// User data segment selector (32-bit compat, Ring 3)
pub const DS_USER_32: u16 = 0x18 | 3; // Ring 3
/// User code segment selector (32-bit compat, Ring 3)
pub const CS_USER_32: u16 = 0x20 | 3; // Ring 3

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 8192 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            stack_start + STACK_SIZE as u64
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());

        // User mode segments for syscall/sysret
        // GDT Index Layout:
        // 0: Null
        // 1: Kernel Code (0x08)
        // 2: Kernel Data (0x10)
        // 3: User Data (0x18)
        // 4: User Code (0x20)
        // 5-6: TSS (0x28)

        // Index 3: User Data
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        // Index 4: User Code
        let user_code_selector = gdt.append(Descriptor::user_code_segment());

        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                user_data_selector,
                user_code_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// Get the user code segment selector (for sysret)
pub fn user_code_selector() -> SegmentSelector {
    GDT.1.user_code_selector
}

/// Get the user data segment selector (for sysret)
pub fn user_data_selector() -> SegmentSelector {
    GDT.1.user_data_selector
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        SS::set_reg(GDT.1.data_selector);
        DS::set_reg(GDT.1.data_selector);
        ES::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}

pub mod error;
pub mod frame;
pub mod heap;
pub mod paging;
pub mod protection;

pub use frame::FRAME_ALLOCATOR;
pub use paging::vmm::translate_addr;
pub use paging::{phys_to_virt, virt_to_phys_direct};

pub fn init() {
    let memory_map_response = crate::MEMORY_MAP_REQUEST
        .get_response()
        .expect("Memory subsystem critical failure: Could not retrieve memory map from bootloader");
    let hhdm_offset = paging::get_hhdm_offset();
    let mut mapper = unsafe { paging::init_offset_page_table(hhdm_offset) };
    unsafe {
        paging::init_frame_allocator(memory_map_response);
    }
    let mut frame_allocator = FRAME_ALLOCATOR;

    // 1. Initialize heap with a small pre-mapped area for bootstrapping
    heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Memory subsystem critical failure: Failed to initialize kernel heap");

    // 2. Initialize VMM (uses heap for VMAs)
    paging::vmm::init(mapper);

    // Print memory stats
    paging::print_memory_stats(&frame_allocator);
}

/// Run sanity checks for the frame allocator
pub fn test_allocator_sanity() {
    use crate::println;
    use x86_64::structures::paging::FrameAllocator;

    let memory_map_response = crate::MEMORY_MAP_REQUEST
        .get_response()
        .expect("Failed to get memory map response");

    // We get the allocator instance safely
    // Since init_frame_allocator is idempotent (checks total_frames == 0),
    // calling it again is safe.
    unsafe {
        paging::init_frame_allocator(memory_map_response);
    }
    let mut allocator = FRAME_ALLOCATOR;

    println!("=== Testing Buddy Allocator ===");

    // 1. Single Frame Allocation
    if let Some(frame) = allocator.allocate_frame() {
        println!(
            "[PASS] Single frame allocated at {:#x}",
            frame.start_address()
        );
        allocator.deallocate_frame(frame);
        println!("[PASS] Single frame deallocated");
    } else {
        println!("[FAIL] Single frame allocation failed");
    }

    // 2. Contiguous Allocation (4 frames)
    println!("Testing contiguous allocation (4 frames)...");
    if let Some(start_frame) = allocator.allocate_contiguous(4) {
        let start_addr = start_frame.start_address().as_u64();
        println!("[PASS] 4 frames allocated starting at {:#x}", start_addr);

        // Verify they are contiguous? The allocator returns just the start frame.
        // But since we asked for 4 contiguous, we assume the physical memory is [start, start + 4*4096)
        // We can verify this by checking if we can write to them via HHDM (if mapped).
        // For now, trust the allocator logic but verify address alignment.

        assert_eq!(start_addr % 4096, 0, "Address not page aligned");
        // For order 2 (4 pages), it should be aligned to 4*4096?
        // Buddy system usually guarantees natural alignment for the block size.
        // 4 * 4096 = 16384 (0x4000).
        if start_addr % 0x4000 == 0 {
            println!("[PASS] Block is naturally aligned to 16KB");
        } else {
            println!(
                "[WARN] Block is NOT naturally aligned to 16KB (addr: {:#x})",
                start_addr
            );
        }

        allocator.deallocate_contiguous(start_frame, 4);
        println!("[PASS] 4 frames deallocated");
    } else {
        println!("[FAIL] Contiguous allocation failed");
    }

    println!("=== Allocator Tests Complete ===");
}

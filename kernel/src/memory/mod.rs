pub mod allocator;
pub mod frame_allocator;
pub mod paging;
pub mod protection;
pub mod vmm;

pub fn init() {
    let memory_map_response = crate::MEMORY_MAP_REQUEST
        .get_response()
        .expect("Failed to get memory map response");
    let hhdm_offset = paging::get_hhdm_offset();
    let mut mapper = unsafe { paging::init_offset_page_table(hhdm_offset) };
    let mut frame_allocator = unsafe { paging::init_frame_allocator(memory_map_response) };

    // 1. Initialize heap with a small pre-mapped area for bootstrapping
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Failed to init heap");

    // 2. Initialize VMM (uses heap for VMAs)
    vmm::init(mapper);

    // Print memory stats
    paging::print_memory_stats(&frame_allocator);
}

//! Buddy System Allocator
//!
//! This module implements a Buddy System frame allocator that manages physical memory
//! using embedded linked lists. It supports efficient allocation and deallocation of
//! power-of-two sized blocks of frames.
//!
//! # Metadata Storage
//! The allocator stores the `next` pointer for free blocks *inside* the free physical
//! pages themselves. This means the memory overhead is minimal (just the array of list heads).
//! This requires access to the physical memory via the Higher Half Direct Map (HHDM).

use core::cmp;
use core::ptr::NonNull;
use x86_64::structures::paging::{PhysFrame, Size4KiB};
use x86_64::PhysAddr;

use crate::memory::paging::get_hhdm_offset;

/// Magic value to identify free blocks.
/// This is used to probabilistically check if a buddy block is free.
/// We use a large random constant to minimize false positives with allocated data.
const FREE_BLOCK_MAGIC: u64 = 0xDEAD_BEEF_BAAD_F00D;

/// A free block in the buddy system.
/// This struct is stored at the beginning of the free memory block itself.
#[repr(C)]
struct FreeBlock {
    next: Option<NonNull<FreeBlock>>,
    prev: Option<NonNull<FreeBlock>>,
    magic: u64,
    order: usize,
}

/// A Buddy System Frame Allocator.
///
/// # generic parameters
/// * `MAX_ORDER`: The maximum order of the buddy system.
///   The allocator will manage blocks of size `2^0` to `2^(MAX_ORDER-1)` pages.
///   Default is 12 (up to 2^11 = 2048 pages = 8MiB blocks).
pub struct BuddyAllocator<const MAX_ORDER: usize = 12> {
    /// Array of free lists, one for each order.
    free_lists: [Option<NonNull<FreeBlock>>; MAX_ORDER],
    /// Total number of frames managed by the allocator.
    total_frames: usize,
    /// Number of frames currently allocated.
    used_frames: usize,
    /// Bitmap to track free frames (1 bit per frame).
    /// If bit is 1, the frame is free (and head of a free block).
    /// If bit is 0, the frame is allocated, or not the head of a free block, or unmapped.
    bitmap: &'static mut [u64],
}

// Sync is safe because the allocator is protected by a Mutex in the wrapper.
// However, the raw `BuddyAllocator` is not thread-safe itself.
// The `LockedFrameAllocator` wrapper handles the locking.
unsafe impl<const MAX_ORDER: usize> Send for BuddyAllocator<MAX_ORDER> {}

impl<const MAX_ORDER: usize> BuddyAllocator<MAX_ORDER> {
    /// Create a new empty buddy allocator.
    pub const fn new() -> Self {
        Self {
            free_lists: [None; MAX_ORDER],
            total_frames: 0,
            used_frames: 0,
            bitmap: &mut [],
        }
    }

    /// Set the bitmap for the allocator.
    pub fn set_bitmap(&mut self, bitmap: &'static mut [u64]) {
        self.bitmap = bitmap;
        // Clear bitmap
        for x in self.bitmap.iter_mut() {
            *x = 0;
        }
    }

    fn set_bit(&mut self, frame_idx: usize, val: bool) {
        if frame_idx / 64 >= self.bitmap.len() {
            return; // Out of bounds, ignore (should not happen if bitmap is large enough)
        }
        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        if val {
            self.bitmap[word_idx] |= 1 << bit_idx;
        } else {
            self.bitmap[word_idx] &= !(1 << bit_idx);
        }
    }

    fn get_bit(&self, frame_idx: usize) -> bool {
        if frame_idx / 64 >= self.bitmap.len() {
            return false; // Out of bounds is considered allocated/unmapped
        }
        let word_idx = frame_idx / 64;
        let bit_idx = frame_idx % 64;
        (self.bitmap[word_idx] & (1 << bit_idx)) != 0
    }

    /// Helper to convert a physical address to a virtual address pointer to a FreeBlock.
    ///
    /// # Safety
    /// Caller must ensure that HHDM is initialized.
    unsafe fn phys_to_node_ptr(phys: PhysAddr) -> *mut FreeBlock {
        let virt = phys.as_u64() + get_hhdm_offset().as_u64();
        virt as *mut FreeBlock
    }

    /// Helper to convert a virtual address pointer to a FreeBlock back to physical address.
    unsafe fn node_ptr_to_phys(ptr: NonNull<FreeBlock>) -> PhysAddr {
        let virt = ptr.as_ptr() as u64;
        PhysAddr::new(virt - get_hhdm_offset().as_u64())
    }

    // Update push_block to set bit
    unsafe fn push_block(&mut self, frame: PhysFrame<Size4KiB>, order: usize) {
        // Set bit
        let frame_idx = frame.start_address().as_u64() as usize / 4096;
        self.set_bit(frame_idx, true);

        let ptr = Self::phys_to_node_ptr(frame.start_address());
        // Write metadata
        let mut node = NonNull::new_unchecked(ptr);
        let node_ref = node.as_mut();
        node_ref.magic = FREE_BLOCK_MAGIC;
        node_ref.order = order;
        node_ref.prev = None;
        node_ref.next = self.free_lists[order];

        // Update head
        if let Some(mut head) = self.free_lists[order] {
            head.as_mut().prev = Some(node);
        }
        self.free_lists[order] = Some(node);
    }

    // Update pop_block to clear bit
    unsafe fn pop_block(&mut self, order: usize) -> Option<PhysFrame<Size4KiB>> {
        if let Some(mut head) = self.free_lists[order] {
            // Clear bit
            let phys = Self::node_ptr_to_phys(head);
            let frame_idx = phys.as_u64() as usize / 4096;
            self.set_bit(frame_idx, false);

            // Remove head
            let head_ref = head.as_mut();
            self.free_lists[order] = head_ref.next;

            if let Some(mut next) = head_ref.next {
                next.as_mut().prev = None;
            }

            // Clear pointers and magic for safety
            head_ref.next = None;
            head_ref.prev = None;
            head_ref.magic = 0;

            Some(PhysFrame::containing_address(phys))
        } else {
            None
        }
    }

    // Update remove_block to clear bit
    unsafe fn remove_block(&mut self, mut node: NonNull<FreeBlock>, order: usize) {
        // Clear bit
        let phys = Self::node_ptr_to_phys(node);
        let frame_idx = phys.as_u64() as usize / 4096;
        self.set_bit(frame_idx, false);

        let node_ref = node.as_mut();

        // Sanity check
        debug_assert_eq!(node_ref.magic, FREE_BLOCK_MAGIC);
        debug_assert_eq!(node_ref.order, order);

        // Clear magic
        node_ref.magic = 0;

        if let Some(mut prev) = node_ref.prev {
            prev.as_mut().next = node_ref.next;
        } else {
            // It was head
            self.free_lists[order] = node_ref.next;
        }

        if let Some(mut next) = node_ref.next {
            next.as_mut().prev = node_ref.prev;
        }

        node_ref.prev = None;
        node_ref.next = None;
    }

    // Update is_buddy_free to check bit
    unsafe fn is_buddy_free(&self, buddy_addr: PhysAddr, order: usize) -> bool {
        let frame_idx = buddy_addr.as_u64() as usize / 4096;
        if !self.get_bit(frame_idx) {
            return false;
        }

        // Bit is 1, so it's safe to read memory
        let ptr = Self::phys_to_node_ptr(buddy_addr);
        let node = &*ptr;
        node.magic == FREE_BLOCK_MAGIC && node.order == order
    }

    /// Add a frame to the free list, merging buddies if possible.
    unsafe fn merge_block(&mut self, frame: PhysFrame<Size4KiB>, order: usize) {
        if order >= MAX_ORDER - 1 {
            // Can't merge further
            self.push_block(frame, order);
            return;
        }

        let block_addr = frame.start_address().as_u64();
        let size = 4096 * (1 << order);
        let buddy_addr_val = block_addr ^ size;
        let buddy_addr = PhysAddr::new(buddy_addr_val);

        // Check if buddy is free
        if self.is_buddy_free(buddy_addr, order) {
            // Remove buddy from list
            let buddy_ptr = Self::phys_to_node_ptr(buddy_addr);
            self.remove_block(NonNull::new_unchecked(buddy_ptr), order);

            // Merge
            let merged_addr = PhysAddr::new(cmp::min(block_addr, buddy_addr_val));
            let merged_frame = PhysFrame::containing_address(merged_addr);

            // Recurse
            self.merge_block(merged_frame, order + 1);
        } else {
            self.push_block(frame, order);
        }
    }

    /// Add a single frame to the allocator.
    pub unsafe fn add_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        self.total_frames += 1;
        self.merge_block(frame, 0);
    }

    /// Add a range of frames to the allocator.
    pub unsafe fn add_region(&mut self, start: PhysFrame<Size4KiB>, end: PhysFrame<Size4KiB>) {
        let mut current = start;
        while current < end {
            self.add_frame(current);
            current += 1;
        }
    }

    /// Allocate a frame of the given order.
    pub fn alloc(&mut self, order: usize) -> Option<PhysFrame<Size4KiB>> {
        if order >= MAX_ORDER {
            return None;
        }

        // Try allocating from current order
        if let Some(frame) = unsafe { self.pop_block(order) } {
            self.used_frames += 1 << order;
            return Some(frame);
        }

        // Try allocating from higher order
        let block = self.alloc(order + 1)?;

        // Split block
        // block is size 2^(order+1)
        // We need size 2^order.
        let buddy_addr = block.start_address() + (4096 * (1 << order));
        let buddy_frame = PhysFrame::containing_address(buddy_addr);

        // Push buddy to free list
        unsafe { self.push_block(buddy_frame, order) };

        // Adjust usage: alloc(order+1) increased usage by 2^(order+1).
        // We only use 2^order.
        self.used_frames -= 1 << order;

        Some(block)
    }

    /// Deallocate a frame of the given order.
    /// # Safety
    /// This function is unsafe because it does not check if the frame is allocated.
    pub unsafe fn dealloc(&mut self, frame: PhysFrame<Size4KiB>, order: usize) {
        if self.used_frames >= (1 << order) {
            self.used_frames -= 1 << order;
        } else {
            // Should panic or warn?
            // Since we trust ourselves, we just floor at 0 if bug?
            self.used_frames = 0;
        }
        self.merge_block(frame, order);
    }

    /// Deallocate a range of frames.
    /// This splits the range into power-of-two blocks and deallocates them.
    /// # Safety
    /// This function is unsafe because it does not check if the frames are allocated.
    pub unsafe fn dealloc_range(&mut self, start: PhysFrame<Size4KiB>, end: PhysFrame<Size4KiB>) {
        let mut current = start;
        while current < end {
            let current_addr = current.start_address().as_u64();
            let end_addr = end.start_address().as_u64();
            let remaining_frames = (end_addr - current_addr) / 4096;

            let mut order = 0;
            while order < MAX_ORDER - 1 {
                let next_order = order + 1;
                let size_frames = 1 << next_order;
                let size_bytes = 4096 * size_frames;

                // Check alignment
                if !current_addr.is_multiple_of(size_bytes) {
                    break;
                }
                // Check size
                if size_frames > remaining_frames {
                    break;
                }
                order = next_order;
            }

            self.dealloc(current, order);
            // PhysFrame + u64 is supported
            current += 1 << order;
        }
    }

    /// Get the total number of frames managed.
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// Get the number of used frames.
    pub fn used_frames(&self) -> usize {
        self.used_frames
    }

    /// Get the number of free frames.
    pub fn free_frames(&self) -> usize {
        self.total_frames - self.used_frames
    }
}

impl<const MAX_ORDER: usize> Default for BuddyAllocator<MAX_ORDER> {
    fn default() -> Self {
        Self::new()
    }
}

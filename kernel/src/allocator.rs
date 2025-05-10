// TODO: impl spin lock

use crate::info;
use crate::spin::{self, SpinLock};
use core::alloc::GlobalAlloc;

#[derive(Debug)]
#[allow(dead_code)]
pub struct LinerAllocator {
    start: usize,
    end: usize,
    next: usize,
}

impl LinerAllocator {
    pub const fn new() -> Self {
        LinerAllocator {
            start: 0,
            end: 0,
            next: 0,
        }
    }

    /// Initialize the linear allocator with a start and end address.
    /// This function should be called once at the beginning of the program.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.next = start;
    }
}

unsafe impl GlobalAlloc for spin::SpinLock<LinerAllocator> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut allocator = self.lock();

        let current = allocator.next;
        let alloc_start = (current + layout.align() - 1) & !(layout.align() - 1);
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return core::ptr::null_mut(),
        };
        info!(
            "Allocating {} bytes at {:#018x}",
            layout.size(),
            alloc_start
        );

        if alloc_end > allocator.end {
            core::ptr::null_mut()
        } else {
            allocator.next = alloc_end;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        // Deallocation is not supported in this simple allocator
        info!("Deallocating memory at {:#018x}", ptr as usize);
    }
}

unsafe impl Sync for LinerAllocator {}

#[global_allocator]
pub static ALLOCATOR: SpinLock<LinerAllocator> = SpinLock::new(LinerAllocator::new());

pub fn init_allocator(start: usize, size: usize) {
    unsafe {
        ALLOCATOR.lock().init(start, size);
    }
}

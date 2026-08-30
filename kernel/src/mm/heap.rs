use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use crate::mm::vmm;
use crate::mm::pfa::PAGE_SIZE;

pub const HEAP_START: u64 = 0x_4444_0000_0000;
pub const HEAP_SIZE: usize = 1024 * 1024; // 1 MB heap

struct FreeBlock {
    size: usize,
    next: Option<&'static mut FreeBlock>,
}

pub struct LockedHeap {
    head: spin::Mutex<HeapInner>,
}

struct HeapInner {
    free_list: Option<&'static mut FreeBlock>,
    initialized: bool,
}

impl LockedHeap {
    pub const fn new() -> Self {
        LockedHeap {
            head: spin::Mutex::new(HeapInner {
                free_list: None,
                initialized: false,
            }),
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

impl HeapInner {
    unsafe fn init(&mut self, start: usize, size: usize) {
        self.add_free_region(start, size);
        self.initialized = true;
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        if size < core::mem::size_of::<FreeBlock>() {
            return;
        }
        let block_ptr = addr as *mut FreeBlock;
        block_ptr.write(FreeBlock {
            size,
            next: self.free_list.take(),
        });
        self.free_list = Some(&mut *block_ptr);
    }

    unsafe fn find_region(&mut self, size: usize, align: usize) -> Option<(usize, usize)> {
        let mut prev: Option<&mut FreeBlock> = None;
        let mut current = self.free_list.take();

        while let Some(block) = current {
            let block_addr = block as *mut FreeBlock as usize;
            let alloc_start = align_up(block_addr, align);
            let alloc_end = alloc_start.checked_add(size)?;
            let block_end = block_addr + block.size;

            if alloc_end <= block_end {
                let next = block.next.take();
                if let Some(p) = prev {
                    p.next = next;
                } else {
                    self.free_list = next;
                }
                return Some((alloc_start, block.size));
            } else {
                let next = block.next.take();
                block.next = None;
                let block_ref: &'static mut FreeBlock = &mut *(block as *mut FreeBlock);
                if let Some(p) = prev {
                    p.next = Some(block_ref);
                    prev = p.next.as_deref_mut();
                } else {
                    self.free_list = Some(block_ref);
                    prev = self.free_list.as_deref_mut();
                }
                current = next;
            }
        }
        None
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.head.lock();

        if !heap.initialized {
            let pages = (HEAP_SIZE as u64 + PAGE_SIZE - 1) / PAGE_SIZE;
            if vmm::map_range(HEAP_START, pages, true).is_err() {
                return null_mut();
            }
            heap.init(HEAP_START as usize, HEAP_SIZE);
        }

        let size = layout.size().max(core::mem::size_of::<FreeBlock>());
        let align = layout.align().max(core::mem::align_of::<FreeBlock>());
        let size = align_up(size, align);

        match heap.find_region(size, align) {
            Some((alloc_start, region_size)) => {
                let alloc_end = alloc_start + size;
                let excess = (alloc_start + region_size) - alloc_end;
                if excess > 0 {
                    heap.add_free_region(alloc_end, excess);
                }
                alloc_start as *mut u8
            }
            None => null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.head.lock();
        let size = layout.size().max(core::mem::size_of::<FreeBlock>());
        let align = layout.align().max(core::mem::align_of::<FreeBlock>());
        let size = align_up(size, align);
        
        heap.add_free_region(ptr as usize, size);
    }
}

#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::new();

pub fn init() {} // automatic

fn alloc_error(layout: Layout) -> ! {
    unsafe {
        let r = crate::renderer();
        use core::fmt::Write;
        r.set_color(0x00FF0000);
        let _ = write!(r, "\nHEAP DOLDU! boyut: {}\n", layout.size());
    }
    loop {}
}

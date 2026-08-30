use spin::Mutex;
use common::bootinfo::MemoryRegion;

pub const PAGE_SIZE: u64 = 4096;
pub const PAGE_SIZE_2MB: u64 = 2 * 1024 * 1024;
pub const PAGE_SIZE_1GB: u64 = 1024 * 1024 * 1024;

pub const HHDM_OFFSET: u64 = 0xFFFF_8000_0000_0000;

pub const MAX_ORDER: usize = 20; 

#[repr(C)]
pub struct FreeBlock {
    pub next: Option<*mut FreeBlock>,
}

pub struct FreeList {
    pub head: Option<*mut FreeBlock>,
}

impl FreeList {
    pub const fn new() -> Self {
        Self { head: None }
    }
}

unsafe impl Send for FreeList {}
unsafe impl Sync for FreeList {}

pub struct BuddyAllocator {
    pub free_lists: [FreeList; MAX_ORDER + 1],
    pub total_memory: u64,
    pub free_memory: u64,
}

pub static PFA: Mutex<BuddyAllocator> = Mutex::new(BuddyAllocator::new());

impl BuddyAllocator {
    pub const fn new() -> Self {
        Self {
            free_lists: [const { FreeList::new() }; MAX_ORDER + 1],
            total_memory: 0,
            free_memory: 0
        }
    }

    pub fn init_regions(&mut self, regions: *const MemoryRegion, count: usize) {
        let regions_slice = unsafe { core::slice::from_raw_parts(regions, count) };

        for region in regions_slice {
            if region.usable != 1 {
                continue;
            }

            let start_bytes = region.start;
            let end_bytes = region.start.saturating_add(region.page_count * PAGE_SIZE);

            let mut base = (start_bytes + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            let end = end_bytes & !(PAGE_SIZE - 1);
            if base < 0x100000 { 
            base = 0x100000; 
            }

            while base < end {
                let remaining_pages = (end - base) / PAGE_SIZE;
                let page_idx = base / PAGE_SIZE;

                let align_order = if page_idx == 0 {
                    MAX_ORDER
                } else {
                    page_idx.trailing_zeros() as usize
                };

                let size_order = (63 - remaining_pages.leading_zeros()) as usize;
                let order = align_order.min(size_order).min(MAX_ORDER);
                let block_size = PAGE_SIZE << order;

                self.push_to_list(base, order);
                self.total_memory += block_size;
                self.free_memory += block_size;

                base += block_size;
            }
        }
    }

    pub fn alloc_pages(&mut self, order: usize) -> Option<u64> {
        if order > MAX_ORDER { return None; }
        
        for mut current_order in order..=MAX_ORDER {
            if let Some(ptr) = self.free_lists[current_order].head {
                self.free_lists[current_order].head = unsafe { (*ptr).next };
                self.free_memory -= PAGE_SIZE << order;

                let phys_addr = (ptr as u64) - HHDM_OFFSET;

                while current_order > order {
                    current_order -= 1;
                    let buddy_addr = phys_addr + (PAGE_SIZE << current_order);
                    self.push_to_list(buddy_addr, current_order);
                }
                return Some(phys_addr);
            }
        }
        None
    }

    pub fn free_pages(&mut self, mut addr: u64, mut order: usize) {
        self.free_memory += PAGE_SIZE << order;

        while order < MAX_ORDER {
            let buddy_addr = self.buddy_of(addr, order);
            if self.remove_from_list(buddy_addr, order) {
                addr = core::cmp::min(addr, buddy_addr);
                order += 1;
            } else {
                break;
            }
        }
        self.push_to_list(addr, order);
    }

    fn buddy_of(&self, addr: u64, order: usize) -> u64 {
        let block_size = PAGE_SIZE << order;
        addr ^ block_size
    }

    fn push_to_list(&mut self, addr: u64, order: usize) {
        let ptr = (addr + HHDM_OFFSET) as *mut FreeBlock;
        unsafe {
            (*ptr).next = self.free_lists[order].head;
            self.free_lists[order].head = Some(ptr);
        };
    }

    fn remove_from_list(&mut self, addr: u64, order: usize) -> bool {
        let target_ptr = (addr + HHDM_OFFSET) as *mut FreeBlock;
        let mut current = self.free_lists[order].head;
        let mut prev: Option<*mut FreeBlock> = None;

        while let Some(curr_ptr) = current {
            if curr_ptr == target_ptr {
                unsafe {
                    if let Some(prev_ptr) = prev {
                        (*prev_ptr).next = (*curr_ptr).next;
                    } else {
                        self.free_lists[order].head = (*curr_ptr).next;
                    }
                }
                return true;
            }
            prev = Some(curr_ptr);
            unsafe {
                current = (*curr_ptr).next;
            }
        }
        false
    }
}

pub fn init(regions: *const MemoryRegion, count: usize) { PFA.lock().init_regions(regions, count); }
pub fn alloc_page() -> Option<u64> { PFA.lock().alloc_pages(0) }
pub fn alloc_2mb() -> Option<u64> { PFA.lock().alloc_pages(9) }
pub fn alloc_1gb() -> Option<u64> { PFA.lock().alloc_pages(18) }
pub fn free_page(addr: u64) { PFA.lock().free_pages(addr, 0); }
pub fn free_2mb(addr: u64) { PFA.lock().free_pages(addr, 9); }
pub fn free_1gb(addr: u64) { PFA.lock().free_pages(addr, 18); }

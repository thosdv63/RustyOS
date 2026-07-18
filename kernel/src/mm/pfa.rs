use common::bootinfo::{BootInfo};
use core::slice;

const FRAME_SIZE: u64 = 4096; // 4KB

// Global bitmap structure
pub struct FrameAllocator {
    bitmap: *mut u8,       // bitmap address (each bit is a frame)
    bitmap_size: usize,    // bitmap size (byte)
    total_frames: u64,     // total frames
    used_frames: u64,      // used
    highest_addr: u64,     
}

static mut ALLOCATOR: FrameAllocator = FrameAllocator {
    bitmap: core::ptr::null_mut(),
    bitmap_size: 0,
    total_frames: 0,
    used_frames: 0,
    highest_addr: 0,
};

// Bitmap helpers
// frame number -> fill mark on bitmap
unsafe fn set_used(frame: u64) {
    let byte = (frame / 8) as usize;
    let bit = (frame % 8) as u8;
    *ALLOCATOR.bitmap.add(byte) |= 1 << bit;
}

// frame number -> mark as empty on bitmap
unsafe fn set_free(frame: u64) {
    let byte = (frame / 8) as usize;
    let bit = (frame % 8) as u8;
    *ALLOCATOR.bitmap.add(byte) &= !(1 << bit);
}

// is frame full?
unsafe fn is_used(frame: u64) -> bool {
    let byte = (frame / 8) as usize;
    let bit = (frame % 8) as u8;
    (*ALLOCATOR.bitmap.add(byte) >> bit) & 1 == 1
}

pub fn init(boot_info: *const BootInfo) {
    unsafe {
        let r = crate::renderer();
        use core::fmt::Write;

        let info = &*boot_info;

        if info.memory_region_count == 0 || info.memory_region_count > 1024 {
            let _ = write!(r, "[PFA] Incorrect region count! Stopped.\n");
            return;
        }

        let regions = slice::from_raw_parts(
            info.memory_regions,
            info.memory_region_count as usize,
        );

        const MAX_RAM: u64 = 0x1_0000_0000; // 4 GB

        let mut highest: u64 = 0;
        for rg in regions {
            let end = rg.start + rg.page_count * FRAME_SIZE;
            // Skip areas with very high address values ​​(not actual RAM)
            if rg.start >= MAX_RAM { continue; }
            if end > highest && end <= MAX_RAM { highest = end; }
        }

        ALLOCATOR.highest_addr = highest;
        ALLOCATOR.total_frames = highest / FRAME_SIZE;

        let bitmap_size = (ALLOCATOR.total_frames as usize + 7) / 8;
        ALLOCATOR.bitmap_size = bitmap_size;

        // Find the LARGEST available area and place the bitmap there
        let bitmap_frames_needed = (bitmap_size as u64 + FRAME_SIZE - 1) / FRAME_SIZE;
        let mut bitmap_addr: u64 = 0;
        let mut best_size: u64 = 0;
        for rg in regions {
            // Under MAX RAM, it's available, and the biggest ever
            if rg.usable == 1 && rg.start < MAX_RAM && rg.page_count > best_size {
                if rg.page_count >= bitmap_frames_needed {
                    best_size = rg.page_count;
                    bitmap_addr = rg.start;
                }
            }
        }

        if bitmap_addr == 0 {
            let _ = write!(r, "[PFA] No space found for bitmap! Stopped.\n");
            return;
        }
        ALLOCATOR.bitmap = bitmap_addr as *mut u8;

        for i in 0..bitmap_size {
            *ALLOCATOR.bitmap.add(i) = 0xFF;
        }
        ALLOCATOR.used_frames = ALLOCATOR.total_frames;

        let mut freed: u64 = 0;
        for rg in regions {
            if rg.usable == 1 && rg.start < MAX_RAM {
                let start_frame = rg.start / FRAME_SIZE;
                for f in 0..rg.page_count {
                    let frame = start_frame + f;
                    if frame >= ALLOCATOR.total_frames { break; }
                    if is_used(frame) {
                        set_free(frame);
                        freed += 1;
                    }
                }
            }
        }
        ALLOCATOR.used_frames -= freed;

        let bitmap_start_frame = bitmap_addr / FRAME_SIZE;
        for f in 0..bitmap_frames_needed {
            if !is_used(bitmap_start_frame + f) {
                set_used(bitmap_start_frame + f);
                ALLOCATOR.used_frames += 1;
            }
        }
        if !is_used(0) {
            set_used(0);
            ALLOCATOR.used_frames += 1;
        }

        // CRITICAL: Reserve the userland/CORE.BIN region (0x0 - 0x500000)
        // If DMA takes frames from this region, it will corrupt the running userland (freeze!)
        let reserve_end = 0x500000u64 / FRAME_SIZE; // frame 1280
        for frame in 0..reserve_end {
            if frame >= ALLOCATOR.total_frames { break; }
            if !is_used(frame) {
                set_used(frame);
                ALLOCATOR.used_frames += 1;
            }
        }
        // fallback audio region (0x0100_0000 - 0x0110_0000, 1MB) reserved
        let snd_start = 0x0100_0000u64 / FRAME_SIZE;
        let snd_end   = 0x0110_0000u64 / FRAME_SIZE;
        for frame in snd_start..snd_end {
            if frame >= ALLOCATOR.total_frames { break; }
            if !is_used(frame) {
                set_used(frame);
                ALLOCATOR.used_frames += 1;
            }
        }
    }
}

// Userland code/stack + CORE.BIN + fallback regions: DMA should NEVER fall here.
// (between 0x000000 - 0x500000 = frame 0..1280 reserved)
const DMA_SAFE_START_FRAME: u64 = 0x500000 / FRAME_SIZE; // = 1280

pub fn alloc_frame() -> Option<u64> {
    unsafe {
        let start = DMA_SAFE_START_FRAME;
        for frame in start..ALLOCATOR.total_frames {
            if !is_used(frame) {
                set_used(frame);
                ALLOCATOR.used_frames += 1;
                return Some(frame * FRAME_SIZE);
            }
        }
        None
    }
}

pub fn free_frame(addr: u64) {
    unsafe {
        let frame = addr / FRAME_SIZE;
        if is_used(frame) {
            set_free(frame);
            ALLOCATOR.used_frames -= 1;
        }
    }
}

use x86_64::structures::paging::{FrameAllocator as X86FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

pub struct PfaWrapper;

unsafe impl X86FrameAllocator<Size4KiB> for PfaWrapper {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        alloc_frame().map(|addr| {
            PhysFrame::containing_address(PhysAddr::new(addr))
        })
    }
}
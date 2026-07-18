#[repr(C)]
pub struct Framebuffer {
    pub base: *mut u8,
    pub width: u64,
    pub height: u64,
    pub stride: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryRegion {
    pub start: u64,
    pub page_count: u64,
    pub usable: u64,
}

#[repr(C)]
pub struct BootInfo {
    pub framebuffer: Framebuffer,
    pub memory_regions: *const MemoryRegion,
    pub memory_region_count: u64,
    pub rsdp_addr: u64,
    pub rpe_mode: u64,
}
pub mod pfa;
pub mod vmm;   
pub mod heap;  

use common::bootinfo::BootInfo;

pub fn init(boot_info: *const BootInfo) {
    unsafe {
        let info = &*boot_info;
        pfa::init(info.memory_regions, info.memory_region_count as usize);
    }
    vmm::init();
    heap::init();
}

pub mod pfa;
pub mod ptm;   
pub mod heap;  

use common::bootinfo::BootInfo;

// Tum bellek yonetimini baslat
pub fn init(boot_info: *const BootInfo) {
    pfa::init(boot_info);
    ptm::init();
    heap::init();
}


use core::ptr::{read_volatile, write_volatile};

// LAPIC Register Offsets
const LAPIC_ID: usize = 0x0020;
const LAPIC_EOI: usize = 0x00B0;
const LAPIC_SPURIOUS: usize = 0x00F0;
const LAPIC_TIMER: usize = 0x0320;
const LAPIC_TIMER_DIV: usize = 0x03E0;
const LAPIC_TIMER_INIT: usize = 0x0380;
const LAPIC_TIMER_CURRENT: usize = 0x0390;

pub struct Lapic {
    base_addr: usize,
}

impl Lapic {
    pub fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    unsafe fn write(&self, reg: usize, value: u32) {
        let ptr = (self.base_addr + reg) as *mut u32;
        write_volatile(ptr, value);
    }

    unsafe fn read(&self, reg: usize) -> u32 {
        let ptr = (self.base_addr + reg) as *const u32;
        read_volatile(ptr)
    }

    pub unsafe fn enable(&self) {
        // Install the Spurious Vector Register (Bit 8: APIC Enable, 0xFF: Vector No)
        let val = self.read(LAPIC_SPURIOUS);
        self.write(LAPIC_SPURIOUS, val | 0x100 | 0xFF);
    }

    pub unsafe fn send_eoi(&self) {
        self.write(LAPIC_EOI, 0);
    }

    pub unsafe fn init_timer(&self, initial_count: u32) {
        // Prepare Timer Divide Configuration Register (Divider = 16)
        // Bits 0, 1, and 3 are used to set the value. 0x03 = Divide by 16.
        self.write(LAPIC_TIMER_DIV, 0x03);

        // Prepare LVT Timer Register
        // Bit 17: Timer Mode (0 = One-shot, 1 = Periodic, 2 = TSC-Deadline)
        // Bits 0-7: Interrupt Vector (e.g., 0x20)
        // 0x20000 = Periodic Mode, 0x20 = Vector Number
        self.write(LAPIC_TIMER, 0x20020); 

        // Start the counter by typing Initial Count Register
        self.write(LAPIC_TIMER_INIT, initial_count);
    }

}
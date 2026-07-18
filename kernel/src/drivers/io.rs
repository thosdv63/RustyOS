use x86_64::instructions::port::Port;

// Port I/O (old hardware)
pub unsafe fn inb(port: u16) -> u8 { let mut p = Port::new(port); p.read() }
pub unsafe fn outb(port: u16, val: u8) { let mut p = Port::new(port); p.write(val); }
pub unsafe fn inw(port: u16) -> u16 { let mut p = Port::new(port); p.read() }
pub unsafe fn outw(port: u16, val: u16) { let mut p = Port::new(port); p.write(val); }
pub unsafe fn inl(port: u16) -> u32 { let mut p = Port::new(port); p.read() }
pub unsafe fn outl(port: u16, val: u32) { let mut p = Port::new(port); p.write(val); }

// MMIO
pub unsafe fn mmio_read8(addr: u64) -> u8 {
    core::ptr::read_volatile(addr as *const u8)
}
pub unsafe fn mmio_write8(addr: u64, val: u8) {
    core::ptr::write_volatile(addr as *mut u8, val);
}
pub unsafe fn mmio_read16(addr: u64) -> u16 {
    core::ptr::read_volatile(addr as *const u16)
}
pub unsafe fn mmio_write16(addr: u64, val: u16) {
    core::ptr::write_volatile(addr as *mut u16, val);
}
pub unsafe fn mmio_read32(addr: u64) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}
pub unsafe fn mmio_write32(addr: u64, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val);
}
pub unsafe fn mmio_read64(addr: u64) -> u64 {
    core::ptr::read_volatile(addr as *const u64)
}
pub unsafe fn mmio_write64(addr: u64, val: u64) {
    core::ptr::write_volatile(addr as *mut u64, val);
}
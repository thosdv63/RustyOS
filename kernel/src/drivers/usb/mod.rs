pub mod xhci;
pub mod hid;
pub mod storage;

pub const PROTO_KEYBOARD: u8 = 1;
pub const PROTO_MOUSE: u8 = 2;

pub const CLS_HID: u8 = 3;
pub const CLS_MSC: u8 = 8;

pub fn udelay(us: u64) {
    let mut p = x86_64::instructions::port::Port::<u8>::new(0x80);
    for _ in 0..us {
        unsafe { p.write(0u8); }
    }
}

pub fn mdelay(ms: u64) { udelay(ms * 1000); }
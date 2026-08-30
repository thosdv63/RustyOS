use alloc::vec::Vec;
use crate::drivers::pci::PciDevice;
use x86_64::instructions::port::Port;
use super::Aligned;

const PO_BDBAR: u16 = 0x10;
const PO_LVI:   u16 = 0x15;
const PO_SR:    u16 = 0x16;
const PO_CR:    u16 = 0x1B;
const GLOB_CNT: u16 = 0x2C;

const RESET:       u16 = 0x00;
const MASTER_VOL:  u16 = 0x02;
const PCM_OUT_VOL: u16 = 0x18;

static mut BDL: Aligned<256> = Aligned([0; 256]);

fn bdl_addr() -> u32 { unsafe { #[allow(static_mut_refs)] BDL.0.as_ptr() as u32 } }

pub struct Ac97 { nam: u16, nabm: u16 }

impl Ac97 {
    unsafe fn nam_w16(&self, o: u16, v: u16) { Port::<u16>::new(self.nam + o).write(v); }
    unsafe fn nabm_w8(&self, o: u16, v: u8)  { Port::<u8>::new(self.nabm + o).write(v); }
    unsafe fn nabm_w16(&self, o: u16, v: u16){ Port::<u16>::new(self.nabm + o).write(v); }
    unsafe fn nabm_w32(&self, o: u16, v: u32){ Port::<u32>::new(self.nabm + o).write(v); }

    pub fn init(&self) {
        unsafe {
            self.nabm_w32(GLOB_CNT, 0x0000_0002);
            for _ in 0..100_000 { core::arch::asm!("nop"); }
            self.nam_w16(RESET, 1);
            for _ in 0..100_000 { core::arch::asm!("nop"); }
            self.nam_w16(MASTER_VOL, 0x0000);
            self.nam_w16(PCM_OUT_VOL, 0x0000);
        }
    }

    pub fn play_pcm(&mut self, phys: u32, len: usize) {
        unsafe {
            self.stop();
            let len = len & !0x1;
            if len == 0 { return; }

            let total_samples = len / 2;
            let max_per_desc = 0xFFFEusize;
            let mut remaining = total_samples;
            let mut off = 0usize;
            let mut n = 0usize;

            #[allow(static_mut_refs)]
            let b = BDL.0.as_mut_ptr();

            while remaining > 0 && n < 32 {
                let chunk = remaining.min(max_per_desc);
                let e = b.add(n * 8);
                core::ptr::write_unaligned(e as *mut u32, phys + (off * 2) as u32);
                core::ptr::write_unaligned(e.add(4) as *mut u16, chunk as u16);
                core::ptr::write_unaligned(e.add(6) as *mut u16, 0x8000u16);
                off += chunk; remaining -= chunk; n += 1;
            }
            if n == 0 { return; }

            self.nabm_w8(PO_CR, 0x02);
            for _ in 0..50_000 { core::arch::asm!("nop"); }
            self.nabm_w32(PO_BDBAR, bdl_addr());
            self.nabm_w8(PO_LVI, (n - 1) as u8);
            self.nabm_w8(PO_CR, 0x01);
        }
    }

    pub fn stop(&mut self) {
        unsafe { self.nabm_w8(PO_CR, 0x00); self.nabm_w16(PO_SR, 0x1C); }
    }
}

pub fn init(devices: &Vec<PciDevice>) -> Option<Ac97> {
    for d in devices {
        if d.class == 0x04 && d.subclass == 0x01 {
            crate::drivers::pci::enable_bus_master(d.bus, d.device, d.function);
            let bar0 = crate::drivers::pci::read_bar(d.bus, d.device, d.function, 0);
            let bar1 = crate::drivers::pci::read_bar(d.bus, d.device, d.function, 1);
            let a = Ac97 { nam: (bar0 & !0x3) as u16, nabm: (bar1 & !0x3) as u16 };
            a.init();
            return Some(a);
        }
    }
    None
}

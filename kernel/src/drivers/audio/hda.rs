use alloc::vec::Vec;
use alloc::format;
use crate::drivers::pci::PciDevice;
use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{fence, Ordering};
use x86_64::instructions::port::Port;

use super::{dbg, Aligned};

// Global controller registers
const GCAP:     u64 = 0x00; // u16
const GCTL:     u64 = 0x08; // u32
const STATESTS: u64 = 0x0E; // u16
const INTCTL:   u64 = 0x20; // u32

// Immediate Command Interface
const ICW: u64 = 0x60; 
const IRR: u64 = 0x64;
const ICS: u64 = 0x68;

const ICS_BUSY:  u16 = 0x0001;
const ICS_VALID: u16 = 0x0002;

// Stream descriptor (to sd base)
const SD_CTL:  u64 = 0x00; // 3 byte
const SD_STS:  u64 = 0x03; // u8
const SD_LPIB: u64 = 0x04; // u32
const SD_CBL:  u64 = 0x08; // u32
const SD_LVI:  u64 = 0x0C; // u16
const SD_FMT:  u64 = 0x12; // u16
const SD_BDPL: u64 = 0x18; // u32
const SD_BDPU: u64 = 0x1C; // u32

const BDL_N: usize = 256;
const MAX_DESC: usize = 0x1_0000; // A BDL entry can have a maximum of 64KB
const STREAM_NR: u32 = 1;

// 48kHz / 16-bit / stereo. If the sound is high-pitched/fast, set it to 0x4011 (44.1kHz).
const FMT: u16 = 0x0011;

// BDL: kernel static (identity map -> virt == phys, xHCI/AC97 gibi)
static mut BDL: Aligned<{ BDL_N * 16 }> = Aligned([0; BDL_N * 16]);

fn bdl_addr() -> u64 {
    unsafe {
        #[allow(static_mut_refs)]
        BDL.0.as_ptr() as u64
    }
}

// reading, writing functions
unsafe fn r8(a: u64) -> u8    { read_volatile(a as *const u8) }
unsafe fn w8(a: u64, v: u8)   { write_volatile(a as *mut u8, v) }
unsafe fn r16(a: u64) -> u16  { read_volatile(a as *const u16) }
unsafe fn w16(a: u64, v: u16) { write_volatile(a as *mut u16, v) }
unsafe fn r32(a: u64) -> u32  { read_volatile(a as *const u32) }
unsafe fn w32(a: u64, v: u32) { write_volatile(a as *mut u32, v) }

fn udelay(us: u64) {
    let mut p = Port::<u8>::new(0x80);
    for _ in 0..us { unsafe { p.write(0u8); } }
}

// PCI config: for verifying/logging the command register only.
fn cfg_addr(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC)
}
fn cfg_r32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    unsafe {
        Port::<u32>::new(0xCF8).write(cfg_addr(bus, dev, func, off));
        Port::<u32>::new(0xCFC).read()
    }
}
fn cfg_w32(bus: u8, dev: u8, func: u8, off: u8, v: u32) {
    unsafe {
        Port::<u32>::new(0xCF8).write(cfg_addr(bus, dev, func, off));
        Port::<u32>::new(0xCFC).write(v);
    }
}

// 12-bit verb + 8-bit payload
fn verb12(cad: u32, nid: u32, verb: u32, pl: u32) -> u32 {
    (cad << 28) | (nid << 20) | ((verb & 0xFFF) << 8) | (pl & 0xFF)
}
// 4-bit verb + 16-bit payload
fn verb4(cad: u32, nid: u32, verb: u32, pl: u32) -> u32 {
    (cad << 28) | (nid << 20) | ((verb & 0xF) << 16) | (pl & 0xFFFF)
}

// HDA info struct
pub struct Hda {
    mmio: u64,
    sd: u64,
    cad: u32,
    dac: u32,
    pin: u32,
    playing: bool,
    total: u32,
    prev: u32,
}

impl Hda {
    // ==== Immediate Command Interface: DMA-free codec command ====
    fn cmd(&mut self, c: u32) -> u32 {
        unsafe {
            // Clear previous VALID flag
            let mut g = 0u32;
            while r16(self.mmio + ICS) & ICS_BUSY != 0 {
                udelay(1); g += 1;
                if g > 20_000 { dbg("[HDA] ICI previous command stuck\n"); return 0; }
            }
            w16(self.mmio + ICS, ICS_VALID); // Clear VALID (RW1C)

            w32(self.mmio + ICW, c);
            w16(self.mmio + ICS, ICS_BUSY); // send command

            g = 0;
            loop {
                let s = r16(self.mmio + ICS);
                if (s & ICS_BUSY) == 0 && (s & ICS_VALID) != 0 { break; }
                udelay(1); g += 1;
                if g > 50_000 { dbg("[HDA] ICI timeout\n"); return 0; }
            }
            let resp = r32(self.mmio + IRR);
            w16(self.mmio + ICS, ICS_VALID); // clear VALID
            resp
        }
    }

    fn param(&mut self, nid: u32, p: u32) -> u32 {
        let cad = self.cad;
        self.cmd(verb12(cad, nid, 0xF00, p))
    }

    unsafe fn controller_reset(&mut self) -> bool {
        if r32(self.mmio + GCTL) == 0xFFFF_FFFF {
            dbg("[HDA] MMIO is unreadable\n");
            return false;
        }
        w32(self.mmio + INTCTL, 0);

        // Put it in reset mode
        w32(self.mmio + GCTL, 0);
        let mut g = 0;
        while (r32(self.mmio + GCTL) & 1) != 0 {
            udelay(10); g += 1;
            if g > 5_000 { dbg("[HDA] couldn't enter the reset menu.\n"); return false; }
        }
        udelay(500);

        // Exit from Reset
        w32(self.mmio + GCTL, 1);
        g = 0;
        while (r32(self.mmio + GCTL) & 1) == 0 {
            udelay(10); g += 1;
            if g > 5_000 { dbg("[HDA] couldn't exit the reset\n"); return false; }
        }
        udelay(1500); // Codec enumeration >= 521us

        let st = r16(self.mmio + STATESTS);
        if st == 0 || st == 0xFFFF { dbg("[HDA] no codec\n"); return false; }
        for i in 0..15 {
            if st & (1 << i) != 0 { self.cad = i as u32; break; }
        }

        // Start ICI clean
        w16(self.mmio + ICS, ICS_VALID);

        dbg(&format!("[HDA] reset ok, codec={}\n", self.cad));
        true
    }

    fn codec_setup(&mut self) -> bool {
        let cad = self.cad;

        // Authentication: If vendor/device ID returns zero, ICI is not working
        let vid = self.param(0, 0x00);
        dbg(&format!("[HDA] codec vid=0x{:08X}\n", vid));
        if vid == 0 || vid == 0xFFFF_FFFF { dbg("[HDA] ICI is not responding\n"); return false; }

        let sub = self.param(0, 0x04);
        let fg_start = (sub >> 16) & 0xFF;
        let fg_count = sub & 0xFF;
        if fg_count == 0 || fg_count > 32 { dbg("[HDA] no FG\n"); return false; }

        let mut dac: Option<u32> = None;
        let mut pin: Option<u32> = None;

        for fg in fg_start..fg_start + fg_count {
            if self.param(fg, 0x05) & 0xFF != 0x01 { continue; } // Audio Function Group
            self.cmd(verb12(cad, fg, 0x705, 0x00)); // power state D0
            udelay(1000);

            let sub2 = self.param(fg, 0x04);
            let ws = (sub2 >> 16) & 0xFF;
            let wc = sub2 & 0xFF;
            if wc == 0 || wc > 64 { continue; }

            for nid in ws..ws + wc {
                let caps = self.param(nid, 0x09);
                let wt = (caps >> 20) & 0xF;
                if wt == 0x0 && dac.is_none() {
                    dac = Some(nid); // Audio Output (DAC)
                } else if wt == 0x4 && pin.is_none() {
                    // Pin: is output capable?
                    if self.param(nid, 0x0C) & (1 << 4) != 0 {
                        let cfg = self.cmd(verb12(cad, nid, 0xF1C, 0));
                        // port connectivity == 1 => no physical connection
                        if (cfg >> 30) & 0x3 != 1 { pin = Some(nid); }
                    }
                }
            }
            if dac.is_some() && pin.is_some() { break; }
        }

        let (dac, pin) = match (dac, pin) {
            (Some(d), Some(p)) => (d, p),
            _ => { dbg("[HDA] no DAC/PIN\n"); return false; }
        };
        self.dac = dac;
        self.pin = pin;

        // Power
        self.cmd(verb12(cad, dac, 0x705, 0x00));
        self.cmd(verb12(cad, pin, 0x705, 0x00));
        udelay(1000);

        // Pin: OUT + HP enable, EAPD
        self.cmd(verb12(cad, pin, 0x707, 0xC0));
        self.cmd(verb12(cad, pin, 0x70C, 0x02));

        // If the pin has a connection list, select the first entry
        if self.param(pin, 0x0E) & 0x7F > 0 {
            self.cmd(verb12(cad, pin, 0x701, 0x00));
        }

        // Unmute the amps + max gain
        let dc = self.param(dac, 0x12);
        if dc != 0 {
            let gain = (dc >> 8) & 0x7F;
            self.cmd(verb4(cad, dac, 0x3, 0xB000 | gain));
        }
        let pc = self.param(pin, 0x12);
        if pc != 0 {
            let gain = (pc >> 8) & 0x7F;
            self.cmd(verb4(cad, pin, 0x3, 0xB000 | gain));
        }

        // Format + stream/channel assignment
        self.cmd(verb4(cad, dac, 0x2, FMT as u32));
        self.cmd(verb12(cad, dac, 0x706, (STREAM_NR << 4) as u32));

        dbg(&format!("[HDA] dac={} pin={} hazir\n", dac, pin));
        true
    }

    unsafe fn stream_reset(&self) {
        w8(self.sd + SD_CTL, 0x00);
        let mut g = 0;
        while r8(self.sd + SD_CTL) & 0x02 != 0 { udelay(1); g += 1; if g > 5000 { break; } }

        w8(self.sd + SD_CTL, 0x01); // SRST
        g = 0;
        while r8(self.sd + SD_CTL) & 0x01 == 0 { udelay(1); g += 1; if g > 5000 { break; } }
        udelay(100);
        w8(self.sd + SD_CTL, 0x00);
        g = 0;
        while r8(self.sd + SD_CTL) & 0x01 != 0 { udelay(1); g += 1; if g > 5000 { break; } }
        udelay(100);
        w8(self.sd + SD_STS, 0x1C);
    }

    // phys: PCM buffer physical address, total: 128 bytes aligned total
    pub fn play_pcm(&mut self, phys: u64, total: usize) {
        if total == 0 { return; }
        unsafe {
            self.stop();

            let bdl = bdl_addr() as *mut u32;
            let mut off = 0usize;
            let mut n = 0usize;
            while off < total && n < BDL_N {
                let chunk = (total - off).min(MAX_DESC);
                let a = phys + off as u64;
                write_volatile(bdl.add(n * 4 + 0), (a & 0xFFFF_FFFF) as u32);
                write_volatile(bdl.add(n * 4 + 1), (a >> 32) as u32);
                write_volatile(bdl.add(n * 4 + 2), chunk as u32);
                write_volatile(bdl.add(n * 4 + 3), 0u32); // IOC closed 
                off += chunk;
                n += 1;
            }
            // Spec requires at least 2 entries
            if n < 2 {
                let mut h = total / 2;
                h &= !127;
                if h == 0 { h = total; }
                write_volatile(bdl.add(0), (phys & 0xFFFF_FFFF) as u32);
                write_volatile(bdl.add(1), (phys >> 32) as u32);
                write_volatile(bdl.add(2), h as u32);
                write_volatile(bdl.add(3), 0u32);
                let a2 = phys + h as u64;
                write_volatile(bdl.add(4), (a2 & 0xFFFF_FFFF) as u32);
                write_volatile(bdl.add(5), (a2 >> 32) as u32);
                write_volatile(bdl.add(6), (total - h) as u32);
                write_volatile(bdl.add(7), 0u32);
                n = 2;
            }
            fence(Ordering::SeqCst);

            self.stream_reset();

            let ba = bdl_addr();
            w32(self.sd + SD_CBL, total as u32);
            w16(self.sd + SD_LVI, (n - 1) as u16);
            w16(self.sd + SD_FMT, FMT);
            w32(self.sd + SD_BDPL, (ba & 0xFFFF_FFFF) as u32);
            w32(self.sd + SD_BDPU, (ba >> 32) as u32);
            // stream number: SDCTL bit 20-23 => byte offset +2, upper nibble
            w8(self.sd + SD_CTL + 2, (STREAM_NR as u8) << 4);

            fence(Ordering::SeqCst);
            w8(self.sd + SD_CTL, 0x02); // RUN

            self.total = total as u32;
            self.prev = 0;
            self.playing = true;
        }
    }

    pub fn stop(&mut self) {
        unsafe {
            w8(self.sd + SD_CTL, 0x00);
            w8(self.sd + SD_STS, 0x1C);
        }
        self.playing = false;
        self.total = 0;
    }

    // End detection: Stop if LPIB starts over or nears the end
    pub fn tick(&mut self) {
        if !self.playing || self.total == 0 { return; }
        let lpib = unsafe { r32(self.sd + SD_LPIB) };

        // It's rewound => one round is over
        if lpib < self.prev {
            self.stop();
            return;
        }
        // We're nearing the end (we're in the midst of TAIL silence)
        if lpib + 8192 >= self.total {
            self.stop();
            return;
        }
        self.prev = lpib;
    }

    pub fn is_playing(&self) -> bool { self.playing }
}

pub fn init(devices: &Vec<PciDevice>) -> Option<Hda> {
    for d in devices {
        // class 0x04 (multimedia), subclass 0x03 (HD Audio)
        if d.class == 0x04 && d.subclass == 0x03 {
            // important! Bus master. Same proven function as AC97/xHCI.
            crate::drivers::pci::enable_bus_master(d.bus, d.device, d.function);

            let (b, dv, f) = (d.bus as u8, d.device as u8, d.function as u8);
            // Additional assurance: Also set the IO + MEM + BusMaster bits directly
            let cmd_before = cfg_r32(b, dv, f, 0x04);
            cfg_w32(b, dv, f, 0x04, (cmd_before & 0xFFFF) | 0x0007);
            let cmd_after = cfg_r32(b, dv, f, 0x04) & 0xFFFF;
            dbg(&format!("[HDA] PCI cmd 0x{:04X} -> 0x{:04X}\n", cmd_before & 0xFFFF, cmd_after));
            if cmd_after & 0x04 == 0 { dbg("[HDA] warning: bus master couldn't be open!\n"); }

            let bar0 = cfg_r32(b, dv, f, 0x10);
            if bar0 & 1 != 0 { dbg("[HDA] BAR0 is IO type, skipped.\n"); return None; }
            let mmio = (bar0 & 0xFFFF_FFF0) as u64;
            if mmio == 0 { dbg("[HDA] BAR0 is empty\n"); return None; }

            dbg(&format!("[HDA] mmio=0x{:X} bdl=0x{:X}\n", mmio, bdl_addr()));

            let mut ptm = crate::mm::vmm::PageTableManager::active();
            for i in 0..8u64 {
                let a = mmio + i * 0x1000;
                if ptm.translate(a).is_none() {
                    // MMIO map'i: (virt, phys, size, writable, user, disable_cache, execute)
                    ptm.map(a, a, 4096, true, false, true, false);
                }
            }

            let mut h = Hda {
                mmio, sd: 0, cad: 0, dac: 0, pin: 0,
                playing: false, total: 0, prev: 0,
            };

            unsafe {
                if !h.controller_reset() { return None; }
                let gcap = r16(mmio + GCAP);
                let iss = ((gcap >> 8) & 0xF) as u64;
                let oss = ((gcap >> 12) & 0xF) as u64;
                dbg(&format!("[HDA] gcap=0x{:X} iss={} oss={}\n", gcap, iss, oss));
                if oss == 0 { return None; }
                // stream descriptors
                h.sd = mmio + 0x80 + iss * 0x20;
            }

            if !h.codec_setup() { return None; }
            return Some(h);
        }
    }
    None
}

pub mod ac97;
pub mod hda;

use alloc::vec::Vec;
use alloc::format;
use crate::drivers::pci::PciDevice;
use x86_64::instructions::port::Port;
use core::sync::atomic::{AtomicBool, Ordering};

static AUDIO_BUSY: AtomicBool = AtomicBool::new(false);

#[repr(align(128))]
pub struct Aligned<const N: usize>(pub [u8; N]);

// PCM BUFFER
static mut PCM_VIRT: u64 = 0;
static mut PCM_PHYS: u64 = 0;
static mut PCM_CAP: usize = 0;

const PCM_VIRT_BASE: u64 = 0x_4448_0000_0000;
const PCM_PAGES: u64 = 2048; // 8MB of maple leaves guaranteeing a run BEYOND the restricted area
const TAIL: usize = 16384;

const FALLBACK_PHYS: u64 = 0x0100_0000;
const FALLBACK_LEN: usize = 0x0010_0000;

// FORBIDDEN PHYSICAL AREAS
const DENY: [(u64, u64); 2] = [
    (0x0000_0000, 0x0042_0000),
    (0x0100_0000, 0x0110_0000),
];

fn run_ok(pstart: u64, pend: u64) -> bool {
    for &(ds, de) in DENY.iter() {
        if pstart < de && pend > ds { return false; } // Intersection exists -> forbidden
    }
    true
}

trait ToPhys { fn phys(self) -> u64; }
impl ToPhys for u64 { fn phys(self) -> u64 { self } }
impl ToPhys for x86_64::PhysAddr { fn phys(self) -> u64 { self.as_u64() } }
impl ToPhys for (u64, u64) { fn phys(self) -> u64 { self.0 } }

#[inline]
fn virt_to_phys(v: u64) -> Option<u64> {
    crate::mm::vmm::PageTableManager::active().translate(v)
}

fn init_pcm_buffer() {
    if crate::mm::vmm::map_range(PCM_VIRT_BASE, PCM_PAGES, true).is_err() {
        unsafe { PCM_VIRT = FALLBACK_PHYS; PCM_PHYS = FALLBACK_PHYS; PCM_CAP = FALLBACK_LEN; }
        dbg("[AUDIO] UYARI: map_range is not succesfull -> fallback\n");
        return;
    }

    // add: (virt_idx, phys_start, sayfa_sayisi)
    let mut best_s: u64 = 0; let mut best_p: u64 = 0; let mut best_n: u64 = 0;
    let mut run_s: u64 = 0;  let mut run_p: u64 = 0;  let mut run_n: u64 = 0;
    let mut prev: u64 = 0;

    let consider = |s: u64, p: u64, n: u64, bs: &mut u64, bp: &mut u64, bn: &mut u64| {
        if n == 0 { return; }
        let pend = p + n * 0x1000;
        if run_ok(p, pend) && n > *bn { *bs = s; *bp = p; *bn = n; }
    };

    for i in 0..PCM_PAGES {
        let v = PCM_VIRT_BASE + i * 0x1000;
        let p = match virt_to_phys(v) {
            Some(p) => p & !0xFFF,
            None => {
                consider(run_s, run_p, run_n, &mut best_s, &mut best_p, &mut best_n);
                run_n = 0; continue;
            }
        };
        if run_n > 0 && p == prev + 0x1000 {
            run_n += 1;
        } else {
            consider(run_s, run_p, run_n, &mut best_s, &mut best_p, &mut best_n);
            run_s = i; run_p = p; run_n = 1;
        }
        prev = p;
    }
    consider(run_s, run_p, run_n, &mut best_s, &mut best_p, &mut best_n);

    if best_n >= 64 { // minimum 256 kb
        let v = PCM_VIRT_BASE + best_s * 0x1000;
        unsafe { PCM_VIRT = v; PCM_PHYS = best_p; PCM_CAP = (best_n as usize) * 4096; }
        dbg(&format!("[AUDIO] pcm: virt=0x{:X} phys=0x{:X} cap={}KB (veto-clean)\n",
            v, best_p, best_n * 4));
        return;
    }

    unsafe { PCM_VIRT = FALLBACK_PHYS; PCM_PHYS = FALLBACK_PHYS; PCM_CAP = FALLBACK_LEN; }
    dbg("[AUDIO] warning: no veto-clean -> fallback\n");
}

pub fn pcm_ptr() -> *mut u8 { unsafe { PCM_VIRT as *mut u8 } }
pub fn pcm_phys() -> u64 { unsafe { PCM_PHYS } }
pub fn pcm_cap() -> usize { unsafe { PCM_CAP } }

pub fn pcm_buf() -> &'static mut [u8] {
    unsafe {
        if PCM_VIRT == 0 { return &mut []; }
        core::slice::from_raw_parts_mut(PCM_VIRT as *mut u8, PCM_CAP)
    }
}

pub fn dbg(s: &str) {
    unsafe {
        let mut p = Port::<u8>::new(0x3F8);
        for b in s.bytes() { p.write(b); }
    }
}

pub enum AudioDev {
    None,
    Hda(hda::Hda),
    Ac97(ac97::Ac97),
}

static mut AUDIO: AudioDev = AudioDev::None;

pub fn init(devices: &Vec<PciDevice>) {
    dbg("[AUDIO] init v4 (fiziksel-veto)\n"); // SIGNATURE
    init_pcm_buffer();

    if let Some(h) = hda::init(devices) {
        dbg("[AUDIO] Intel HDA selected\n");
        unsafe { AUDIO = AudioDev::Hda(h); }
        return;
    }
    dbg("[AUDIO] HDA missing/failed, trying AC97\n");
    if let Some(a) = ac97::init(devices) {
        dbg("[AUDIO] AC97 secildi\n");
        unsafe { AUDIO = AudioDev::Ac97(a); }
        return;
    }
    dbg("[AUDIO] no sound device\n");
    unsafe { AUDIO = AudioDev::None; }
}

pub fn present() -> bool {
    unsafe {
        #[allow(static_mut_refs)]
        !matches!(AUDIO, AudioDev::None)
    }
}

pub fn play_pcm(len: usize) {
    let cap = pcm_cap();
    if cap == 0 { return; }

    let len = len.min(cap) & !0x3;
    if len == 0 { return; }

    let mut total = len + TAIL;
    total = (total + 127) & !127;
    if total > cap { total = cap & !127; }
    if total < 4096 { total = 4096; }
    if total > len {
        unsafe { core::ptr::write_bytes(pcm_ptr().add(len), 0, total - len); }
    }
    let len = len.min(total);

    while AUDIO_BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }
    let phys = pcm_phys();
    unsafe {
        #[allow(static_mut_refs)]
        match &mut AUDIO {
            AudioDev::Hda(h) => h.play_pcm(phys, total),
            AudioDev::Ac97(a) => {
                if phys < 0xFFFF_FFFF { a.play_pcm(phys as u32, len); }
            }
            AudioDev::None => {}
        }
    }
    AUDIO_BUSY.store(false, Ordering::Release);
}

pub fn play(data: &[u8]) {
    let cap = pcm_cap();
    if cap <= TAIL { return; }
    let n = data.len().min(cap - TAIL);
    if n == 0 { return; }
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), pcm_ptr(), n); }
    play_pcm(n);
}

pub fn stop() {
    while AUDIO_BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }
    unsafe {
        #[allow(static_mut_refs)]
        match &mut AUDIO {
            AudioDev::Hda(h) => h.stop(),
            AudioDev::Ac97(a) => a.stop(),
            AudioDev::None => {}
        }
    }
    AUDIO_BUSY.store(false, Ordering::Release);
}

pub fn tick() {
    if AUDIO_BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return;
    }
    unsafe {
        #[allow(static_mut_refs)]
        match &mut AUDIO {
            AudioDev::Hda(h) => h.tick(),
            _ => {}
        }
    }
    AUDIO_BUSY.store(false, Ordering::Release);
}

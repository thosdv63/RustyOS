use super::Row;
use x86_64::instructions::port::Port;
use alloc::string::String;
use core::fmt::Write;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const K_UP: u8 = 1;
pub const K_DOWN: u8 = 2;
pub const K_LEFT: u8 = 3;
pub const K_RIGHT: u8 = 4;
pub const K_ENTER: u8 = 5;
pub const K_ESC: u8 = 6;

const ITEMS: [&str; 5] = [
    "Bolum bicimlendiriliyor",
    "Onyukleyici kopyalaniyor",
    "Cekirdek kopyalaniyor",
    "Sistem dosyalari yaziliyor",
    "Tamamlaniyor",
];


fn wait_ib() { 
    let mut st = Port::<u8>::new(0x64);
    let mut g = 0u32;
    unsafe { while st.read() & 0x02 != 0 { g += 1; if g > 500_000 { return; } } }
}
fn drain() { 
    let mut st = Port::<u8>::new(0x64);
    let mut dp = Port::<u8>::new(0x60);
    let mut g = 0u32;
    unsafe { while st.read() & 0x01 != 0 { let _ = dp.read(); g += 1; if g > 4096 { return; } } }
}

fn read_sc_try() -> Option<u8> {
    let mut st = Port::<u8>::new(0x64);
    let mut dp = Port::<u8>::new(0x60);
    let s = unsafe { st.read() };
    if s & 0x01 != 0 {
        let d = unsafe { dp.read() };
        if s & 0x20 == 0 { return Some(d); }
    }
    None
}

fn wait_release(make: u8) {
    let brk = make | 0x80;
    let mut idle = 0u32;
    loop {
        match read_sc_try() {
            Some(sc) => { if sc == brk { break; } idle = 0; }
            None => { idle += 1; if idle > 2_000_000 { break; } }
        }
        core::hint::spin_loop();
    }
    drain();
}

pub fn flush_kbd() {
    let mut cmd = Port::<u8>::new(0x64);
    let mut data = Port::<u8>::new(0x60);
    unsafe {
        drain();
        wait_ib(); cmd.write(0xD4u8);
        wait_ib(); data.write(0xF5u8);
        for _ in 0..200_000 { core::hint::spin_loop(); } // ACK icin kisa bekle
        drain();
        wait_ib(); cmd.write(0xA7u8);
        drain();
        wait_ib(); cmd.write(0xAEu8);
        drain();
    }
}
fn read_sc() -> u8 {
    let mut st = Port::<u8>::new(0x64);
    let mut dp = Port::<u8>::new(0x60);
    loop {
        let s = unsafe { st.read() };
        if s & 0x01 != 0 {
            let d = unsafe { dp.read() };
            if s & 0x20 == 0 { return d; }
        }
        core::hint::spin_loop();
    }
}

fn cmos(reg: u8) -> u8 {
    let mut a = Port::<u8>::new(0x70);
    let mut d = Port::<u8>::new(0x71);
    unsafe { a.write(reg); d.read() }
}
fn rtc_sec() -> u8 {
    let mut g = 0u32;
    while cmos(0x0A) & 0x80 != 0 { g += 1; if g > 5_000_000 { break; } } // guncelleme bitsin
    cmos(0x00)
}
pub fn wait_1s() {
    let s0 = rtc_sec();
    let mut g = 0u64;
    loop {
        if rtc_sec() != s0 { return; }
        g += 1; if g > 500_000_000 { return; }
        core::hint::spin_loop();
    }
}

pub fn wait_key() -> u8 {
    loop {
        let sc = read_sc();
        if sc == 0xFA || sc == 0xAA || sc == 0xEE { continue; }
        if sc & 0x80 != 0 { continue; }
        let k = match sc {
            0x48 => K_UP,
            0x50 => K_DOWN,
            0x4B => K_LEFT,
            0x4D => K_RIGHT,
            0x1C | 0x5A => K_ENTER,
            0x01 => K_ESC,
            _ => continue,
        };
        wait_release(sc);  
        return k;
    }
}
pub fn delay_ms(ms: u64) {
    for _ in 0..(ms * 200_000) { unsafe { core::arch::asm!("nop"); } }
}

fn outer_rect() -> (usize, usize, usize, usize) {
    let r = unsafe { crate::renderer() };
    let w = r.width; let h = r.height;
    let ow = (w * 78 / 100).min(w.saturating_sub(30));
    let oh = (h * 74 / 100).min(h.saturating_sub(30));
    (( w - ow) / 2, (h - oh) / 2, ow, oh)
}
fn panel_rect() -> (usize, usize, usize, usize) {
    let (ox, oy, ow, oh) = outer_rect();
    (ox, oy + 44, ow, oh - 44)
}
fn bg() {
    let r = unsafe { crate::renderer() };
    let w = r.width; let h = r.height;
    let bands = 40usize;
    for i in 0..bands {
        let t = i as u32; let d = bands as u32 - 1;
        let rr = 74 - (74 - 28) * t / d;
        let gg = 144 - (144 - 62) * t / d;
        let bb = 217 - (217 - 110) * t / d;
        let col = (rr << 16) | (gg << 8) | bb;
        let y = h * i / bands; let y2 = h * (i + 1) / bands;
        r.fill_rect(0, y, w, y2 - y, col);
    }
}
fn panel(title: &str) -> (usize, usize, usize, usize) {
    bg();
    let (ox, oy, ow, oh) = outer_rect();
    let r = unsafe { crate::renderer() };
    r.fill_rect(ox + 6, oy + 6, ow, oh, 0x00102840);
    r.fill_rect(ox, oy, ow, oh, 0x00F2F2F2);
    r.draw_rect(ox, oy, ow, oh, 0x00808080);
    r.fill_rect(ox, oy, ow, 34, 0x00D8E4F0);
    r.draw_hline(ox, oy + 34, ow, 0x00A0B0C0);
    r.set_color(0x001A3A6A);
    r.text_at(ox + 16, oy + 9, title);
    (ox, oy + 44, ow, oh - 44)
}
fn text_center(cx: usize, y: usize, s: &str, col: u32) {
    let r = unsafe { crate::renderer() };
    let x = cx.saturating_sub(s.len() * 18 / 2);
    r.set_color(col);
    r.text_at(x, y, s);
}

pub fn welcome() {
    let (px, py, pw, ph) = panel("Rusty Kurulum");
    let cx = px + pw / 2;
    text_center(cx, py + ph / 2 - 70, "Rusty OS", 0x00E0600A);
    text_center(cx, py + ph / 2 - 30, "Welcome to Setup", 0x00202020);
    text_center(cx, py + ph / 2 + 20, "This setup will install Rusty OS to a partition..", 0x00505050);
    text_center(cx, py + ph - 40, "Press ENTER to continue.", 0x00204080);
}

pub fn partition_screen(rows: &[Row], sel: usize) {
    let (px, py, pw, ph) = panel("Where do you want to install Rusty?");
    let r = unsafe { crate::renderer() };
    r.set_color(0x00303030);
    r.text_at(px + 16, py + 2, "Select a partition. [PROTECTED] partitions cannot be modified.");

    let row_h = 42usize;
    let top = py + 30;
    let avail = ((ph - 30 - 34) / row_h).max(1);
    let start = if sel >= avail { sel - avail + 1 } else { 0 };
    let start = start.min(rows.len().saturating_sub(avail));
    let end = (start + avail).min(rows.len());

    let mut y = top;
    for i in start..end {
        let row = &rows[i];
        let is_sel = i == sel;
        if row.header {
            r.fill_rect(px + 8, y, pw - 16, row_h, 0x00203848);
            r.set_color(0x00FFFFFF);
            r.text_at(px + 16, y + 11, &row.line1);
        } else {
            if is_sel { r.fill_rect(px + 8, y, pw - 16, row_h, 0x003399FF); }
            else if row.protected { r.fill_rect(px + 8, y, pw - 16, row_h, 0x00E6E6E6); }
            let c1 = if is_sel { 0x00FFFFFF } else if row.protected { 0x00909090 } else { 0x00202020 };
            let c2 = if is_sel { 0x00E0F0FF } else if row.protected { 0x00A0A0A0 } else { 0x00606060 };
            r.set_color(c1);
            r.text_at(px + 16, y + 4, &row.line1);
            r.set_color(c2);
            r.text_at(px + 30, y + 23, &row.line2);
            if row.protected {
                r.set_color(if is_sel { 0x00FFFFFF } else { 0x00B07020 });
                r.text_at(px + pw - 180, y + 13, "[PROTECTED]");
            }
        }
        y += row_h;
    }
    if start > 0 { r.set_color(0x00203848); r.text_at(px + pw / 2, top - 4, "^"); }
    if end < rows.len() { r.set_color(0x00203848); r.text_at(px + pw / 2, top + avail * row_h - 6, "v"); }

    r.set_color(0x00204080);
    r.text_at(px + 16, py + ph - 26, "Arrows: select   ENTER: install   ESC: back");
}

pub fn confirm_part(row: &Row) {
    let (px, py, pw, ph) = panel("FINAL APPROVAL - ATTENTION");
    let cx = px + pw / 2;
    text_center(cx, py + 24, "This partition will be formatted and Rusty will be installed:", 0x00303030);
    text_center(cx, py + 62, row.line1.trim(), 0x00B02020);
    text_center(cx, py + 92, &row.line2, 0x00404040);
    text_center(cx, py + ph / 2 + 20, "Other partitions (Windows/Linux/EFI) will NOT be touched.", 0x002E7D32);
    text_center(cx, py + ph / 2 + 50, "But ALL data in this partition will be deleted!", 0x00B02020);
    text_center(cx, py + ph - 40, "ENTER: INSTALL       ESC: Cancel", 0x00204080);
    wait_1s();   // <-- 1 sn tus kabul etme (guvenlik)
    drain();
}

static LAST_STEP: AtomicUsize = AtomicUsize::new(usize::MAX);
static LAST_PCT:  AtomicUsize = AtomicUsize::new(usize::MAX);
static PANEL_X:   AtomicUsize = AtomicUsize::new(0);
static PANEL_Y:   AtomicUsize = AtomicUsize::new(0);
static PANEL_W:   AtomicUsize = AtomicUsize::new(0);

pub fn install_begin() {
    let (px, py, pw, _ph) = panel("Rusty Installing");
    PANEL_X.store(px, Ordering::Relaxed);
    PANEL_Y.store(py, Ordering::Relaxed);
    PANEL_W.store(pw, Ordering::Relaxed);
    LAST_STEP.store(usize::MAX, Ordering::Relaxed);
    LAST_PCT.store(usize::MAX, Ordering::Relaxed);
 
    let r = unsafe { crate::renderer() };
    let mut y = py + 16;
    for it in ITEMS.iter() {
        r.fill_rect(px + 22, y + 3, 12, 12, 0x00C8C8C8);
        r.draw_rect(px + 22, y + 3, 12, 12, 0x00808080);
        r.set_color(0x00A0A0A0);
        r.text_at(px + 44, y, it);
        y += 28;
    }
    let by = py + 16 + ITEMS.len() * 28 + 14;
    let bw = pw - 44;
    r.fill_rect(px + 22, by, bw, 20, 0x00E8E8E8);
    r.draw_rect(px + 22, by, bw, 20, 0x00808080);
}


pub fn debug_key_loop() -> ! {
    let r = unsafe { crate::renderer() };
    let mut st = Port::<u8>::new(0x64);
    let mut dp = Port::<u8>::new(0x60);
    let mut x = 20usize;
    let mut y = 400usize;
    r.set_color(0x0000FF00);
    r.text_at(20, 370, "DEBUG: Press a key to see the bytes (ESC wait)");
    loop {
        let s = unsafe { st.read() };
        if s & 0x01 != 0 {
            let data = unsafe { dp.read() };
            // her byte'i hex yaz: [st=XX d=YY]
            let mut buf = String::new();
            let _ = write!(buf, "s{:02X}:d{:02X} ", s, data);
            r.set_color(if s & 0x20 == 0 { 0x0000FF00 } else { 0x00FF4040 }); // keyboard green, mouse red
            r.text_at(x, y, &buf);
            x += 9 * 8;
            if x > r.width - 100 { x = 20; y += 20; if y > r.height - 40 { y = 400; } }
        }
        core::hint::spin_loop();
    }
}

pub fn install_screen(step: usize, pct: u32) {
    let px = PANEL_X.load(Ordering::Relaxed);
    let py = PANEL_Y.load(Ordering::Relaxed);
    let pw = PANEL_W.load(Ordering::Relaxed);
    if pw == 0 { return;}
 
    let r = unsafe { crate::renderer() };
    let prev = LAST_STEP.swap(step, Ordering::Relaxed);
 
    if prev != step {
        let mut y = py + 16;
        for (i, it) in ITEMS.iter().enumerate() {
            let done = i < step;
            let cur = i == step;
            let bc = if done { 0x0033A02A } else if cur { 0x00FF9020 } else { 0x00C8C8C8 };
            r.fill_rect(px + 22, y + 3, 12, 12, bc);
            r.draw_rect(px + 22, y + 3, 12, 12, 0x00808080);
            r.set_color(if done || cur { 0x00202020 } else { 0x00A0A0A0 });
            r.text_at(px + 44, y, it);
            y += 28;
        }
        LAST_PCT.store(usize::MAX, Ordering::Relaxed);
    }
 
    let p = pct.min(100) as usize;
    if LAST_PCT.swap(p, Ordering::Relaxed) == p { return; }
 
    let by = py + 16 + ITEMS.len() * 28 + 14;
    let bw = pw - 44;
    let fw = bw.saturating_sub(2) * p / 100;
    r.fill_rect(px + 23, by + 1, fw, 18, 0x003399FF);
    if fw < bw - 2 {
        r.fill_rect(px + 23 + fw, by + 1, bw - 2 - fw, 18, 0x00E8E8E8);
    }
}



pub fn done_and_reboot(part_index: u32) -> ! {
    let (px, py, pw, ph) = panel("Installation Finished");
    let cx = px + pw / 2;
    text_center(cx, py + ph / 2 - 60, "Rusty OS has been successfully installed!", 0x002E7D32);
    let mut l = String::new();
    let _ = write!(l, "Partition {} - installation succesful", part_index);
    text_center(cx, py + ph / 2 - 25, &l, 0x00303030);
    text_center(cx, py + ph / 2 + 15, "Remove the installation media (USB) now.", 0x00303030);
    let r = unsafe { crate::renderer() };
    for sec in (0..=8u32).rev() {
        r.fill_rect(px + 10, py + ph - 52, pw - 20, 28, 0x00F2F2F2);
        let mut c = String::new();
        let _ = write!(c, "It restarts within {} seconds...", sec);
        text_center(cx, py + ph - 45, &c, 0x00204080);
        wait_1s();
    }
    crate::drivers::power::reboot();
}

pub fn error_screen(msg: &str) {
    let (px, py, pw, ph) = panel("Installation Error");
    let cx = px + pw / 2;
    text_center(cx, py + ph / 2 - 20, "Error during installation:", 0x00B02020);
    text_center(cx, py + ph / 2 + 20, msg, 0x00303030);
    text_center(cx, py + ph - 40, "Press ENTER to continue", 0x00204080);
    LAST_STEP.store(usize::MAX, Ordering::Relaxed);
}
pub fn fatal(msg: &str) -> ! {
    let (px, py, pw, ph) = panel("Error");
    let cx = px + pw / 2;
    text_center(cx, py + ph / 2 - 20, "Installation could not be started:", 0x00B02020);
    text_center(cx, py + ph / 2 + 20, msg, 0x00303030);
    text_center(cx, py + ph - 40, "You can turn off the computer.", 0x00606060);
    loop { unsafe { core::arch::asm!("hlt"); } }
}
use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

const TOP: usize      = 34;   // toolbar yuksekligi
const STATUS_H: usize = 22;
const LH: usize       = 14;   // satir yuksekligi
const CW: usize       = 7;    // karakter genisligi
const GUT: usize      = 38;   // satir no sutunu
const MAX_BYTES: usize = 16000;

pub struct Notepad {
    path: String,
    lines: Vec<String>,
    cx: usize,      // sutun (karakter)
    cy: usize,      // satir
    sy: usize,      // dikey kaydirma
    sx: usize,      // yatay kaydirma
    dirty: bool,
    loaded: bool,
    msg: Option<&'static str>,
    saveas: bool,
    name_buf: String,
    confirm_new: bool,
    vw: usize,
    vh: usize,
}

impl Notepad {
    pub fn new() -> Self { Self::new_at(String::new()) }

    pub fn new_at(path: String) -> Self {
        Notepad {
            path, lines: Vec::new(),
            cx: 0, cy: 0, sy: 0, sx: 0,
            dirty: false, loaded: false, msg: None,
            saveas: false, name_buf: String::new(),
            confirm_new: false,
            vw: 0, vh: 0,
        }
    }

    fn load(&mut self) {
        self.lines.clear();
        self.cx = 0; self.cy = 0; self.sy = 0; self.sx = 0;
        self.dirty = false;
        self.loaded = true;

        if self.path.is_empty() {
            self.lines.push(String::new());
            return;
        }

        let mut buf = vec![0u8; MAX_BYTES];
        let n = (syscall::sys_read_file(&self.path, &mut buf) as usize).min(buf.len());

        if n == 0 { self.lines.push(String::new()); return; }

        // gecersiz baytlari temizle
        let mut clean = String::new();
        for &b in buf[..n].iter() {
            match b {
                b'\r' => {}
                b'\t' => clean.push_str("    "),
                b'\n' => clean.push('\n'),
                32..=126 => clean.push(b as char),
                _ => clean.push('.'),
            }
        }
        for l in clean.split('\n') { self.lines.push(String::from(l)); }
        if self.lines.is_empty() { self.lines.push(String::new()); }
    }

    fn text(&self) -> String {
        let mut out = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 { out.push('\n'); }
            out.push_str(l);
        }
        out
    }

    fn sys_drive() -> String {
        let mut buf = vec![0u8; 512];
        let n = syscall::sys_list_dir("", &mut buf);
        if n > 0 && buf[0].is_ascii_alphabetic() {
            let mut s = String::new();
            s.push(buf[0] as char);
            s.push(':');
            s
        } else {
            String::from("C:")
        }
    }

    fn save(&mut self) {
        if self.path.is_empty() {
            self.saveas = true;
            self.name_buf.clear();
            return;
        }
        let t = self.text();
        if t.len() > MAX_BYTES { self.msg = Some("Dosya cok buyuk!"); return; }
        match syscall::sys_write_file(&self.path, t.as_bytes()) {
            0 => { self.dirty = false; self.msg = Some("Kaydedildi."); }
            2 => self.msg = Some("Bu dosyaya yazma izniniz yok!"),
            _ => self.msg = Some("Kaydedilemedi!"),
        }
    }

    fn do_saveas(&mut self) {
        if self.name_buf.is_empty() { self.saveas = false; return; }
        let dir = format!("{}/Users/User/Desktop", Self::sys_drive());
        let mut name = self.name_buf.clone();
        if !name.contains('.') { name.push_str(".TXT"); }
        self.path = format!("{}/{}", dir, name);
        self.saveas = false;
        self.save();
    }

    fn cur_len(&self) -> usize {
        self.lines.get(self.cy).map(|l| l.len()).unwrap_or(0)
    }

    fn rows(&self) -> usize {
        if self.vh < TOP + STATUS_H + LH { return 1; }
        (self.vh - TOP - STATUS_H) / LH
    }
    fn cols(&self) -> usize {
        if self.vw < GUT + 16 { return 1; }
        (self.vw - GUT - 12) / CW
    }

    fn scroll_to_cursor(&mut self) {
        let r = self.rows();
        let c = self.cols();
        if self.cy < self.sy { self.sy = self.cy; }
        if self.cy >= self.sy + r { self.sy = self.cy + 1 - r; }
        if self.cx < self.sx { self.sx = self.cx; }
        if self.cx >= self.sx + c { self.sx = self.cx + 1 - c; }
    }

    fn insert(&mut self, ch: char) {
        if self.lines.len() > 900 { return; }
        let l = &mut self.lines[self.cy];
        if l.len() >= 512 { return; }
        let cx = self.cx.min(l.len());
        l.insert(cx, ch);
        self.cx = cx + 1;
        self.dirty = true;
    }

    fn newline(&mut self) {
        if self.lines.len() > 900 { return; }
        let cx = self.cx.min(self.lines[self.cy].len());
        let rest = self.lines[self.cy].split_off(cx);
        self.lines.insert(self.cy + 1, rest);
        self.cy += 1;
        self.cx = 0;
        self.dirty = true;
    }

    fn backspace(&mut self) {
        if self.cx > 0 {
            let cx = self.cx.min(self.lines[self.cy].len());
            if cx == 0 { return; }
            self.lines[self.cy].remove(cx - 1);
            self.cx = cx - 1;
            self.dirty = true;
        } else if self.cy > 0 {
            let cur = self.lines.remove(self.cy);
            self.cy -= 1;
            self.cx = self.lines[self.cy].len();
            self.lines[self.cy].push_str(&cur);
            self.dirty = true;
        }
    }
}

fn btn(r: &Renderer, x: usize, y: usize, w: usize, label: &str, on: bool) {
    let (a, b) = if on { (0x00FFA850, 0x00D86818) } else { (0x00E2DAD2, 0x00BAB2AA) };
    r.fill_rounded_glossy(x, y, w, 22, 5, a, b);
    let lw = label.len() * CW;
    r.draw_text(label, x + w / 2 - lw / 2, y + 7, 0x00FFFFFF, 1);
}

fn u32s(mut n: usize, buf: &mut [u8; 8]) -> &str {
    if n == 0 { buf[0] = b'0'; return core::str::from_utf8(&buf[..1]).unwrap_or("0"); }
    let mut i = 8;
    while n > 0 && i > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    let len = 8 - i;
    for j in 0..len { buf[j] = buf[i + j]; }
    core::str::from_utf8(&buf[..len]).unwrap_or("?")
}

impl App for Notepad {
    fn title(&self) -> &'static str { "Not Defteri" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w;
        self.vh = h;
        if !self.loaded { self.load(); }
        if w < 260 || h < 140 { return; }

        let status_top = h - STATUS_H;

        // === Metin alani ===
        r.fill_rect(x, y, w, h, 0x00FFFFFF);

        // === Toolbar ===
        r.fill_gradient(x, y, w, TOP, 0x00F9F4EF, 0x00E5DACE);
        r.fill_rect(x, y + TOP - 1, w, 1, 0x00C9B9A9);
        btn(r, x + 8,   y + 6, 62, "Kaydet", self.dirty || self.path.is_empty());
        btn(r, x + 76,  y + 6, 96, "Farkli Kaydet", true);
        btn(r, x + 178, y + 6, 48, "Yeni", true);

        let title = if self.path.is_empty() { "Adsiz" } else {
            self.path.rsplit('/').next().unwrap_or(&self.path)
        };
        let tx = x + 236;
        if tx + 12 < x + w {
            r.draw_text(title, tx, y + 13, 0x00604838, 1);
            if self.dirty {
                r.draw_text("*", tx + title.len() * CW + 3, y + 13, 0x00C04408, 1);
            }
        }

        // === Satir no sutunu ===
        r.fill_gradient(x, y + TOP, GUT, status_top - TOP, 0x00F6F1EC, 0x00EDE6DE);
        r.fill_rect(x + GUT - 1, y + TOP, 1, status_top - TOP, 0x00DCD0C4);

        let rows = self.rows();
        let cols = self.cols();
        let x0 = x + GUT + 6;
        let mut ry = y + TOP + 3;

        for i in 0..rows {
            let li = self.sy + i;
            if li >= self.lines.len() { break; }

            if li == self.cy {
                r.fill_rect_alpha(x + GUT, ry - 2, w - GUT - 2, LH, 0x00FF9030, 30);
            }

            let mut nb = [0u8; 8];
            let ns = u32s(li + 1, &mut nb);
            let nw = ns.len() * CW;
            if nw + 8 < GUT {
                r.draw_text(ns, x + GUT - 8 - nw, ry + 2, 0x00A89888, 1);
            }

            let line = &self.lines[li];
            let bytes = line.as_bytes();
            if self.sx < bytes.len() {
                let end = (self.sx + cols).min(bytes.len());
                if let Ok(seg) = core::str::from_utf8(&bytes[self.sx..end]) {
                    r.draw_text(seg, x0, ry + 2, 0x00201810, 1);
                }
            }

            // imlec
            if li == self.cy && !self.saveas && !self.confirm_new {
                let cc = self.cx.min(line.len());
                if cc >= self.sx && cc - self.sx <= cols {
                    let px = x0 + (cc - self.sx) * CW;
                    r.fill_rect(px, ry - 1, 1, LH - 2, 0x00C04408);
                }
            }
            ry += LH;
        }

        // === Durum cubugu ===
        r.fill_gradient(x, y + status_top, w, STATUS_H, 0x00F5EFE9, 0x00E3D9CF);
        r.fill_rect(x, y + status_top, w, 1, 0x00D9C9B9);
        let st = format!("Satir {}, Sutun {}   |   {} satir",
            self.cy + 1, self.cx.min(self.cur_len()) + 1, self.lines.len());
        r.draw_text(&st, x + 10, y + status_top + 7, 0x00605040, 1);

        // === Farkli Kaydet ===
        if self.saveas {
            let dx = x + w / 2 - 160;
            let dy = y + h / 2 - 55;
            r.fill_rect_alpha(x, y, w, h, 0x00000000, 90);
            r.fill_rounded(dx, dy, 320, 110, 8, 0x00301008);
            r.draw_rounded_border(dx, dy, 320, 110, 8, 0x00FF8020);
            r.draw_text("Dosya adi (8.3 formati):", dx + 16, dy + 12, 0x00FFE0C0, 1);
            r.fill_rect(dx + 16, dy + 36, 288, 20, 0x00FFFFFF);
            r.draw_rounded_border(dx + 16, dy + 36, 288, 20, 3, 0x00FF8020);
            r.draw_text(&self.name_buf, dx + 20, dy + 42, 0x00201008, 1);
            r.draw_text("_", dx + 20 + self.name_buf.len() * CW, dy + 42, 0x00C04408, 1);
            r.draw_text("Enter = kaydet, Esc = iptal", dx + 16, dy + 80, 0x00C0A080, 1);
        }

        // === Yeni onayi ===
        if self.confirm_new {
            let dx = x + w / 2 - 150;
            let dy = y + h / 2 - 50;
            r.fill_rect_alpha(x, y, w, h, 0x00000000, 90);
            r.fill_rounded(dx, dy, 300, 100, 8, 0x00301008);
            r.draw_rounded_border(dx, dy, 300, 100, 8, 0x00FF8020);
            r.draw_text("KAYDEDILMEMIS DEGISIKLIK", dx + 16, dy + 12, 0x00FF6040, 1);
            r.draw_text("Yine de yeni dosya acilsin mi?", dx + 16, dy + 32, 0x00FFE0C0, 1);
            r.fill_rounded_glossy(dx + 30, dy + 60, 100, 28, 5, 0x00F04020, 0x00A01808);
            r.draw_text("Evet", dx + 62, dy + 68, 0x00FFFFFF, 1);
            r.fill_rounded_glossy(dx + 170, dy + 60, 100, 28, 5, 0x00FFA850, 0x00D86818);
            r.draw_text("Hayir", dx + 200, dy + 68, 0x00FFFFFF, 1);
        }

        // === Mesaj ===
        if let Some(m) = self.msg {
            let dx = x + w / 2 - 140;
            let dy = y + h - STATUS_H - 46;
            r.fill_rounded(dx, dy, 280, 34, 6, 0x00301008);
            r.draw_rounded_border(dx, dy, 280, 34, 6, 0x00FF8020);
            let lw = m.len() * CW;
            r.draw_text(m, dx + 140 - lw / 2, dy + 12, 0x00FFE0C0, 1);
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Key { ch } => {
                if self.confirm_new { return false; }

                if self.saveas {
                    match *ch {
                        '\n' => self.do_saveas(),
                        '\u{1b}' => self.saveas = false,
                        '\u{8}' => { self.name_buf.pop(); }
                        c if (c as u32) >= 32 && c != '/' && c != ':' && c != '\\' => {
                            if self.name_buf.len() < 12 { self.name_buf.push(c); }
                        }
                        _ => {}
                    }
                    return true;
                }

                self.msg = None;
                match *ch {
                    '\n' => self.newline(),
                    '\u{8}' => self.backspace(),
                    '\u{1b}' => return false,
                    c if (c as u32) >= 32 => self.insert(c),
                    _ => return false,
                }
                self.scroll_to_cursor();
                true
            }

            AppEvent::Click { x, y } => {
                let w = self.vw as i32;
                let h = self.vh as i32;

                if self.confirm_new {
                    let dx = w / 2 - 150;
                    let dy = h / 2 - 50;
                    if *y >= dy + 60 && *y < dy + 88 {
                        if *x >= dx + 30 && *x < dx + 130 {
                            self.confirm_new = false;
                            self.path = String::new();
                            self.load();
                            return true;
                        }
                        if *x >= dx + 170 && *x < dx + 270 { self.confirm_new = false; return true; }
                    }
                    return true;
                }

                if self.saveas { return true; }

                if self.msg.is_some() { self.msg = None; return true; }

                // Toolbar
                if *y < TOP as i32 {
                    if *y >= 6 && *y < 28 {
                        if *x >= 8 && *x < 70 { self.save(); return true; }
                        if *x >= 76 && *x < 172 {
                            self.saveas = true;
                            self.name_buf.clear();
                            return true;
                        }
                        if *x >= 178 && *x < 226 {
                            if self.dirty { self.confirm_new = true; }
                            else { self.path = String::new(); self.load(); }
                            return true;
                        }
                    }
                    return false;
                }

                let status_top = h - STATUS_H as i32;
                if *y >= status_top { return false; }
                if *x < GUT as i32 { return false; }

                // Imleci yerlestir
                let row = ((*y - TOP as i32) / LH as i32).max(0) as usize;
                let li = (self.sy + row).min(self.lines.len().saturating_sub(1));
                self.cy = li;

                let px = (*x - (GUT as i32 + 6)).max(0) as usize;
                let col = self.sx + (px + CW / 2) / CW;
                self.cx = col.min(self.cur_len());

                self.scroll_to_cursor();
                true
            }

            AppEvent::Drag { .. } => false,
            AppEvent::RClick { .. } => false,
        }
    }
}
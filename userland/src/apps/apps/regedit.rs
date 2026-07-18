use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

pub struct Regedit {
    keys: Vec<(String, String, String)>, // (yol, tip, deger)
    sel: i32,
    scroll: usize,
    input: String,
    mode: u8, // 0=gezinme 1=deger duzenle 2=yeni anahtar
    msg: Option<String>,
    vw: usize, vh: usize,
}

impl Regedit {
    pub fn new() -> Self {
        let mut r = Regedit {
            keys: Vec::new(), sel: -1, scroll: 0,
            input: String::new(), mode: 0, msg: None, vw: 0, vh: 0,
        };
        r.reload();
        r
    }
    fn reload(&mut self) {
        self.keys.clear();
        let mut dump = vec![0u8; 8192];
        let n = syscall::sys_reg_list(&mut dump) as usize;
        if let Ok(t) = core::str::from_utf8(&dump[..n.min(8192)]) {
            for line in t.lines() {
                let l = line.trim();
                if l.is_empty() { continue; }
                let Some(eq) = l.find('=') else { continue };
                let path = &l[..eq];
                let rest = &l[eq+1..];
                let Some(c) = rest.find(':') else { continue };
                self.keys.push((String::from(path),
                    String::from(&rest[..c]), String::from(&rest[c+1..])));
            }
        }
        self.keys.sort_by(|a, b| a.0.cmp(&b.0));
        if self.sel >= self.keys.len() as i32 { self.sel = -1; }
    }
    fn apply(&mut self) {
        match self.mode {
            1 => {
                if self.sel < 0 { return; }
                let (p, t, _) = self.keys[self.sel as usize].clone();
                // tip dogrulama
                if t == "u32" && self.input.parse::<u32>().is_err() {
                    self.msg = Some(String::from("HATA: u32 icin sayi girin")); return;
                }
                if t == "bool" && !matches!(self.input.as_str(), "0" | "1" | "true" | "false") {
                    self.msg = Some(String::from("HATA: bool icin 0/1/true/false")); return;
                }
                let line = format!("{}={}:{}", p, t, self.input);
                if syscall::sys_reg_set_line(&line) == 0 {
                    self.msg = Some(format!("Yazildi: {}", p));
                } else {
                    self.msg = Some(String::from("HATA: yazilamadi"));
                }
            }
            2 => {
                // format: Yol=tip:deger
                let ok = self.input.contains('=') && self.input.contains(':');
                if !ok { self.msg = Some(String::from("Format: Yol=tip:deger (tip: str/u32/bool)")); return; }
                if syscall::sys_reg_set_line(&self.input) == 0 {
                    self.msg = Some(String::from("Anahtar eklendi."));
                } else {
                    self.msg = Some(String::from("HATA: eklenemedi"));
                }
            }
            _ => {}
        }
        self.mode = 0;
        self.input.clear();
        self.reload();
    }
}

impl App for Regedit {
    fn title(&self) -> &'static str { "Kayit Duzenleyici" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w; self.vh = h;
        r.fill_rect(x, y, w, h, 0x00FBF8F5);

        // Arac cubugu
        r.fill_gradient(x, y, w, 30, 0x00F0E8E0, 0x00E0D4C8);
        let btn = |r: &Renderer, bx: usize, lbl: &str, wpx: usize| {
            r.fill_rounded_glossy(bx, y + 4, wpx, 22, 4, 0x00FF9030, 0x00C06010);
            r.draw_text(lbl, bx + 8, y + 10, 0x00FFFFFF, 1);
        };
        btn(r, x + 6, "Yenile", 58);
        btn(r, x + 70, "Yeni", 46);
        r.fill_rounded_glossy(x + w - 60, y + 4, 24, 22, 4, 0x00F0E4D8, 0x00D0C0B0);
        r.draw_text("^", x + w - 52, y + 10, 0x00604838, 1);
        r.fill_rounded_glossy(x + w - 32, y + 4, 24, 22, 4, 0x00F0E4D8, 0x00D0C0B0);
        r.draw_text("v", x + w - 24, y + 10, 0x00604838, 1);

        // Liste
        let list_top = y + 36;
        let bottom_h = 58;
        let rows = (h - 36 - bottom_h) / 18;
        let max_scroll = self.keys.len().saturating_sub(rows);
        if self.scroll > max_scroll { self.scroll = max_scroll; }

        let mut ry = list_top;
        for (i, (p, t, v)) in self.keys.iter().enumerate().skip(self.scroll).take(rows) {
            if i as i32 == self.sel {
                r.fill_rect_alpha(x + 2, ry - 2, w - 4, 17, 0x00FF9030, 70);
            }
            let tc = match t.as_str() {
                "u32" => 0x001060C0, "bool" => 0x00A03090, _ => 0x00308030,
            };
            r.draw_text(p, x + 6, ry, 0x00403028, 1);
            let vs = format!("{}:{}", t, v);
            let vx = x + w.saturating_sub(vs.len() * 7 + 8);
            r.draw_text(&vs, vx.max(x + 200), ry, tc, 1);
            ry += 18;
        }
        if self.keys.is_empty() { r.draw_text("(kayit yok)", x + 8, list_top + 4, 0x00A09080, 1); }

        // Alt panel: duzenleme
        let by = y + h - bottom_h;
        r.fill_rect(x, by, w, bottom_h, 0x00F0E6DA);
        r.fill_rect(x, by, w, 1, 0x00C8B4A0);
        match self.mode {
            1 => {
                let p = self.keys.get(self.sel as usize).map(|k| k.0.clone()).unwrap_or_default();
                r.draw_text(&format!("Duzenle: {}", p), x + 6, by + 6, 0x00806040, 1);
            }
            2 => { r.draw_text("Yeni (Yol=tip:deger):", x + 6, by + 6, 0x00806040, 1); }
            _ => {
                r.draw_text("Cift islem: satir sec -> tekrar tikla = duzenle. ENTER = uygula",
                    x + 6, by + 6, 0x00A09080, 1);
            }
        }
        r.fill_rounded(x + 6, by + 24, w - 12, 24, 4, 0x00FFFFFF);
        r.draw_rounded_border(x + 6, by + 24, w - 12, 24, 4,
            if self.mode != 0 { 0x00FF8020 } else { 0x00C0B0A0 });
        let cols = (w - 24) / 7;
        let shown: String = if self.input.len() > cols {
            self.input.chars().skip(self.input.len() - cols).collect()
        } else { self.input.clone() };
        r.draw_text(&shown, x + 12, by + 31, 0x00302018, 1);
        if self.mode != 0 { r.draw_text("_", x + 12 + shown.len() * 7, by + 31, 0x00FF8020, 1); }
        if let Some(m) = &self.msg {
            let mc = if m.starts_with("HATA") { 0x00C02020 } else { 0x00308030 };
            r.draw_text(m, x + 200, by + 6, mc, 1);
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Click { x, y } => {
                let w = self.vw as i32; let h = self.vh as i32;
                // arac cubugu
                if *y < 30 {
                    if *x >= 6 && *x < 64 { self.reload(); self.msg = None; return true; }
                    if *x >= 70 && *x < 116 { self.mode = 2; self.input.clear(); return true; }
                    if *x >= w - 60 && *x < w - 36 { self.scroll = self.scroll.saturating_sub(5); return true; }
                    if *x >= w - 32 && *x < w - 8 { self.scroll += 5; return true; }
                    return false;
                }
                // liste
                let bottom_h = 58;
                if *y >= 36 && *y < h - bottom_h {
                    let idx = self.scroll + ((*y - 36) / 18) as usize;
                    if idx < self.keys.len() {
                        if self.sel == idx as i32 {
                            // ikinci tik: duzenlemeye gir
                            self.mode = 1;
                            self.input = self.keys[idx].2.clone();
                        } else {
                            self.sel = idx as i32;
                            self.mode = 0;
                        }
                        self.msg = None;
                        return true;
                    }
                    self.sel = -1;
                    return true;
                }
                false
            }
            AppEvent::Key { ch } => {
                if self.mode == 0 { return false; }
                match *ch {
                    '\n' => self.apply(),
                    '\u{1b}' => { self.mode = 0; self.input.clear(); }
                    '\u{8}' => { self.input.pop(); }
                    c if c as u32 >= 32 && self.input.len() < 120 => self.input.push(c),
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }
}
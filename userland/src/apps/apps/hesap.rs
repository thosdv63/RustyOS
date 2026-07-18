use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use alloc::string::String;
use alloc::format;

const BTNS: [[&str; 4]; 5] = [
    ["C", "<", "%", "/"],
    ["7", "8", "9", "*"],
    ["4", "5", "6", "-"],
    ["1", "2", "3", "+"],
    ["+-", "0", ".", "="],
];

pub struct Hesap {
    cur: String,
    prev: f64,
    op: char,
    fresh: bool, // yeni sayi baslayacak
    vw: usize, vh: usize,
}

impl Hesap {
    pub fn new() -> Self {
        Hesap { cur: String::from("0"), prev: 0.0, op: ' ', fresh: true, vw: 0, vh: 0 }
    }
    fn val(&self) -> f64 {
        self.cur.parse::<f64>().unwrap_or(0.0)
    }
    fn setv(&mut self, v: f64) {
        if v.is_nan() || v.is_infinite() { self.cur = String::from("Hata"); }
        else {
            let s = format!("{}", v);
            self.cur = if s.len() > 14 { format!("{:.6}", v) } else { s };
        }
        self.fresh = true;
    }
    fn apply(&mut self) {
        let b = self.val();
        let r = match self.op {
            '+' => self.prev + b,
            '-' => self.prev - b,
            '*' => self.prev * b,
            '/' => if b == 0.0 { f64::NAN } else { self.prev / b },
            _ => b,
        };
        self.op = ' ';
        self.setv(r);
    }
    fn press(&mut self, b: &str) {
        match b {
            "C" => { self.cur = String::from("0"); self.prev = 0.0; self.op = ' '; self.fresh = true; }
            "<" => {
                if !self.fresh { self.cur.pop(); if self.cur.is_empty() { self.cur.push('0'); } }
            }
            "+-" => { let v = -self.val(); self.setv(v); self.fresh = false; }
            "%" => { let v = self.val() / 100.0; self.setv(v); }
            "=" => { if self.op != ' ' { self.apply(); } }
            "+" | "-" | "*" | "/" => {
                if self.op != ' ' && !self.fresh { self.apply(); }
                self.prev = self.val();
                self.op = b.chars().next().unwrap();
                self.fresh = true;
            }
            "." => {
                if self.fresh { self.cur = String::from("0."); self.fresh = false; }
                else if !self.cur.contains('.') { self.cur.push('.'); }
            }
            d => { // rakam
                if self.fresh || self.cur == "0" || self.cur == "Hata" {
                    self.cur.clear(); self.fresh = false;
                }
                if self.cur.len() < 14 { self.cur.push_str(d); }
            }
        }
    }
}

impl App for Hesap {
    fn title(&self) -> &'static str { "Hesap Makinesi" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w; self.vh = h;
        r.fill_rect(x, y, w, h, 0x00F5F0EA);

        // Ekran
        r.fill_rounded(x + 8, y + 8, w - 16, 40, 4, 0x00FFFFFF);
        r.draw_rounded_border(x + 8, y + 8, w - 16, 40, 4, 0x00C0B0A0);
        let dv = &self.cur;
        let tx = x + w - 16 - dv.len() * 14;
        r.draw_text(dv, tx.max(x + 12), y + 20, 0x00302018, 2);
        if self.op != ' ' {
            let mut b = [0u8; 4];
            r.draw_text(self.op.encode_utf8(&mut b), x + 14, y + 20, 0x00FF8020, 2);
        }

        // Butonlar
        let top = y + 56;
        let bw = (w - 16) / 4;
        let bh = (h - 64) / 5;
        for (row, line) in BTNS.iter().enumerate() {
            for (col, lbl) in line.iter().enumerate() {
                let bx = x + 8 + col * bw;
                let by = top + row * bh;
                let (c1, c2, fg) = match *lbl {
                    "=" => (0x00FF9030, 0x00C05808, 0x00FFFFFF),
                    "C" => (0x00F05038, 0x00B02818, 0x00FFFFFF),
                    "+" | "-" | "*" | "/" | "%" | "<" | "+-" =>
                        (0x00F0E4D8, 0x00D8C4B0, 0x00604838),
                    _ => (0x00FFFFFF, 0x00E8DED2, 0x00302018),
                };
                r.fill_rounded_glossy(bx + 2, by + 2, bw - 4, bh - 4, 5, c1, c2);
                r.draw_rounded_border(bx + 2, by + 2, bw - 4, bh - 4, 5, 0x00C8B8A8);
                let lx = bx + bw / 2 - lbl.len() * 4;
                r.draw_text(lbl, lx, by + bh / 2 - 6, fg, 1);
            }
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Click { x, y } => {
                let w = self.vw; let h = self.vh;
                if *y < 56 { return false; }
                let bw = ((w - 16) / 4) as i32;
                let bh = ((h - 64) / 5) as i32;
                if bw == 0 || bh == 0 { return false; }
                let col = ((*x - 8) / bw).clamp(0, 3) as usize;
                let row = ((*y - 56) / bh).clamp(0, 4) as usize;
                self.press(BTNS[row][col]);
                true
            }
            AppEvent::Key { ch } => {
                match *ch {
                    '0'..='9' => { let mut b = [0u8; 4]; self.press((*ch).encode_utf8(&mut b)); }
                    '+' | '-' | '*' | '/' | '.' => { let mut b = [0u8; 4]; self.press((*ch).encode_utf8(&mut b)); }
                    '\n' | '=' => self.press("="),
                    '\u{8}' => self.press("<"),
                    '\u{1b}' => self.press("C"),
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }
}
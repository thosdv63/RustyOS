use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::apps::bmp;
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

const CANW: usize = 160;
const CANH: usize = 120;
const MAX_WRITE: usize = 60 * 1024; // kernel write_file_call buffer siniri (guvenli)
const TOOL_H: i32 = 58;
const PAL: [u32; 10] = [
    0x00000000, 0x00FFFFFF, 0x00E03020, 0x00FF8020, 0x00F0C020,
    0x0030A040, 0x002070E0, 0x00803090, 0x00805030, 0x00909090,
];

pub struct Paint {
    canvas: Vec<u32>,
    color: u32,
    size: i32,
    eraser: bool,
    lx: i32, ly: i32,   // son firca noktasi (-1 = yok)
    name: String,       // dosya adi (uzantisiz)
    editing: bool,
    msg: Option<String>,
    drive: String,
}

impl Paint {
    pub fn new(path: String) -> Self {
        let mut p = Paint {
            canvas: vec![0x00FFFFFF; CANW * CANH],
            color: 0x00000000, size: 3, eraser: false,
            lx: -1, ly: -1,
            name: String::from("RESIM"),
            editing: false, msg: None,
            drive: String::from("C:"),
        };
        // sistem surucusu
        let mut buf = vec![0u8; 256];
        let n = syscall::sys_list_dir("", &mut buf);
        if n >= 1 && buf[0].is_ascii_alphabetic() && buf[1] == b':' {
            p.drive.clear();
            p.drive.push(buf[0] as char);
            p.drive.push(':');
        }
        if !path.is_empty() { p.open_path(&path); }
        p
    }
    fn full(&self) -> String {
        format!("{}/Users/User/Documents/{}.BMP", self.drive, self.name)
    }
    fn open_path(&mut self, path: &str) {
        let mut buf = vec![0u8; 512 * 1024];
        let n = syscall::sys_read_file(path, &mut buf) as usize;
        if n == 0 { self.msg = Some(String::from("Acilamadi.")); return; }
        match bmp::decode(&buf[..n.min(buf.len())]) {
            Some((iw, ih, px)) => {
                self.canvas.iter_mut().for_each(|p| *p = 0x00FFFFFF);
                for y in 0..ih.min(CANH) {
                    for x in 0..iw.min(CANW) {
                        self.canvas[y * CANW + x] = px[y * iw + x];
                    }
                }
                if let Some(fname) = path.rsplit('/').next() {
                    self.name = fname.trim_end_matches(".BMP").trim_end_matches(".bmp").into();
                }
                self.msg = Some(format!("Acildi ({}x{})", iw, ih));
            }
            None => self.msg = Some(String::from("BMP cozulemedi.")),
        }
    }
    fn save(&mut self) {
        let data = bmp::encode24(CANW, CANH, &self.canvas);
        // Kernel write buffer sinirini asma kontrolu (cokme yerine uyari)
        if data.len() > MAX_WRITE {
            self.msg = Some(format!("HATA: dosya cok buyuk ({}KB)", data.len() / 1024));
            return;
        }
        let path = self.full();
        match syscall::sys_write_file(&path, &data) {
            0 => self.msg = Some(format!("Kaydedildi: {}.BMP", self.name)),
            _ => self.msg = Some(String::from("HATA: kaydedilemedi")),
        }
    }
    fn open(&mut self) {
        let p = self.full();
        self.open_path(&p);
    }
    // canvas'a kalin cizgi (Bresenham + kare firca)
    fn cline(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let col = if self.eraser { 0x00FFFFFF } else { self.color };
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs(); let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let s = if self.eraser { self.size + 3 } else { self.size };
            for oy in -s/2..=s/2 {
                for ox in -s/2..=s/2 {
                    let px = x + ox; let py = y + oy;
                    if px >= 0 && py >= 0 && (px as usize) < CANW && (py as usize) < CANH {
                        self.canvas[py as usize * CANW + px as usize] = col;
                    }
                }
            }
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }
}

impl App for Paint {
    fn title(&self) -> &'static str { "Paint" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        r.fill_rect(x, y, w, h, 0x00EFE8E0);

        // ==== Arac cubugu ====
        r.fill_gradient(x, y, w, TOOL_H as usize, 0x00F5EDE4, 0x00E5D8CA);
        // Palet
        for (i, c) in PAL.iter().enumerate() {
            let sx = x + 6 + i * 22;
            r.fill_rounded(sx, y + 5, 18, 18, 3, *c);
            let bc = if !self.eraser && *c == self.color { 0x00FF8020 } else { 0x00907860 };
            r.draw_rounded_border(sx, y + 5, 18, 18, 3, bc);
        }
        // Boyutlar
        let sizes = [1i32, 3, 6];
        for (i, s) in sizes.iter().enumerate() {
            let sx = x + 6 + i * 24;
            let selc = if self.size == *s { 0x00FF9030 } else { 0x00E0D4C8 };
            r.fill_rounded(sx, y + 30, 20, 20, 3, selc);
            r.fill_circle((sx + 10) as i32, (y + 40) as i32, *s, 0x00302018);
        }
        // Silgi + Temizle
        let ex = x + 86;
        r.fill_rounded_glossy(ex, y + 30, 48, 20, 3,
            if self.eraser { 0x00FF9030 } else { 0x00F0E4D8 },
            if self.eraser { 0x00C06010 } else { 0x00D0C0B0 });
        r.draw_text("Silgi", ex + 8, y + 36, if self.eraser { 0x00FFFFFF } else { 0x00604838 }, 1);
        r.fill_rounded_glossy(ex + 52, y + 30, 58, 20, 3, 0x00F0E4D8, 0x00D0C0B0);
        r.draw_text("Temizle", ex + 58, y + 36, 0x00604838, 1);

        // Dosya adi + Ac/Kaydet
        let fx = x + 230;
        r.fill_rounded(fx, y + 5, 110, 20, 3, 0x00FFFFFF);
        r.draw_rounded_border(fx, y + 5, 110, 20, 3,
            if self.editing { 0x00FF8020 } else { 0x00C0B0A0 });
        r.draw_text(&self.name, fx + 5, y + 10, 0x00302018, 1);
        if self.editing { r.draw_text("_", fx + 5 + self.name.len() * 7, y + 10, 0x00FF8020, 1); }
        r.draw_text(".BMP", fx + 112, y + 10, 0x00806040, 1);
        r.fill_rounded_glossy(fx, y + 30, 46, 20, 3, 0x00F0E4D8, 0x00D0C0B0);
        r.draw_text("Ac", fx + 14, y + 36, 0x00604838, 1);
        r.fill_rounded_glossy(fx + 52, y + 30, 62, 20, 3, 0x00FF9030, 0x00C06010);
        r.draw_text("Kaydet", fx + 60, y + 36, 0x00FFFFFF, 1);

        if let Some(m) = &self.msg {
            let mc = if m.starts_with("HATA") { 0x00C02020 } else { 0x00308030 };
            r.draw_text(m, fx + 122, y + 36, mc, 1);
        }

        // ==== Kanvas ====
        let cx = x as i32 + 8;
        let cy = y as i32 + TOOL_H + 4;
        r.fill_rect((cx - 1) as usize, (cy - 1) as usize, CANW + 2, CANH + 2, 0x00806040);
        r.draw_image(cx, cy, CANW, CANH, &self.canvas);
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Click { x, y } => {
                // arac cubugu
                if *y < TOOL_H {
                    // palet
                    if *y >= 5 && *y < 23 {
                        let i = (*x - 6) / 22;
                        if i >= 0 && i < 10 && (*x - 6) % 22 < 18 {
                            self.color = PAL[i as usize];
                            self.eraser = false;
                            return true;
                        }
                        // ad kutusu
                        if *x >= 230 && *x < 340 { self.editing = true; return true; }
                    }
                    if *y >= 30 && *y < 50 {
                        let i = (*x - 6) / 24;
                        if i >= 0 && i < 3 && *x < 80 {
                            self.size = [1, 3, 6][i as usize];
                            return true;
                        }
                        if *x >= 86 && *x < 134 { self.eraser = !self.eraser; return true; }
                        if *x >= 138 && *x < 196 {
                            self.canvas.iter_mut().for_each(|p| *p = 0x00FFFFFF);
                            return true;
                        }
                        if *x >= 230 && *x < 276 { self.editing = false; self.open(); return true; }
                        if *x >= 282 && *x < 344 { self.editing = false; self.save(); return true; }
                    }
                    self.editing = false;
                    return true;
                }
                // kanvas: nokta koy + stroke baslat
                self.editing = false;
                let px = *x - 8;
                let py = *y - TOOL_H - 4;
                if px >= 0 && py >= 0 && (px as usize) < CANW && (py as usize) < CANH {
                    self.cline(px, py, px, py);
                    self.lx = px; self.ly = py;
                    return true;
                }
                self.lx = -1;
                false
            }
            AppEvent::Drag { x, y } => {
                let px = *x - 8;
                let py = *y - TOOL_H - 4;
                if px >= 0 && py >= 0 && (px as usize) < CANW && (py as usize) < CANH {
                    if self.lx >= 0 { self.cline(self.lx, self.ly, px, py); }
                    else { self.cline(px, py, px, py); }
                    self.lx = px; self.ly = py;
                    return true;
                }
                self.lx = -1;
                false
            }
            AppEvent::Key { ch } if self.editing => {
                match *ch {
                    '\n' => self.editing = false,
                    '\u{8}' => { self.name.pop(); }
                    c if c.is_ascii_alphanumeric() && self.name.len() < 8 => {
                        self.name.push(c.to_ascii_uppercase());
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }
}
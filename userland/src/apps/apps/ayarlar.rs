use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

const CATS: [&str; 4] = ["Gorunum", "Sistem", "Ses", "Guc"];
const COLORS: [u32; 8] = [
    0x00CC5510, 0x00108ACC, 0x0010A040, 0x00803090,
    0x00C02020, 0x00C0A010, 0x00202830, 0x00507080,
];
const LEFT_W: usize = 120;

pub struct Ayarlar {
    cat: usize,
    user: String,
    input: String,
    editing: bool,
    info: Vec<(String, String)>,
    msg: Option<&'static str>,
    vw: usize, vh: usize,
}

impl Ayarlar {
    pub fn new() -> Self {
        let mut a = Ayarlar {
            cat: 0, user: String::new(), input: String::new(),
            editing: false, info: Vec::new(), msg: None, vw: 0, vh: 0,
        };
        a.refresh();
        a
    }
    fn refresh(&mut self) {
        self.info.clear();
        let mut dump = vec![0u8; 8192];
        let n = syscall::sys_reg_list(&mut dump) as usize;
        if let Ok(t) = core::str::from_utf8(&dump[..n.min(8192)]) {
            for line in t.lines() {
                let l = line.trim();
                let grab = |pfx: &str| l.strip_prefix(pfx).map(String::from);
                if let Some(v) = grab("Sistem/Ad=str:") { self.info.push((String::from("Sistem Adi"), v)); }
                if let Some(v) = grab("Sistem/Surum=str:") { self.info.push((String::from("Surum"), v)); }
                if let Some(v) = grab("Oturum/AktifKullanici=str:") {
                    self.user = v.clone();
                    self.info.push((String::from("Kullanici"), v));
                }
            }
        }
        let mut si: [u32; 4] = [0; 4];
        syscall::sys_sysinfo(&mut si);
        self.info.push((String::from("Toplam RAM"), format!("{} MB", si[0])));
    }
}

impl App for Ayarlar {
    fn title(&self) -> &'static str { "Ayarlar" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w; self.vh = h;
        r.fill_rect(x, y, w, h, 0x00FBF8F5);

        // Left category panel
        r.fill_rect(x, y, LEFT_W, h, 0x00F0E6DA);
        for (i, c) in CATS.iter().enumerate() {
            let iy = y + 10 + i * 34;
            if i == self.cat {
                r.fill_rounded(x + 4, iy - 4, LEFT_W - 8, 26, 4, 0x00FF8020);
                r.draw_text(c, x + 14, iy + 2, 0x00FFFFFF, 1);
            } else {
                r.draw_text(c, x + 14, iy + 2, 0x00604838, 1);
            }
        }

        let cx = x + LEFT_W + 14;
        match self.cat {
            0 => {
                r.draw_text("Masaustu Rengi", cx, y + 14, 0x00804010, 2);
                for (i, c) in COLORS.iter().enumerate() {
                    let sx = cx + (i % 4) * 52;
                    let sy = y + 44 + (i / 4) * 52;
                    r.fill_rounded_glossy(sx, sy, 44, 44, 6, *c, *c);
                    r.draw_rounded_border(sx, sy, 44, 44, 6, 0x00806040);
                }
                r.draw_text("Renge tiklayin. Masaustu yenilendiginde uygulanir.",
                    cx, y + 160, 0x00A09080, 1);
            }
            1 => {
                r.draw_text("Sistem Bilgisi", cx, y + 14, 0x00804010, 2);
                let mut iy = y + 44;
                for (k, v) in self.info.iter() {
                    r.draw_text(k, cx, iy, 0x00806040, 1);
                    r.draw_text(v, cx + 110, iy, 0x00403028, 1);
                    iy += 22;
                }
                iy += 8;
                r.draw_text("Kullanici adi degistir:", cx, iy, 0x00806040, 1);
                iy += 20;
                r.fill_rounded(cx, iy, 180, 26, 4, 0x00FFFFFF);
                r.draw_rounded_border(cx, iy, 180, 26, 4,
                    if self.editing { 0x00FF8020 } else { 0x00C0B0A0 });
                let shown = if self.editing { &self.input } else { &self.user };
                r.draw_text(shown, cx + 8, iy + 8, 0x00302018, 1);
                if self.editing { r.draw_text("_", cx + 8 + shown.len() * 7, iy + 8, 0x00FF8020, 1); }
                r.fill_rounded_glossy(cx + 190, iy, 70, 26, 4, 0x00FF9030, 0x00C06010);
                r.draw_text("Kaydet", cx + 200, iy + 8, 0x00FFFFFF, 1);
                if let Some(m) = self.msg { r.draw_text(m, cx, iy + 36, 0x00308030, 1); }
            }
            2 => {
                r.draw_text("Ses", cx, y + 14, 0x00804010, 2);
                r.fill_rounded_glossy(cx, y + 50, 130, 32, 5, 0x00FF9030, 0x00C06010);
                r.draw_text("Test Sesi Cal", cx + 14, y + 60, 0x00FFFFFF, 1);
                r.fill_rounded_glossy(cx + 140, y + 50, 90, 32, 5, 0x00F05038, 0x00B02818);
                r.draw_text("Durdur", cx + 158, y + 60, 0x00FFFFFF, 1);
            }
            _ => {
                r.draw_text("Guc Secenekleri", cx, y + 14, 0x00804010, 2);
                r.fill_rounded_glossy(cx, y + 50, 130, 34, 5, 0x00F05038, 0x00B02818);
                r.draw_text("Bilgisayari Kapat", cx + 8, y + 61, 0x00FFFFFF, 1);
                r.fill_rounded_glossy(cx, y + 94, 130, 34, 5, 0x00FF9030, 0x00C06010);
                r.draw_text("Yeniden Baslat", cx + 14, y + 105, 0x00FFFFFF, 1);
            }
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Click { x, y } => {
                // kategori
                if *x < LEFT_W as i32 {
                    let idx = ((*y - 6) / 34) as usize;
                    if idx < CATS.len() { self.cat = idx; self.editing = false; self.msg = None; }
                    return true;
                }
                let cx = LEFT_W as i32 + 14;
                match self.cat {
                    0 => {
                        for i in 0..8i32 {
                            let sx = cx + (i % 4) * 52;
                            let sy = 44 + (i / 4) * 52;
                            if *x >= sx && *x < sx + 44 && *y >= sy && *y < sy + 44 {
                                syscall::sys_set_desktop_color(COLORS[i as usize]);
                                return true;
                            }
                        }
                    }
                    1 => {
                        // giris kutusu bolgesi (info sayisina gore dinamik)
                        let iy = 44 + self.info.len() as i32 * 22 + 28;
                        if *y >= iy && *y < iy + 26 {
                            if *x >= cx && *x < cx + 180 {
                                self.editing = true;
                                self.input = self.user.clone();
                                return true;
                            }
                            if *x >= cx + 190 && *x < cx + 260 {
                                let name = if self.editing { self.input.clone() } else { self.user.clone() };
                                if !name.is_empty() {
                                    syscall::sys_reg_set_line(
                                        &format!("Oturum/AktifKullanici=str:{}", name));
                                    self.user = name;
                                    self.editing = false;
                                    self.msg = Some("Kaydedildi.");
                                    self.refresh();
                                }
                                return true;
                            }
                        }
                        self.editing = false;
                        return true;
                    }
                    2 => {
                        if *y >= 50 && *y < 82 {
                            if *x >= cx && *x < cx + 130 { syscall::sys_play_startup(); return true; }
                            if *x >= cx + 140 && *x < cx + 230 { syscall::sys_stop_sound(); return true; }
                        }
                    }
                    _ => {
                        if *x >= cx && *x < cx + 130 {
                            if *y >= 50 && *y < 84 { syscall::sys_power(0); }
                            if *y >= 94 && *y < 128 { syscall::sys_power(1); }
                        }
                    }
                }
                false
            }
            AppEvent::Key { ch } if self.editing => {
                match *ch {
                    '\n' => { self.editing = false; }
                    '\u{8}' => { self.input.pop(); }
                    c if c.is_ascii_alphanumeric() && self.input.len() < 16 => self.input.push(c),
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }
}
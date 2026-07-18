use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::apps::bmp;
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

pub struct Resim {
    path: String,
    img: Option<(usize, usize, Vec<u32>)>,
    err: Option<&'static str>,
    loaded: bool,
    fit: bool,
    vw: usize, vh: usize,
}

impl Resim {
    pub fn new(path: String) -> Self {
        Resim { path, img: None, err: None, loaded: false, fit: true, vw: 0, vh: 0 }
    }
    fn load(&mut self) {
        self.loaded = true;
        if self.path.is_empty() { self.err = Some("Dosya yok. Gezgin'den bir .BMP acin."); return; }
        let mut buf = vec![0u8; 512 * 1024];
        let n = syscall::sys_read_file(&self.path, &mut buf) as usize;
        if n == 0 { self.err = Some("Dosya okunamadi."); return; }
        match bmp::decode(&buf[..n.min(buf.len())]) {
            Some(t) => self.img = Some(t),
            None => self.err = Some("BMP cozulemedi (24/32-bit sikistirmasiz olmali)."),
        }
    }
}

impl App for Resim {
    fn title(&self) -> &'static str { "Resim Goruntuleyici" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w; self.vh = h;
        if !self.loaded { self.load(); }
        if h < 40 { return; }
        let info_h = 22;
        let ah = h - info_h; // resim alani
        r.checker(x, y, w, ah);

        match (&self.img, self.err) {
            (Some((iw, ih, px)), _) => {
                let (dw, dh) = if self.fit && (*iw > w || *ih > ah) {
                    let s1 = w * 1000 / iw;
                    let s2 = ah * 1000 / ih;
                    let s = s1.min(s2);
                    ((iw * s / 1000).max(1), (ih * s / 1000).max(1))
                } else { (*iw, *ih) };
                let dx = x as i32 + ((w as i32 - dw as i32) / 2).max(0);
                let dy = y as i32 + ((ah as i32 - dh as i32) / 2).max(0);
                if dw == *iw && dh == *ih { r.draw_image(dx, dy, *iw, *ih, px); }
                else { r.draw_image_scaled(dx, dy, dw, dh, *iw, *ih, px); }

                r.fill_rect(x, y + ah, w, info_h, 0x00F0E8E0);
                let name = self.path.rsplit('/').next().unwrap_or("?");
                let mode = if self.fit { "Sigdir" } else { "%100" };
                r.draw_text(&format!("{}  {}x{}  [{}] (tikla: degistir)", name, iw, ih, mode),
                    x + 8, y + ah + 6, 0x00604838, 1);
            }
            (_, Some(e)) => {
                r.fill_rect(x, y, w, h, 0x00FBF8F5);
                r.draw_text("Resim acilamadi", x + 12, y + 16, 0x00A03010, 2);
                r.draw_text(e, x + 12, y + 44, 0x00604838, 1);
            }
            _ => {}
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        if let AppEvent::Click { .. } = ev {
            if self.img.is_some() { self.fit = !self.fit; return true; }
        }
        false
    }
}
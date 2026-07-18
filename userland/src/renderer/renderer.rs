#[path = "font.rs"]
mod font;
use font::FONT;

pub struct Renderer {
    fb: *mut u32,
    pub back: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
}

impl Renderer {
    pub fn new(fb_base: u64, width: u64, height: u64, stride: u64, back_base: u64) -> Renderer {
        Renderer {
            fb: fb_base as *mut u32,
            back: back_base as *mut u32,
            width: width as usize,
            height: height as usize,
            stride: stride as usize,
        }
    }

    #[inline]
    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height { return; }
        // RGB -> BGR cevir (framebuffer BGR ise)
        let r = (color >> 16) & 0xFF;
        let g = (color >> 8) & 0xFF;
        let b = color & 0xFF;
        let bgr = (b << 16) | (g << 8) | r;
        unsafe {
            *self.back.add(y * self.stride + x) = bgr;
        }
    }

    pub fn clear(&self, color: u32) {
        let total = self.stride * self.height;
        unsafe {
            // Teker teker put_pixel yerine inanılmaz hızlı blok doldurma (memset)
            core::slice::from_raw_parts_mut(self.back, total).fill(color);
        }
    }

    pub fn fill_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let start_x = x.min(self.width);
        let start_y = y.min(self.height);
        let end_x = (x + w).min(self.width);
        let end_y = (y + h).min(self.height);
        let draw_w = end_x - start_x;

        if draw_w == 0 || start_y == end_y { return; }

        unsafe {
            for dy in start_y..end_y {
                let ptr = self.back.add(dy * self.stride + start_x);
                core::slice::from_raw_parts_mut(ptr, draw_w).fill(color);
            }
        }
    }

    pub fn fill_gradient(&self, x: usize, y: usize, w: usize, h: usize, top: u32, bottom: u32) {
        let start_x = x.min(self.width);
        let start_y = y.min(self.height);
        let end_x = (x + w).min(self.width);
        let end_y = (y + h).min(self.height);
        let draw_w = end_x - start_x;
        if draw_w == 0 || start_y == end_y { return; }

        let hh = if h == 0 { 1 } else { h as i32 };
        let tr = ((top >> 16) & 0xFF) as i32;
        let tg = ((top >> 8) & 0xFF) as i32;
        let tb = (top & 0xFF) as i32;
        let br = ((bottom >> 16) & 0xFF) as i32;
        let bg = ((bottom >> 8) & 0xFF) as i32;
        let bb = (bottom & 0xFF) as i32;

        for dy in start_y..end_y {
            let t = (dy - y) as i32;
            // signed hesap (underflow yok)
            let r = (tr + (br - tr) * t / hh) as u32;
            let g = (tg + (bg - tg) * t / hh) as u32;
            let b = (tb + (bb - tb) * t / hh) as u32;
            let color = (r << 16) | (g << 8) | b;
            unsafe {
                let ptr = self.back.add(dy * self.stride + start_x);
                core::slice::from_raw_parts_mut(ptr, draw_w).fill(color);
            }
        }
    }

    // Tek karakter ciz. scale = boyut carpani (1 = orijinal, 2 = 2x buyuk)
    pub fn draw_char(&self, ch: char, x: usize, y: usize, color: u32, scale: usize) {
        let idx = ch as usize;
        if idx >= 128 { return; } // sadece ASCII
        let glyph = &FONT[idx];
        // FONT: her karakter 8 satir, her satir 8 bit (8x8 grid)
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if (bits >> (7 - col)) & 1 == 1 {
                    // scale kadar buyut (her piksel scale x scale blok)
                    for sy in 0..scale {
                        for sx in 0..scale {
                            self.put_pixel(x + col * scale + sx, y + row * scale + sy, color);
                        }
                    }
                }
            }
        }
    }

    // Metin ciz (soldan saga)
    pub fn draw_text(&self, text: &str, x: usize, y: usize, color: u32, scale: usize) {
        let mut cx = x;
        for ch in text.chars() {
            if ch == '\n' { continue; }
            if ch == ' ' {
                cx += 6 * scale; // bosluk
                continue;
            }
            self.draw_char(ch, cx, y, color, scale);
            cx += 7 * scale; // karakter genisligi + bosluk (daha sik ama net)
        }
    }

    // PRESENT: back buffer'i framebuffer'a kopyala (ekrana bas)
    pub fn present(&self) {
        let total = self.stride * self.height;
        unsafe {
            // Manuel for döngüsü yerine tek komutta ultra hızlı memcpy
            core::ptr::copy_nonoverlapping(self.back, self.fb, total);
        }
    }

    pub fn present_rect(&self, x: usize, y: usize, w: usize, h: usize) {
        let start_x = x.min(self.width);
        let start_y = y.min(self.height);
        let end_x = (x + w).min(self.width);
        let end_y = (y + h).min(self.height);
        let draw_w = end_x - start_x;

        if draw_w == 0 || start_y == end_y { return; }

        unsafe {
            for dy in start_y..end_y {
                let offset = dy * self.stride + start_x;
                core::ptr::copy_nonoverlapping(
                    self.back.add(offset),
                    self.fb.add(offset),
                    draw_w,
                );
            }
        }
    }

    // === AERO YETENEKLERI ===

    // Alpha blend: src rengini back buffer'daki mevcut renkle karistir (alpha 0-255)
    #[inline]
    pub fn blend_pixel(&self, x: usize, y: usize, src: u32, alpha: u32) {
        if x >= self.width || y >= self.height { return; }
        unsafe {
            let ptr = self.back.add(y * self.stride + x);
            let dst = *ptr;
            let inv = 255 - alpha;
            let sr = (src >> 16) & 0xFF;
            let sg = (src >> 8) & 0xFF;
            let sb = src & 0xFF;
            let dr = (dst >> 16) & 0xFF;
            let dg = (dst >> 8) & 0xFF;
            let db = dst & 0xFF;
            let r = (sr * alpha + dr * inv) / 255;
            let g = (sg * alpha + dg * inv) / 255;
            let b = (sb * alpha + db * inv) / 255;
            *ptr = (r << 16) | (g << 8) | b;
        }
    }

    // Yari saydam dikdortgen (cam efekti)
    pub fn fill_rect_alpha(&self, x: usize, y: usize, w: usize, h: usize, color: u32, alpha: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.blend_pixel(x + dx, y + dy, color, alpha);
            }
        }
    }

    // GLOSSY: Aero cam parlamasi. Ust yari parlak gradient, alt yari koyu, ortada highlight cizgisi
    pub fn fill_glossy(&self, x: usize, y: usize, w: usize, h: usize, base_top: u32, base_bottom: u32) {
        let half = h / 2;
        // Ust yari: acik -> orta (parlak cam)
        self.fill_gradient(x, y, w, half, base_top, base_bottom);
        // Alt yari: orta -> koyu (mat)
        let darker = darken(base_bottom, 40);
        self.fill_gradient(x, y + half, w, h - half, base_bottom, darker);
        // Ust kenar highlight (ince beyaz parlama, yari saydam)
        self.fill_rect_alpha(x, y, w, 2, 0x00FFFFFF, 90);
        // Ortadaki cam cizgisi (hafif parlak)
        self.fill_rect_alpha(x, y + half - 1, w, 1, 0x00FFFFFF, 40);
    }

    // ROUNDED RECT: yuvarlak koseli dolu dikdortgen
    pub fn fill_rounded(&self, x: usize, y: usize, w: usize, h: usize, radius: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                if in_rounded(dx, dy, w, h, radius) {
                    self.put_pixel(x + dx, y + dy, color);
                }
            }
        }
    }

    // ROUNDED GLOSSY: yuvarlak koseli + cam parlamasi (Win7 buton/baslik)
    pub fn fill_rounded_glossy(&self, x: usize, y: usize, w: usize, h: usize, radius: usize, top: u32, bottom: u32) {
        let half = if h / 2 == 0 { 1 } else { h / 2 };
        for dy in 0..h {
            // satir rengi: ust yari top->bottom parlak, alt yari bottom->biraz koyu
            let color = if dy < half {
                lerp_color(top, bottom, dy as u32, half as u32)
            } else {
                let darker = darken(bottom, 30);
                let bh = if h - half == 0 { 1 } else { h - half };
                lerp_color(bottom, darker, (dy - half) as u32, bh as u32)
            };
            for dx in 0..w {
                if in_rounded(dx, dy, w, h, radius) {
                    self.put_pixel(x + dx, y + dy, color);
                }
            }
        }
        // ust kenar highlight (ince parlama)
        for dx in 0..w {
            if in_rounded(dx, 1, w, h, radius) {
                self.blend_pixel(x + dx, y + 1, 0x00FFFFFF, 70);
            }
        }
    }

    // Yuvarlak koseli cerceve (kenarlik cizgisi)
    pub fn draw_rounded_border(&self, x: usize, y: usize, w: usize, h: usize, radius: usize, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                if in_rounded(dx, dy, w, h, radius) && is_edge(dx, dy, w, h, radius) {
                    self.put_pixel(x + dx, y + dy, color);
                }
            }
        }
    }

    pub fn save_rect(&self, x: usize, y: usize, w: usize, h: usize, out: &mut [u32]) {
        for row in 0..h {
            let sy = y + row;
            if sy >= self.height || x >= self.width { break; }
            let cw = w.min(self.width - x);
            unsafe {
                core::ptr::copy_nonoverlapping(self.back.add(sy * self.stride + x), out.as_mut_ptr().add(row * w), cw);
            }
        }
    }
    pub fn restore_rect(&self, x: usize, y: usize, w: usize, h: usize, data: &[u32]) {
        for row in 0..h {
            let sy = y + row;
            if sy >= self.height || x >= self.width { break; }
            let cw = w.min(self.width - x);
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr().add(row * w), self.back.add(sy * self.stride + x), cw);
            }
        }
    }
    // Hizli blok basma (Paint tuvali: 104bin put_pixel yerine satir kopyasi)
    pub fn blit(&self, x: usize, y: usize, w: usize, h: usize, pixels: &[u32]) {
        for row in 0..h {
            let sy = y + row;
            if sy >= self.height || x >= self.width { break; }
            let cw = w.min(self.width - x);
            unsafe {
                core::ptr::copy_nonoverlapping(pixels.as_ptr().add(row * w), self.back.add(sy * self.stride + x), cw);
            }
        }
    }

    // ================= GRAFIK EKLERI =================
    // Piksel dizisini blit et (kirpmali)
    pub fn draw_image(&self, x: i32, y: i32, iw: usize, ih: usize, px: &[u32]) {
        for row in 0..ih {
            let dy = y + row as i32;
            if dy < 0 || dy as usize >= self.height { continue; }
            for col in 0..iw {
                let dx = x + col as i32;
                if dx < 0 || dx as usize >= self.width { continue; }
                let c = px[row * iw + col];
                if c & 0xFF00_0000 == 0xFF00_0000 { continue; } // FF alfa = seffaf
                self.put_pixel(dx as usize, dy as usize, c);
            }
        }
    }

    // En-yakin komsu olcekleme ile blit
    pub fn draw_image_scaled(&self, x: i32, y: i32, dw: usize, dh: usize,
                             iw: usize, ih: usize, px: &[u32]) {
        if dw == 0 || dh == 0 || iw == 0 || ih == 0 { return; }
        for row in 0..dh {
            let sy = row * ih / dh;
            let dy = y + row as i32;
            if dy < 0 || dy as usize >= self.height { continue; }
            for col in 0..dw {
                let sx = col * iw / dw;
                let dx = x + col as i32;
                if dx < 0 || dx as usize >= self.width { continue; }
                self.put_pixel(dx as usize, dy as usize, px[sy * iw + sx]);
            }
        }
    }

    // Satranc tahtasi arkaplan (resim goruntuleyici)
    pub fn checker(&self, x: usize, y: usize, w: usize, h: usize) {
        for cy in (0..h).step_by(8) {
            for cx in (0..w).step_by(8) {
                let c = if ((cx / 8) + (cy / 8)) % 2 == 0 { 0x00E8E8E8 } else { 0x00C8C8C8 };
                self.fill_rect(x + cx, y + cy, 8.min(w - cx), 8.min(h - cy), c);
            }
        }
    }

    pub fn fill_circle(&self, cx: i32, cy: i32, rad: i32, color: u32) {
        for dy in -rad..=rad {
            for dx in -rad..=rad {
                if dx * dx + dy * dy <= rad * rad {
                    let px = cx + dx; let py = cy + dy;
                    if px >= 0 && py >= 0 && (px as usize) < self.width && (py as usize) < self.height {
                        self.put_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
}

// === Yardimci fonksiyonlar ===

// Bir nokta yuvarlak dikdortgenin icinde mi?
fn in_rounded(dx: usize, dy: usize, w: usize, h: usize, radius: usize) -> bool {
    let r = radius as i32;
    let x = dx as i32;
    let y = dy as i32;
    let wi = w as i32;
    let hi = h as i32;

    // Hangi kosedeyiz?
    let (cx, cy) = if x < r && y < r {
        (r, r) // sol ust
    } else if x >= wi - r && y < r {
        (wi - r - 1, r) // sag ust
    } else if x < r && y >= hi - r {
        (r, hi - r - 1) // sol alt
    } else if x >= wi - r && y >= hi - r {
        (wi - r - 1, hi - r - 1) // sag alt
    } else {
        return true; // kose disindaki duz alan
    };

    let ddx = x - cx;
    let ddy = y - cy;
    ddx * ddx + ddy * ddy <= r * r
}

// Kenar pikseli mi? (cerceve cizimi icin)
fn is_edge(dx: usize, dy: usize, w: usize, h: usize, radius: usize) -> bool {
    // icerdeyse ama bir komsusu disardaysa kenardir
    !in_rounded(dx.wrapping_sub(1).min(w), dy, w, h, radius)
        || !in_rounded(dx + 1, dy, w, h, radius)
        || !in_rounded(dx, dy.wrapping_sub(1).min(h), w, h, radius)
        || !in_rounded(dx, dy + 1, w, h, radius)
}

// Bir satirda hic cizilecek piksel var mi? (optimizasyon)
fn row_has_pixels(dy: usize, _w: usize, h: usize, radius: usize) -> bool {
    dy < h && (dy >= radius || dy < h) // basit, hep true gibi ama sinir guvenli
        && dy < h
}

fn lerp_color(a: u32, b: u32, t: u32, max: u32) -> u32 {
    let ar = ((a >> 16) & 0xFF) as i32;
    let ag = ((a >> 8) & 0xFF) as i32;
    let ab = (a & 0xFF) as i32;
    let br = ((b >> 16) & 0xFF) as i32;
    let bg = ((b >> 8) & 0xFF) as i32;
    let bb = (b & 0xFF) as i32;
    let tt = t as i32;
    let mx = if max == 0 { 1 } else { max as i32 };
    // signed hesap: negatif fark sorun olmaz
    let r = (ar + (br - ar) * tt / mx) as u32;
    let g = (ag + (bg - ag) * tt / mx) as u32;
    let b = (ab + (bb - ab) * tt / mx) as u32;
    (r << 16) | (g << 8) | b
}

// Rengi karart (amount kadar)
fn darken(color: u32, amount: u32) -> u32 {
    let r = ((color >> 16) & 0xFF).saturating_sub(amount);
    let g = ((color >> 8) & 0xFF).saturating_sub(amount);
    let b = (color & 0xFF).saturating_sub(amount);
    (r << 16) | (g << 8) | b
}
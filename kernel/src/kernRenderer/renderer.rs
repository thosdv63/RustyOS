use crate::kernRenderer::font::FONT;

pub struct Renderer {
    base: *mut u32,      
    pub width: usize,        
    pub height: usize,       
    pub stride: usize,       
    cursor_x: usize,     
    cursor_y: usize,     
    scale: usize,        
    color: u32,          
}

impl Renderer {
    pub fn new(base: *mut u8, width: usize, height: usize, stride: usize) -> Renderer {
        Renderer {
            base: base as *mut u32,
            width,
            height,
            stride,
            cursor_x: 0,
            cursor_y: 0,
            scale: 2,            
            color: 0x00FFFFFF,   
        }
    }

    pub fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    pub fn put_pixel(&self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height { return; }
        unsafe {
            *self.base.add(y * self.stride + x) = color;
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x >= self.width || y >= self.height { return 0; }
        unsafe {
            core::ptr::read_volatile(self.base.add(y * self.stride + x))
        }
    }   

    pub fn clear(&mut self, color: u32) {
        let total = self.stride * self.height;
        unsafe {
            core::slice::from_raw_parts_mut(self.base, total).fill(color);
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
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
                let ptr = self.base.add(dy * self.stride + start_x);
                core::slice::from_raw_parts_mut(ptr, draw_w).fill(color);
            }
        }
    }

    pub fn draw_sprite(&self, x: usize, y: usize, w: usize, h: usize, data: &[u32]) {
        let start_x = x.min(self.width);
        let start_y = y.min(self.height);
        let end_x = (x + w).min(self.width);
        let end_y = (y + h).min(self.height);
        let draw_w = end_x - start_x;

        if draw_w == 0 || start_y == end_y { return; }

        unsafe {
            for dy in 0..(end_y - start_y) {
                let dest_ptr = self.base.add((start_y + dy) * self.stride + start_x);
                let src_ptr = data.as_ptr().add(dy * w);
                core::ptr::copy_nonoverlapping(src_ptr, dest_ptr, draw_w);
            }
        }
    }

    pub fn put_char(&mut self, c: char) {
        if c == '\n' {
            self.new_line();
            return;
        }
        if c == '\r' {
            self.cursor_x = 0;
            return;
        }

        let code = c as usize;
        if code < 32 || code > 126 {
            self.advance();
            return;
        }

        let glyph = &FONT[code - 32];

        for row in 0..8 {
            let bits = glyph[row];
            for col in 0..8 {
                if (bits >> col) & 1 == 1 {
                    for dy in 0..self.scale {
                        for dx in 0..self.scale {
                            let px = self.cursor_x + col * self.scale + dx;
                            let py = self.cursor_y + row * self.scale + dy;
                            self.put_pixel(px, py, self.color);
                        }
                    }
                }
            }
        }

        self.advance();
    }

    fn advance(&mut self) {
        self.cursor_x += 8 * self.scale + self.scale;
        if self.cursor_x + 8 * self.scale >= self.width {
            self.new_line();
        }
    }

    fn new_line(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 8 * self.scale + self.scale;
        if self.cursor_y + 8 * self.scale >= self.height {
            self.cursor_y = 0;
        }
    }

    pub fn text(&mut self, s: &str) {
        for c in s.chars() {
            self.put_char(c);
        }
    }

    pub fn draw_rect(&self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        for dx in 0..w {
            self.put_pixel(x + dx, y, color);
            self.put_pixel(x + dx, y + h - 1, color);
        }
        for dy in 0..h {
            self.put_pixel(x, y + dy, color);
            self.put_pixel(x + w - 1, y + dy, color);
        }
    }

    pub fn text_at(&mut self, x: usize, y: usize, s: &str) {
        self.cursor_x = x;
        self.cursor_y = y;
        self.text(s);
    }

    pub fn draw_hline(&self, x: usize, y: usize, length: usize, color: u32) {
        for dx in 0..length {
            self.put_pixel(x + dx, y, color);
        }
    }

    pub fn draw_vline(&self, x: usize, y: usize, length: usize, color: u32) {
        for dy in 0..length {
            self.put_pixel(x, y + dy, color);
        }
    }

    pub fn draw_line(&self, x0: usize, y0: usize, x1: usize, y1: usize, color: u32) {
        let mut x0 = x0 as isize;
        let mut y0 = y0 as isize;
        let x1 = x1 as isize;
        let y1 = y1 as isize;

        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.put_pixel(x0 as usize, y0 as usize, color);
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}

impl core::fmt::Write for Renderer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.text(s);
        Ok(())
    }
}
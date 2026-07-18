use crate::renderer::Renderer;
use crate::ui::theme;
use alloc::string::String;

#[derive(Clone, Copy, PartialEq)]
pub enum WinState {
    Normal,
    Maximized,
    Minimized,
}

pub struct Window {
    pub id: u32,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    // Maximize oncesi konumu sakla (geri donmek icin)
    pub saved_x: i32,
    pub saved_y: i32,
    pub saved_w: i32,
    pub saved_h: i32,
    pub state: WinState,
    pub app_id: u32, // hangi uygulama (app manager kullanacak)
}

pub const TITLE_H: i32 = 32;
const BTN_SIZE: i32 = 24;
const BTN_GAP: i32 = 4;

// Pencere icindeki bolgeler (tiklamayi yorumlamak icin)
#[derive(PartialEq)]
pub enum Region {
    None,
    TitleBar,   // surukleme
    CloseBtn,
    MaxBtn,
    MinBtn,
    Body,
}

impl Window {
    pub fn new(id: u32, title: &str, x: i32, y: i32, w: i32, h: i32, app_id: u32) -> Window {
        Window {
            id, title: String::from(title),
            x, y, w, h,
            saved_x: x, saved_y: y, saved_w: w, saved_h: h,
            state: WinState::Normal,
            app_id,
        }
    }

    // Bir nokta pencerenin neresinde?
    pub fn region_at(&self, mx: i32, my: i32) -> Region {
        if mx < self.x || mx >= self.x + self.w || my < self.y || my >= self.y + self.h {
            return Region::None;
        }
        // Butonlar (sag ust)
        let by = self.y + 4;
        let close_x = self.x + self.w - BTN_SIZE - BTN_GAP;
        let max_x = close_x - BTN_SIZE - BTN_GAP;
        let min_x = max_x - BTN_SIZE - BTN_GAP;
        if my >= by && my < by + BTN_SIZE {
            if mx >= close_x && mx < close_x + BTN_SIZE { return Region::CloseBtn; }
            if mx >= max_x && mx < max_x + BTN_SIZE { return Region::MaxBtn; }
            if mx >= min_x && mx < min_x + BTN_SIZE { return Region::MinBtn; }
        }
        // Baslik cubugu (surukleme)
        if my < self.y + TITLE_H { return Region::TitleBar; }
        Region::Body
    }

    // Pencereyi ciz. active = on plandaki pencere mi (renk farki)
    pub fn draw(&self, r: &Renderer, active: bool) {
        if self.state == WinState::Minimized { return; }

        let x = self.x.max(0) as usize;
        let y = self.y.max(0) as usize;
        let w = self.w as usize;
        let h = self.h as usize;

        // Maximized'da yuvarlak kose YOK (hizli duz cizim)
        let maximized = self.state == WinState::Maximized;

        // Golge (maximized'da golge yok - zaten tam ekran)
        if !maximized {
            let sa = if active { 80 } else { 40 };
            r.fill_rect_alpha(x + 5, y + h, w, 4, 0x00000000, sa);      // alt kenar
            r.fill_rect_alpha(x + w, y + 5, 4, h - 1, 0x00000000, sa);  // sag kenar
        }

        // Govde
        if maximized {
            r.fill_rect(x, y, w, h, theme::WINDOW_BODY); // DUZ = hizli
        } else {
            r.fill_rounded(x, y, w, h, 8, theme::WINDOW_BODY);
            r.draw_rounded_border(x, y, w, h, 8, theme::WINDOW_BORDER);
        }

        // Baslik cubugu
        let (t_top, t_bot) = if active {
            (theme::TITLE_TOP, theme::TITLE_BOTTOM)
        } else {
            (theme::TITLE_INACTIVE_TOP, theme::TITLE_INACTIVE_BOTTOM)
        };
        if maximized {
            r.fill_gradient(x, y, w, TITLE_H as usize, t_top, t_bot); // DUZ glossy
            r.fill_rect_alpha(x, y, w, 2, 0x00FFFFFF, 90);
        } else {
            r.fill_rounded_glossy(x, y, w, TITLE_H as usize, 8, t_top, t_bot);
        }

        // Baslik metni
        r.draw_text(&self.title, x + 12, y + 9, 0x00301008, 2);
        r.draw_text(&self.title, x + 11, y + 8, theme::TITLE_TEXT, 2);

        self.draw_buttons(r);

        // Govde icerigi (placeholder)
        let body_y = y + TITLE_H as usize + 4;
        let body_h = h.saturating_sub(TITLE_H as usize + 8);
        r.fill_rect(x + 4, body_y, w - 8, body_h, 0x00FFFFFF);
    }

    fn draw_buttons(&self, r: &Renderer) {
        let by = (self.y + 4) as usize;
        let close_x = (self.x + self.w - BTN_SIZE - BTN_GAP) as usize;
        let max_x = (self.x + self.w - 2*BTN_SIZE - 2*BTN_GAP) as usize;
        let min_x = (self.x + self.w - 3*BTN_SIZE - 3*BTN_GAP) as usize;
        let s = BTN_SIZE as usize;

        // Kucult (-)
        r.fill_rounded_glossy(min_x, by, s, s, 4, 0x00FFA850, 0x00D86818);
        r.fill_rect(min_x + 6, by + s - 8, s - 12, 2, 0x00FFFFFF);

        // Buyut (kare)
        r.fill_rounded_glossy(max_x, by, s, s, 4, 0x00FFA850, 0x00D86818);
        r.draw_rounded_border(max_x + 6, by + 6, s - 12, s - 12, 1, 0x00FFFFFF);

        // Kapat (x, kirmizi)
        r.fill_rounded_glossy(close_x, by, s, s, 4, theme::CLOSE_TOP, theme::CLOSE_BOTTOM);
        r.draw_text("x", close_x + 8, by + 6, 0x00FFFFFF, 2);
    }
}
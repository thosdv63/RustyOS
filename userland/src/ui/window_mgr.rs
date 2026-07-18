use crate::renderer::Renderer;
use crate::ui::window::{Window, WinState, Region, TITLE_H};
use crate::ui::theme;
use alloc::vec::Vec;

pub struct WindowManager {
    pub windows: Vec<Window>, // sira = z-order (son = en ustte)
    pub drag_dirty: Option<(i32, i32, i32, i32)>,
    next_id: u32,
    // Surukleme durumu
    dragging: bool,
    drag_id: u32,
    drag_off_x: i32,
    drag_off_y: i32,
    prev_btn: bool,
    screen_w: i32,
    screen_h: i32,
}

impl WindowManager {
    pub fn new(screen_w: i32, screen_h: i32) -> WindowManager {
        WindowManager {
            windows: Vec::new(),
            drag_dirty: None,
            next_id: 1,
            dragging: false, drag_id: 0, drag_off_x: 0, drag_off_y: 0,
            prev_btn: false,
            screen_w, screen_h,
        }
    }

    // Yeni pencere ac (basamakli konum, ic ice gecmesin)
    pub fn open(&mut self, title: &str, app_id: u32) {
        let n = self.windows.len() as i32;
        let x = 120 + (n % 6) * 40; // her yeni pencere kaydir (cascade)
        let y = 80 + (n % 6) * 40;
        let w = 480;
        let h = 340;
        let id = self.next_id;
        self.next_id += 1;
        self.windows.push(Window::new(id, title, x, y, w, h, app_id));
    }

    // Pencere mesgul mu (surukleme aktif mi)
    pub fn is_busy(&self) -> bool {
        self.dragging
    }

    pub fn close_by_id(&mut self, id: u32) {
        if let Some(i) = self.windows.iter().position(|w| w.id == id) {
            self.windows.remove(i);
        }
    }

    pub fn over_any(&self, mx: i32, my: i32) -> bool {
        self.windows.iter().any(|w| w.state != WinState::Minimized
            && mx >= w.x && mx < w.x + w.w && my >= w.y && my < w.y + w.h)
    }
    fn taskbar_top(&self) -> i32 { self.screen_h - theme::TASKBAR_HEIGHT as i32 }

    // En ustteki (z-order son) pencere id'si
    pub fn active_id(&self) -> u32 {
        self.windows.last().map(|w| w.id).unwrap_or(0)
    }

    // Bir pencereyi en uste getir (z-order)
    fn bring_to_front(&mut self, idx: usize) {
        let win = self.windows.remove(idx);
        self.windows.push(win);
    }

    pub fn take_drag_dirty(&mut self) -> Option<(i32, i32, i32, i32)> { self.drag_dirty.take() }

    // Mouse olayi. Donus: true = yeniden cizim gerekli
    pub fn handle_mouse(&mut self, mx: i32, my: i32, btn: bool) -> bool {
        let prev = self.prev_btn;
        self.prev_btn = btn;
        let mut changed = false;

        // BASMA
        if btn && !prev {
            // ustten alta pencereleri tara (z-order tersten)
            let mut hit_idx: Option<usize> = None;
            let mut region = Region::None;
            let mut i = self.windows.len();
            while i > 0 {
                i -= 1;
                let reg = self.windows[i].region_at(mx, my);
                if reg != Region::None {
                    hit_idx = Some(i);
                    region = reg;
                    break;
                }
            }

            if let Some(idx) = hit_idx {
                // bu pencereyi one getir
                self.bring_to_front(idx);
                let last = self.windows.len() - 1;
                match region {
                    Region::CloseBtn => {
                        self.windows.remove(last);
                    }
                    Region::MaxBtn => {
                        self.toggle_max(last);
                    }
                    Region::MinBtn => {
                        self.windows[last].state = WinState::Minimized;
                    }
                    Region::TitleBar => {
                        self.dragging = true;
                        self.drag_id = self.windows[last].id;
                        self.drag_off_x = mx - self.windows[last].x;
                        self.drag_off_y = my - self.windows[last].y;
                    }
                    _ => {}
                }
                changed = true;
            }
        }

        // SURUKLEME
        if btn && prev && self.dragging {
            if let Some(win) = self.windows.iter_mut().find(|w| w.id == self.drag_id) {
                if win.state == WinState::Normal {
                    let mut nx = mx - self.drag_off_x;
                    let mut ny = my - self.drag_off_y;
                    // ekran disina cikmasin
                    if nx < 0 { nx = 0; }
                    if ny < 0 { ny = 0; }
                    if nx + win.w > self.screen_w { nx = self.screen_w - win.w; }
                    // taskbar'i gecmesin (baslik gorunur kalsin)
                    let tb = self.screen_h - theme::TASKBAR_HEIGHT as i32;
                    if ny + TITLE_H > tb { ny = tb - TITLE_H; }
                    let (ox, oy) = (win.x, win.y);
                    win.x = nx; win.y = ny;
                    let x0 = ox.min(nx).max(0);
                    let y0 = oy.min(ny).max(0);
                    self.drag_dirty = Some((x0, y0, ox.max(nx) + win.w + 10 - x0, oy.max(ny) + win.h + 10 - y0));
                    changed = true;
                }
            }
        }

        // BIRAKMA
        if !btn && prev {
            if self.dragging { changed = true; }
            self.dragging = false;
        }

        changed
    }

    // Buyut/geri al (taskbar'i kaplamadan)
    fn toggle_max(&mut self, idx: usize) {
        let tb = self.screen_h - theme::TASKBAR_HEIGHT as i32;
        let win = &mut self.windows[idx];
        if win.state == WinState::Maximized {
            // geri al
            win.x = win.saved_x; win.y = win.saved_y;
            win.w = win.saved_w; win.h = win.saved_h;
            win.state = WinState::Normal;
        } else {
            // kaydet + buyut (taskbar haric)
            win.saved_x = win.x; win.saved_y = win.y;
            win.saved_w = win.w; win.saved_h = win.h;
            win.x = 0; win.y = 0;
            win.w = self.screen_w;
            win.h = tb; // taskbar ustunde kal
            win.state = WinState::Maximized;
        }
    }

    // Taskbar ikonuna tiklaninca (minimized'i geri getir / one getir)
    pub fn restore_by_id(&mut self, id: u32) {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.windows[idx].state = WinState::Normal;
            self.bring_to_front(idx);
        }
    }

    // Tum pencereleri ciz (z-order: bastan sona)
    pub fn draw(&self, r: &Renderer) {
        let active = self.active_id();
        for win in self.windows.iter() {
            win.draw(r, win.id == active);
        }
    }

    // Taskbar'da acik pencere butonlarini ciz
    pub fn draw_taskbar_buttons(&self, r: &Renderer) {
        let ty = self.taskbar_top();
        let active = self.active_id();
        let size = theme::TASKBAR_HEIGHT as i32 - 12; // kare
        let mut bx = 88;
        let by = ty + 6;
        for win in self.windows.iter() {
            let is_active = win.id == active && win.state != WinState::Minimized;
            let (top, bot) = if is_active {
                (0x00FF9030, 0x00D06010)
            } else {
                (0x00603018, 0x00402010)
            };
            // kare glossy ikon
            r.fill_rounded_glossy(bx as usize, by as usize, size as usize, size as usize, 5, top, bot);
            // ic parlama
            r.fill_rect_alpha((bx+4) as usize, (by+3) as usize, (size-8) as usize, 3, 0x00FFFFFF, 70);
            // pencere bas harfi (ikon icinde, ortali)
            let ch = win.title.chars().next().unwrap_or('?');
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            r.draw_text(s, (bx + size/2 - 6) as usize, (by + size/2 - 8) as usize, 0x00FFFFFF, 2);
            // aktifse alt cizgi (Win7 tarzi)
            if is_active {
                r.fill_rect(bx as usize, (by + size - 2) as usize, size as usize, 2, 0x00FFD060);
            }
            bx += size + 6;
        }
    }

    pub fn taskbar_button_at(&self, mx: i32, my: i32) -> Option<u32> {
        let ty = self.taskbar_top();
        let size = theme::TASKBAR_HEIGHT as i32 - 12;
        let by = ty + 6;
        let mut bx = 88;
        for win in self.windows.iter() {
            if mx >= bx && mx < bx + size && my >= by && my < by + size {
                return Some(win.id);
            }
            bx += size + 6;
        }
        None
    }
}
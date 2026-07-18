use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::ui::app_mgr;
use crate::syscall;
use alloc::format;

pub struct GorevMgr { sel: i32, vw: usize }
impl GorevMgr { pub fn new() -> Self { GorevMgr { sel: -1, vw: 0 } } }

impl App for GorevMgr {
    fn title(&self) -> &'static str { "Gorevler" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w;
        r.fill_rect(x, y, w, h, 0x00FBF8F5);
        r.fill_gradient(x, y, w, 28, 0x00F0E8E0, 0x00E0D4C8);
        r.draw_text("Gorev Yoneticisi", x + 8, y + 8, 0x00804010, 1);
        r.fill_rounded_glossy(x + w - 96, y + 3, 90, 22, 4, 0x00F04020, 0x00A01808);
        r.draw_text("Sonlandir", x + w - 86, y + 8, 0x00FFFFFF, 1);

        // === CPU / RAM bandi ===
        let mut si: [u32; 4] = [0; 4];
        syscall::sys_sysinfo(&mut si);
        let (total, used, cpu) = (si[0].max(1), si[1].min(si[0]), si[2].min(100));
        let bw = (w - 190) / 2;
        // CPU
        r.draw_text("CPU", x + 8, y + 36, 0x00806040, 1);
        r.fill_rounded(x + 44, y + 33, bw, 14, 3, 0x00E8DED2);
        r.fill_rounded(x + 44, y + 33, (bw * cpu as usize / 100).max(2), 14, 3, 0x00FF8020);
        r.draw_text(&format!("%{}", cpu), x + 48 + bw, y + 36, 0x00403028, 1);
        // RAM
        let rx = x + 44 + bw + 60;
        r.draw_text("RAM", rx - 36, y + 36, 0x00806040, 1);
        r.fill_rounded(rx, y + 33, bw, 14, 3, 0x00E8DED2);
        r.fill_rounded(rx, y + 33, (bw * used as usize / total as usize).max(2), 14, 3, 0x00E06010);
        r.draw_text(&format!("{}/{} MB", used, total), rx + bw + 4, y + 36, 0x00403028, 1);

        r.draw_text("PID", x + 8, y + 58, 0x00806040, 1);
        r.draw_text("Uygulama", x + 60, y + 58, 0x00806040, 1);
        r.draw_text("Durum", x + w - 96, y + 58, 0x00806040, 1);
        let tasks = app_mgr::tasks();
        let mut ry = y + 76;
        for (i, (id, title)) in tasks.iter().enumerate() {
            if ry + 18 > y + h { break; }
            if i as i32 == self.sel {
                r.fill_rect_alpha(x + 4, ry.saturating_sub(2), w - 8, 18, 0x00FF9030, 60);
            }
            let mut b = [0u8; 12];
            r.draw_text(u32s(*id, &mut b), x + 8, ry, 0x00403028, 1);
            r.draw_text(title, x + 60, ry, 0x00403028, 1);
            r.draw_text("Calisiyor", x + w - 96, ry, 0x00308030, 1);
            ry += 20;
        }
        if tasks.is_empty() { r.draw_text("(acik gorev yok)", x + 8, y + 80, 0x00A09080, 1); }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        if let AppEvent::Click { x, y } = ev {
            let wv = self.vw as i32;
            if *y >= 3 && *y < 25 && *x >= wv - 96 && *x < wv - 6 {
                let tasks = app_mgr::tasks();
                if self.sel >= 0 && (self.sel as usize) < tasks.len() {
                    app_mgr::request_kill(tasks[self.sel as usize].0);
                    self.sel = -1;
                }
                return true;
            }
            if *y >= 30 && *y < 50 { return true; } // bant tiki = tazele
            if *y >= 76 {
                let idx = ((*y - 76) / 20) as usize;
                if idx < app_mgr::tasks().len() { self.sel = idx as i32; return true; }
                self.sel = -1;
                return true;
            }
        }
        false
    }
}

fn u32s(mut n: u32, buf: &mut [u8; 12]) -> &str {
    if n == 0 { buf[0] = b'0'; return core::str::from_utf8(&buf[..1]).unwrap_or("0"); }
    let mut i = 12;
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    let len = 12 - i;
    for j in 0..len { buf[j] = buf[i + j]; }
    core::str::from_utf8(&buf[..len]).unwrap_or("?")
}
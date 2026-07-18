use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;

pub struct Hakkinda { sayac: u32 }

impl Hakkinda {
    pub fn new() -> Self { Hakkinda { sayac: 0 } }
}

impl App for Hakkinda {
    fn title(&self) -> &'static str { "Hakkinda" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        r.fill_rect(x, y, w, h, 0x00FFF8F0);
        r.draw_text("Rusty OS", x + 16, y + 14, 0x00C04408, 3);
        r.draw_text("Surum 0.1 - Aero", x + 16, y + 44, 0x00805030, 1);
        // registry'den canli renk (kanit)
        let renk = syscall::sys_reg_get_id(0);
        r.fill_rect(x + 16, y + 64, 24, 24, renk);
        r.draw_text("Masaustu rengi (registry)", x + 48, y + 70, 0x00604020, 1);
        // sayac butonu (olay hatti testi)
        let bx = x + 16; let by = y + 104;
        r.fill_rounded_glossy(bx, by, 120, 32, 6, 0x00FF9030, 0x00C85810);
        let mut buf = [0u8; 12];
        let s = u32_to_str(self.sayac, &mut buf);
        r.draw_text("Tikla:", bx + 10, by + 9, 0x00FFFFFF, 2);
        r.draw_text(s, bx + 78, by + 9, 0x00FFFFFF, 2);
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Click { x, y } => {
                if *x >= 16 && *x < 136 && *y >= 104 && *y < 136 {
                    self.sayac += 1;
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

fn u32_to_str(mut n: u32, buf: &mut [u8; 12]) -> &str {
    if n == 0 { buf[0] = b'0'; return core::str::from_utf8(&buf[..1]).unwrap_or("0"); }
    let mut i = 12;
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    let len = 12 - i;
    let mut j = 0;
    while j < len { buf[j] = buf[i + j]; j += 1; }
    core::str::from_utf8(&buf[..len]).unwrap_or("?")
}
use crate::renderer::Renderer;
use crate::ui::theme;

// Aero R logosu (Rusty amblemi) - ortada duracak
// 16x16 maske, R harfi (boot animasyonundaki ile ayni ruhta)
const R_LOGO: [u16; 16] = [
    0b0111111111100000,
    0b0111111111111000,
    0b0111000000011100,
    0b0111000000001110,
    0b0111000000001110,
    0b0111000000011100,
    0b0111111111111000,
    0b0111111111100000,
    0b0111000011100000,
    0b0111000001110000,
    0b0111000000111000,
    0b0111000000011100,
    0b0111000000001110,
    0b0111000000000111,
    0b0000000000000000,
    0b0000000000000000,
];

// Masaustu ikonu: basit bir kare ikon + altinda metin
// (Win7 tarzi: ust gradient kutu, alt etiket)
pub struct DesktopIcon {
    pub x: usize,
    pub y: usize,
    pub label: &'static str,
    pub color_top: u32,
    pub color_bottom: u32,
}

pub fn draw(r: &Renderer) {
    use crate::syscall;
    let renk = syscall::sys_reg_get_id(0);
    let top = if renk == 0 { theme::BG_TOP } else { renk };

    let h = r.height();
    let half = h / 2;
    r.fill_gradient(0, 0, r.width(), half, top, theme::BG_MID);
    r.fill_gradient(0, half, r.width(), h - half, theme::BG_MID, theme::BG_BOTTOM);
    draw_r_logo(r);
}

// Ortadaki buyuk parlak R logosu (Aero camsi)
fn draw_r_logo(r: &Renderer) {
    let scale = 10;
    let logo_px = 16 * scale; // 160 px
    let cx = (r.width() - logo_px) / 2;
    let cy = (r.height() - logo_px) / 2 - 60; // biraz yukarida

    for row in 0..16 {
        for col in 0..16 {
            if (R_LOGO[row] >> (15 - col)) & 1 == 1 {
                // R'nin ust kismi parlak sari-turuncu, alt kismi koyu (camsi gecis)
                let color = if row < 7 {
                    0x00FFD060 // parlak ust
                } else if row == 7 {
                    0x00FF9020 // orta
                } else {
                    0x00E85810 // koyu alt
                };
                for dy in 0..scale {
                    for dx in 0..scale {
                        r.put_pixel(cx + col * scale + dx, cy + row * scale + dy, color);
                    }
                }
            }
        }
    }

    // Logo altinda "Rusty OS" yazisi (golgeli, ortada)
    let text = "Rusty OS";
    let text_w = text.len() * 7 * 3; // scale 3
    let tx = (r.width() - text_w) / 2;
    let ty = cy + logo_px + 20;
    // golge
    r.draw_text(text, tx + 2, ty + 2, 0x00401808, 3);
    // metin
    r.draw_text(text, tx, ty, 0x00FFF0E0, 3);
}

// Tek bir masaustu ikonu ciz (Win7 tarzi: gradient kutu + etiket)
fn draw_icon(r: &Renderer, icon: &DesktopIcon) {
    let size = 48;
    // ikon kutusu (yuvarlak koseli glossy)
    r.fill_rounded_glossy(icon.x, icon.y, size, size, 8, icon.color_top, icon.color_bottom);
    // ic parlama (cam)
    r.fill_rect_alpha(icon.x + 4, icon.y + 4, size - 8, 6, 0x00FFFFFF, 60);

    // etiket (ikon altinda, golgeli, ortali)
    let label_w = icon.label.len() * 7; // scale 1
    let lx = icon.x + (size / 2) - (label_w / 2);
    let ly = icon.y + size + 6;
    r.draw_text(icon.label, lx + 1, ly + 1, 0x00000000, 1); // golge
    r.draw_text(icon.label, lx, ly, theme::ICON_TEXT, 1);   // metin
}
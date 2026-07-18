use crate::renderer::Renderer;
use crate::ui::theme;

pub fn draw(r: &Renderer) {
    let th = theme::TASKBAR_HEIGHT;
    let ty = r.height() - th;
    let w = r.width();

    // Zemin - daha belirgin koyu (siyaha yakin, arka plandan ayrissin)
    r.fill_gradient(0, ty, w, th, 0x00201008, 0x00100804);

    // KALIN ust kenar parlama (3px parlak turuncu - net gorunsun)
    r.fill_rect(0, ty, w, 3, 0x00FF8020);

    draw_start_button(r, ty);
    draw_clock(r, ty, w, th);
}

fn draw_start_button(r: &Renderer, ty: usize) {
    let th = theme::TASKBAR_HEIGHT;
    // Win7 tarzi yuvarlak kure - taskbar'in ortasinda, biraz buyuk
    let diameter = th + 6; // taskbar'dan biraz buyuk (ustune tasar)
    let radius = diameter / 2;
    let cx = 8 + radius;          // merkez x (sol kenar + yaricap)
    let cy = ty + (th / 2);       // merkez y (taskbar ortasi)

    // 1. Dis halka (koyu turuncu cerceve - derinlik)
    draw_circle(r, cx, cy, radius, 0x00802808);

    // 2. Ic kure (parlak turuncu, ust parlak alt koyu - camsi)
    for dy in 0..(diameter as i32) {
        for dx in 0..(diameter as i32) {
            let px = cx as i32 - radius as i32 + dx;
            let py = cy as i32 - radius as i32 + dy;
            let ddx = px - cx as i32;
            let ddy = py - cy as i32;
            let dist2 = ddx * ddx + ddy * ddy;
            let inner_r = (radius - 2) as i32;
            if dist2 <= inner_r * inner_r {
                // ust parlak, alt koyu (camsi kure)
                let t = (py - (cy as i32 - inner_r)) as u32;
                let max = (inner_r * 2) as u32;
                let color = lerp(0x00FFC860, 0x00C84808, t, max.max(1));
                r.put_pixel(px as usize, py as usize, color);
            }
        }
    }

    // 3. Ust parlama (cam isigi - ust yarida hafif beyaz)
    for dy in 0..(radius as i32) {
        for dx in 0..(diameter as i32) {
            let px = cx as i32 - radius as i32 + dx;
            let py = cy as i32 - radius as i32 + dy;
            let ddx = px - cx as i32;
            let ddy = py - cy as i32;
            let inner_r = (radius - 4) as i32;
            if ddx * ddx + ddy * ddy <= inner_r * inner_r {
                r.blend_pixel(px as usize, py as usize, 0x00FFFFFF, 50);
            }
        }
    }

    // 4. Ortada R logosu (beyaz)
    // 4. Ortada R logosu (buyuk, tam ortali)
    draw_start_r(r, cx, cy);
}

// Dolu daire ciz
fn draw_circle(r: &Renderer, cx: usize, cy: usize, radius: usize, color: u32) {
    let rad = radius as i32;
    for dy in -rad..=rad {
        for dx in -rad..=rad {
            if dx * dx + dy * dy <= rad * rad {
                let px = cx as i32 + dx;
                let py = cy as i32 + dy;
                if px >= 0 && py >= 0 {
                    r.put_pixel(px as usize, py as usize, color);
                }
            }
        }
    }
}

// Start kuresi icin R logosu (16x16, beyaz)
// Start kuresi icin R logosu (merkeze gore ortali, buyuk)
fn draw_start_r(r: &Renderer, center_x: usize, center_y: usize) {
    const START_R: [u16; 13] = [
        0b1111111000,
        0b1111111100,
        0b1100001100,
        0b1100001100,
        0b1100001100,
        0b1111111100,
        0b1111111000,
        0b1101100000,
        0b1100110000,
        0b1100011000,
        0b1100001100,
        0b1100000110,
        0b0000000000,
    ];
    let cols = 16; // maske TAM genisligi (16 bit, hepsini oku)
    let rows = 13;
    let scale = 3;

    // logonun toplam boyutu
    let logo_w = cols * scale;
    let logo_h = rows * scale;
    // merkeze gore sol-ust kose
    let x = center_x - logo_w / 2 - 6;
    let y = center_y - logo_h / 2;

    for row in 0..rows {
        for col in 0..cols {
            // maske 16 bit, soldan cols kadar bit kullaniyoruz (en soldan)
            if (START_R[row] >> (15 - col)) & 1 == 1 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        r.put_pixel(x + col * scale + dx, y + row * scale + dy, 0x00FFFFFF);
                    }
                }
            }
        }
    }
}
// Renk interpolasyon (lokal yardimci)
fn lerp(a: u32, b: u32, t: u32, max: u32) -> u32 {
    let ar = ((a >> 16) & 0xFF) as i32; let ag = ((a >> 8) & 0xFF) as i32; let ab = (a & 0xFF) as i32;
    let br = ((b >> 16) & 0xFF) as i32; let bg = ((b >> 8) & 0xFF) as i32; let bb = (b & 0xFF) as i32;
    let tt = t as i32; let mx = if max == 0 { 1 } else { max as i32 };
    let rr = (ar + (br - ar) * tt / mx) as u32;
    let gg = (ag + (bg - ag) * tt / mx) as u32;
    let bl = (ab + (bb - ab) * tt / mx) as u32;
    (rr << 16) | (gg << 8) | bl
}

fn draw_clock(r: &Renderer, ty: usize, w: usize, th: usize) {
    use crate::syscall;
    // RTC'den saat/tarih al
    let mut time: [i32; 6] = [0; 6]; // h,m,s,day,month,year
    syscall::sys_get_time(time.as_mut_ptr() as u64);
    let h = unsafe { core::ptr::read_volatile(&time[0]) };
    let m = unsafe { core::ptr::read_volatile(&time[1]) };
    let day = unsafe { core::ptr::read_volatile(&time[3]) };
    let mon = unsafe { core::ptr::read_volatile(&time[4]) };
    let yr = unsafe { core::ptr::read_volatile(&time[5]) };

    // Saat metni HH:MM
    let mut clock = [0u8; 5];
    clock[0] = b'0' + (h / 10) as u8;
    clock[1] = b'0' + (h % 10) as u8;
    clock[2] = b':';
    clock[3] = b'0' + (m / 10) as u8;
    clock[4] = b'0' + (m % 10) as u8;
    let clock_str = core::str::from_utf8(&clock).unwrap_or("00:00");

    // Tarih metni DD/MM/YY
    let mut date = [0u8; 8];
    date[0] = b'0' + (day / 10) as u8;
    date[1] = b'0' + (day % 10) as u8;
    date[2] = b'/';
    date[3] = b'0' + (mon / 10) as u8;
    date[4] = b'0' + (mon % 10) as u8;
    date[5] = b'/';
    date[6] = b'0' + (yr / 10) as u8;
    date[7] = b'0' + (yr % 10) as u8;
    let date_str = core::str::from_utf8(&date).unwrap_or("01/01/26");

    let scale = 2;
    let cx = w - 90;
    // saat (ust)
    let cy = ty + 6;
    r.draw_text(clock_str, cx + 1, cy + 1, 0x00000000, scale);
    r.draw_text(clock_str, cx, cy, 0x00FFF0E0, scale);
    // tarih (alt, kucuk)
    r.draw_text(date_str, cx, cy + 20, 0x00FFC890, 1);
}
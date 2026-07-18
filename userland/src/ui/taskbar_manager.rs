use crate::renderer::Renderer;
use crate::syscall;
use alloc::string::String;
use alloc::format;

// === Olculer (draw ve hit-test AYNI sabitleri kullanir) ===
const TH: i32 = 44;        // taskbar yuksekligi
const MENU_W: i32 = 400;
const MENU_H: i32 = 460;
const MENU_X: i32 = 8;
const PAD: i32 = 8;
const LEFT_W: i32 = 224;   // beyaz panel genisligi
const ROW_H: i32 = 38;     // sol uygulama satiri
const LROW_H: i32 = 28;    // sag link satiri

const APPS: [(&str, u32); 9] = [
    ("Gezgin", 3), ("Not Defteri", 6), ("Paint", 2),
    ("Komut Istemi", 7), ("Hesap Makinesi", 9), ("Ayarlar", 10),
    ("Kayit Duzenleyici", 4), ("Gorev Yoneticisi", 5), ("Rusty Hakkinda", 1),
];

// Sag panel klasor linkleri (surucu harfi calisma aninda eklenir)
const LINKS: [(&str, &str); 3] = [
    ("Belgeler", "Users/User/Documents"),
    ("Indirilenler", "Users/User/Downloads"),
    ("Masaustu", "Users/User/Desktop"),
];

static mut MENU_OPEN: bool = false;
static mut OPEN_WINDOW_REQUEST: bool = false;
static mut OPEN_APP_KIND: u32 = 1;
static mut OPEN_APP_PATH: Option<String> = None;
static mut USER_CACHE: Option<String> = None;
static mut DRIVE_CACHE: Option<String> = None;

pub fn is_menu_open() -> bool { unsafe { MENU_OPEN } }

pub fn toggle_menu() {
    unsafe {
        MENU_OPEN = !MENU_OPEN;
        if MENU_OPEN { refresh_info(); }
    }
}

pub fn close_menu() { unsafe { MENU_OPEN = false; } }

// Menu acilirken kullanici adi + sistem surucusunu bir kez cek
fn refresh_info() {
    let mut dump = [0u8; 4096];
    let n = syscall::sys_reg_list(&mut dump) as usize;
    let mut user = String::from("User");
    if let Ok(text) = core::str::from_utf8(&dump[..n.min(4096)]) {
        for line in text.lines() {
            if let Some(v) = line.trim().strip_prefix("Oturum/AktifKullanici=str:") {
                if !v.is_empty() { user = String::from(v); }
                break;
            }
        }
    }

    let mut buf = [0u8; 256];
    let dn = syscall::sys_list_dir("", &mut buf);
    let mut drive = String::from("C:");
    if dn >= 1 && buf[0].is_ascii_alphabetic() && buf[1] == b':' {
        drive.clear();
        drive.push(buf[0] as char);
        drive.push(':');
    }

    unsafe { USER_CACHE = Some(user); DRIVE_CACHE = Some(drive); }
}

#[allow(static_mut_refs)]
fn user_name() -> String {
    unsafe { USER_CACHE.clone().unwrap_or_else(|| String::from("User")) }
}
#[allow(static_mut_refs)]
fn sys_drive() -> String {
    unsafe { DRIVE_CACHE.clone().unwrap_or_else(|| String::from("C:")) }
}

fn request(kind: u32, path: String) {
    unsafe {
        OPEN_APP_KIND = kind;
        OPEN_APP_PATH = Some(path);
        OPEN_WINDOW_REQUEST = true;
    }
}

pub fn take_open_window_request() -> bool {
    unsafe { let r = OPEN_WINDOW_REQUEST; OPEN_WINDOW_REQUEST = false; r }
}
pub fn take_app_kind() -> u32 { unsafe { OPEN_APP_KIND } }
#[allow(static_mut_refs)]
pub fn take_app_path() -> String {
    unsafe { OPEN_APP_PATH.take().unwrap_or_default() }
}

// ============================================================
// === TIKLAMA ================================================
// ============================================================
pub fn handle_click(mx: i32, my: i32, height: usize, _width: usize) -> bool {
    let ty = height as i32 - TH;

    // Start kuresi
    let scx = 8 + 25;
    let scy = ty + TH / 2;
    let dx = mx - scx; let dy = my - scy;
    if dx * dx + dy * dy <= 28 * 28 {
        toggle_menu();
        return true;
    }

    if !is_menu_open() { return false; }

    let my0 = (height as i32 - TH - MENU_H).max(0);
    let inside = mx >= MENU_X && mx < MENU_X + MENU_W && my >= my0 && my < my0 + MENU_H;
    if !inside {
        close_menu();
        return true;
    }

    // --- Sol panel: uygulamalar ---
    let lx = MENU_X + PAD;
    let ly = my0 + PAD;
    if mx >= lx && mx < lx + LEFT_W {
        let mut ay = ly + 12;
        for (_, kind) in APPS.iter() {
            if my >= ay && my < ay + ROW_H - 6 {
                request(*kind, String::new());
                close_menu();
                return true;
            }
            ay += ROW_H;
        }
    }

    // --- Sag panel ---
    let rx = MENU_X + PAD + LEFT_W + PAD;
    let rw = MENU_W - PAD - (rx - MENU_X);
    if mx >= rx && mx < rx + rw {
        let avy = my0 + PAD + 6;
        let mut y = avy + 56 + 26; // avatar + isim altindan basla
        let drive = sys_drive();

        for (_, sub) in LINKS.iter() {
            if my >= y && my < y + LROW_H - 2 {
                request(3, format!("{}/{}", drive, sub));
                close_menu();
                return true;
            }
            y += LROW_H;
        }
        y += 12; // ayrac
        if my >= y && my < y + LROW_H - 2 {
            request(3, String::new()); // Bilgisayar
            close_menu();
            return true;
        }

        // Kapat / Yeniden
        let sd_y = my0 + MENU_H - PAD - 34;
        if my >= sd_y && my < sd_y + 30 {
            if mx >= rx && mx < rx + 78 { syscall::sys_power(0); }
            if mx >= rx + 84 && mx < rx + 148 { syscall::sys_power(1); }
        }
    }

    true // menu ici bos tiklama: yut, acik kalsin
}

// ============================================================
// === CIZIM ==================================================
// ============================================================
pub fn draw_menu(r: &Renderer, _width: usize, height: usize) {
    if !is_menu_open() { return; }

    let my0 = (height as i32 - TH - MENU_H).max(0);
    let x = MENU_X as usize;
    let y = my0 as usize;
    let w = MENU_W as usize;
    let h = MENU_H as usize;

    // Golge + aero cam govde
    r.fill_rect_alpha(x + 5, y + 5, w, h, 0x00000000, 90);
    r.fill_rounded(x, y, w, h, 10, 0x002A1006);
    r.fill_rect_alpha(x + 2, y + 2, w - 4, 24, 0x00FF9040, 40); // ust cam parlama
    r.draw_rounded_border(x, y, w, h, 10, 0x00FF8020);

    // === SOL: beyaz uygulama paneli ===
    let lx = x + PAD as usize;
    let ly = y + PAD as usize;
    let lw = LEFT_W as usize;
    let lh = (MENU_H - PAD * 2 - 40) as usize;
    r.fill_rounded(lx, ly, lw, lh, 6, 0x00FFFFFF);
    r.draw_rounded_border(lx, ly, lw, lh, 6, 0x00D8C8B8);

    let mut ay = ly + 12;
    for (i, (name, _)) in APPS.iter().enumerate() {
        let ix = lx + 10;
        match i {
            0 => ic_folder(r, ix, ay),
            1 => ic_note(r, ix, ay),
            2 => ic_paint(r, ix, ay),
            3 => ic_cmd(r, ix, ay),
            4 => ic_reg(r, ix, ay),
            5 => ic_task(r, ix, ay),
            _ => ic_info(r, ix, ay),
        }
        r.draw_text(name, ix + 28, ay + 5, 0x00403028, 1);
        ay += ROW_H as usize;
    }

    // Ayrac + Tum Programlar (dekoratif)
    let sep_y = ly + lh - 34;
    r.fill_rect(lx + 10, sep_y, lw - 20, 1, 0x00E0D4C8);
    r.draw_text("Tum Programlar", lx + 28, sep_y + 12, 0x00806850, 1);
    r.draw_text(">", lx + 12, sep_y + 12, 0x00FF8020, 1);

    // === SOL ALT: arama kutusu (dekoratif) ===
    let sy = y + h - PAD as usize - 32;
    r.fill_rounded(lx, sy, lw, 30, 6, 0x00FFFFFF);
    r.draw_rounded_border(lx, sy, lw, 30, 6, 0x00C8B8A8);
    r.draw_text("Program ve dosya ara", lx + 10, sy + 11, 0x00A89888, 1);
    // minik buyutec
    let gx = (lx + lw - 22) as i32; let gy = (sy + 9) as i32;
    for a in 0..64i32 {
        let px = gx + 5; let py = gy + 5;
        let ddx = (a % 11) - 5; let ddy = (a / 11) - 3;
        let d2 = ddx * ddx + ddy * ddy;
        if d2 >= 12 && d2 <= 25 { r.put_pixel((px + ddx) as usize, (py + ddy) as usize, 0x00806850); }
    }
    r.fill_rect((gx + 9) as usize, (gy + 9) as usize, 4, 2, 0x00806850);

    // === SAG: cam panel icerigi ===
    let rx = x + (PAD + LEFT_W + PAD) as usize;
    let rw = (MENU_W - PAD) as usize + x - rx;

    // Avatar (login karesinin kucugu)
    let av = 56;
    let avx = rx + (rw - av) / 2;
    let avy = y + PAD as usize + 6;
    r.fill_rect_alpha(avx + 3, avy + 3, av, av, 0x00000000, 80);
    r.fill_rounded_glossy(avx, avy, av, av, 10, 0x00FFB048, 0x00C85810);
    r.fill_rect_alpha(avx + 4, avy + 4, av - 8, 10, 0x00FFFFFF, 60);
    r.draw_rounded_border(avx, avy, av, av, 10, 0x00FFE0A0);

    let user = user_name();
    let ch = user.chars().next().unwrap_or('U').to_ascii_uppercase();
    let mut b = [0u8; 4];
    let s = ch.encode_utf8(&mut b);
    r.draw_text(s, avx + 15, avy + 13, 0x00FFFFFF, 4);

    // Kullanici adi
    let nw = user.len() * 7;
    let ux = rx + (rw.saturating_sub(nw)) / 2;
    r.draw_text(&user, ux + 1, avy + av + 9, 0x00200A04, 1);
    r.draw_text(&user, ux, avy + av + 8, 0x00FFF0E0, 1);

    // Linkler
    let mut yy = avy + av + 26;
    for (label, _) in LINKS.iter() {
        r.draw_text(label, rx + 8, yy + 6, 0x00FFE0C0, 1);
        yy += LROW_H as usize;
    }
    r.fill_rect(rx + 4, yy + 5, rw - 8, 1, 0x00804018);
    yy += 12;
    r.draw_text("Bilgisayar", rx + 8, yy + 6, 0x00FFE0C0, 1);

    // === SAG ALT: Kapat + Yeniden ===
    let sd_y = y + h - PAD as usize - 34;
    r.fill_rounded_glossy(rx, sd_y, 78, 30, 6, 0x00F04020, 0x00A01808);
    r.draw_text("Kapat", rx + 22, sd_y + 10, 0x00FFFFFF, 1);
    r.fill_rounded_glossy(rx + 84, sd_y, 64, 30, 6, 0x00FF9030, 0x00C06010);
    r.draw_text("Yeniden", rx + 84 + 8, sd_y + 10, 0x00FFFFFF, 1);
}

// ============================================================
// === MINI IKONLAR (18x18) ===================================
// ============================================================
fn ic_folder(r: &Renderer, x: usize, y: usize) {
    r.fill_rect(x + 1, y + 2, 8, 3, 0x00E09020);
    r.fill_rounded_glossy(x, y + 4, 18, 12, 3, 0x00FFC850, 0x00E09020);
}
fn ic_note(r: &Renderer, x: usize, y: usize) {
    r.fill_rounded_glossy(x + 2, y, 14, 18, 2, 0x00FFFFFF, 0x00D8D0C8);
    r.draw_rounded_border(x + 2, y, 14, 18, 2, 0x00A09888);
    for i in 0..4 { r.fill_rect(x + 5, y + 4 + i * 3, 8, 1, 0x00B0A090); }
}
fn ic_paint(r: &Renderer, x: usize, y: usize) {
    r.fill_rounded_glossy(x + 1, y + 1, 16, 16, 3, 0x00FFF0E0, 0x00E0D0C0);
    r.draw_rounded_border(x + 1, y + 1, 16, 16, 3, 0x00A09080);
    r.fill_rect(x + 4, y + 4, 4, 4, 0x00E04030);
    r.fill_rect(x + 10, y + 4, 4, 4, 0x00F0C020);
    r.fill_rect(x + 4, y + 10, 4, 4, 0x003070E0);
    r.fill_rect(x + 10, y + 10, 4, 4, 0x0030A040);
}
fn ic_reg(r: &Renderer, x: usize, y: usize) {
    r.fill_rounded_glossy(x + 1, y + 2, 16, 14, 3, 0x00E8E0D8, 0x00A89A8C);
    r.fill_rect(x + 4, y + 6, 10, 2, 0x00FF8020);
    r.fill_rect(x + 4, y + 10, 10, 2, 0x00C05810);
}
fn ic_task(r: &Renderer, x: usize, y: usize) {
    r.fill_rect(x + 2, y + 10, 4, 7, 0x00FF9030);
    r.fill_rect(x + 7, y + 6, 4, 11, 0x00E06010);
    r.fill_rect(x + 12, y + 2, 4, 15, 0x00FFC050);
}
fn ic_cmd(r: &Renderer, x: usize, y: usize) {
    r.fill_rounded_glossy(x, y + 1, 18, 16, 3, 0x00303030, 0x00000000);
    r.draw_rounded_border(x, y + 1, 18, 16, 3, 0x00808080);
    r.draw_text(">", x + 3, y + 5, 0x00C0C0C0, 1);
    r.fill_rect(x + 10, y + 11, 5, 1, 0x00C0C0C0);
}
fn ic_calc(r: &Renderer, x: usize, y: usize) {
    r.fill_rounded_glossy(x + 2, y, 14, 18, 2, 0x00E8E0D8, 0x00A89A8C);
    r.fill_rect(x + 4, y + 2, 10, 4, 0x00203040);
    for i in 0..3 { for j in 0..3 {
        r.fill_rect(x + 4 + j * 4, y + 8 + i * 3, 2, 2, 0x00604838);
    }}
}
fn ic_gear(r: &Renderer, x: usize, y: usize) {
    r.fill_circle((x + 9) as i32, (y + 9) as i32, 7, 0x00808890);
    r.fill_circle((x + 9) as i32, (y + 9) as i32, 3, 0x00F0E6DA);
    for a in [(9i32, 0i32), (9, 18), (0, 9), (18, 9)] {
        r.fill_rect((x as i32 + a.0 - 1).max(0) as usize, (y as i32 + a.1 - 1).max(0) as usize, 3, 3, 0x00808890);
    }
}
fn ic_info(r: &Renderer, x: usize, y: usize) {
    for dy in -8i32..=8 {
        for dx in -8i32..=8 {
            if dx * dx + dy * dy <= 64 {
                r.put_pixel((x as i32 + 9 + dx) as usize, (y as i32 + 9 + dy) as usize, 0x00FF8C20);
            }
        }
    }
    r.draw_text("i", x + 6, y + 5, 0x00FFFFFF, 1);
}
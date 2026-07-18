use crate::renderer::Renderer;
use crate::syscall;
use alloc::string::String;
use alloc::vec;
use alloc::format;

const CUR: [[u8; 12]; 19] = [
    [2,2,0,0,0,0,0,0,0,0,0,0],[2,1,2,0,0,0,0,0,0,0,0,0],[2,1,1,2,0,0,0,0,0,0,0,0],
    [2,1,1,1,2,0,0,0,0,0,0,0],[2,1,1,1,1,2,0,0,0,0,0,0],[2,1,1,1,1,1,2,0,0,0,0,0],
    [2,1,1,1,1,1,1,2,0,0,0,0],[2,1,1,1,1,1,1,1,2,0,0,0],[2,1,1,1,1,1,1,1,1,2,0,0],
    [2,1,1,1,1,1,1,1,1,1,2,0],[2,1,1,1,1,1,2,2,2,2,2,2],[2,1,1,2,1,1,2,0,0,0,0,0],
    [2,1,2,0,2,1,1,2,0,0,0,0],[2,2,0,0,2,1,1,2,0,0,0,0],[0,0,0,0,0,2,1,1,2,0,0,0],
    [0,0,0,0,0,2,1,1,2,0,0,0],[0,0,0,0,0,0,2,1,1,2,0,0],[0,0,0,0,0,0,2,1,1,2,0,0],
    [0,0,0,0,0,0,0,2,2,0,0,0],
];

fn arrow(r: &Renderer, mx: usize, my: usize) {
    for cy in 0..19 { for cx in 0..12 {
        match CUR[cy][cx] {
            1 => r.put_pixel(mx + cx, my + cy, 0x00FFFFFF),
            2 => r.put_pixel(mx + cx, my + cy, 0x00000000),
            _ => {}
        }
    }}
}

fn reg_get(dump: &str, key: &str) -> String {
    for line in dump.lines() {
        if let Some(rest) = line.trim().strip_prefix(key) {
            if let Some(v) = rest.strip_prefix("=str:") { return String::from(v); }
        }
    }
    String::new()
}

fn rtc_sec() -> i32 {
    let mut t: [i32; 6] = [0; 6];
    syscall::sys_get_time(t.as_mut_ptr() as u64);
    unsafe { core::ptr::read_volatile(&t[2]) }
}

fn bg(r: &Renderer, w: usize, h: usize) {
    r.fill_gradient(0, 0, w, h / 2, 0x00120503, 0x00481006);
    r.fill_gradient(0, h / 2, w, h - h / 2, 0x00481006, 0x00180804);
}

fn scene(r: &Renderer, w: usize, h: usize, user: &str, pw_mode: bool, has_pw: bool, input: &str, error: bool) {
    bg(r, w, h);
    let cx = w / 2;
    let sq_y = h / 2 - 150;
    r.fill_rect_alpha(cx - 56, sq_y + 6, 120, 120, 0x00000000, 90);
    r.fill_rounded_glossy(cx - 60, sq_y, 120, 120, 12, 0x00FFB048, 0x00C85810);
    r.fill_rect_alpha(cx - 52, sq_y + 6, 104, 14, 0x00FFFFFF, 60);
    r.draw_rounded_border(cx - 60, sq_y, 120, 120, 12, 0x00FFE0A0);
    let ch = user.chars().next().unwrap_or('U').to_ascii_uppercase();
    let mut b = [0u8; 4];
    let s = ch.encode_utf8(&mut b);
    r.draw_text(s, cx - 24, sq_y + 32, 0x00FFFFFF, 7);
    let nw = user.len() * 7 * 2;
    r.draw_text(user, cx - nw / 2 + 1, sq_y + 141, 0x00200a04, 2);
    r.draw_text(user, cx - nw / 2, sq_y + 140, 0x00FFF0E0, 2);

    if !pw_mode {
        let hint = if has_pw { "Sifre icin tiklayin" } else { "Giris icin tiklayin" };
        r.draw_text(hint, cx - hint.len() * 7 / 2, sq_y + 172, 0x00C09070, 1);
    } else {
        r.fill_rounded(cx - 110, sq_y + 168, 220, 30, 6, 0x00FFFFFF);
        r.draw_rounded_border(cx - 110, sq_y + 168, 220, 30, 6, 0x00FF8020);
        let mut mask = String::new();
        for _ in 0..input.len() { mask.push('*'); }
        r.draw_text(&mask, cx - 100, sq_y + 177, 0x00201008, 2);
        r.draw_text("_", cx - 100 + mask.len() * 14, sq_y + 177, 0x00C04408, 2);
        r.fill_rounded_glossy(cx - 50, sq_y + 208, 100, 30, 6, 0x00FF9030, 0x00C85810);
        r.draw_text("Giris", cx - 17, sq_y + 217, 0x00FFFFFF, 1);
        if error { r.draw_text("Hatali sifre!", cx - 45, sq_y + 246, 0x00FF5040, 1); }
    }
    r.draw_text("Rusty OS 0.1", 20, h - 30, 0x00A06840, 1);
    let pcx = (w - 46) as i32; let pcy = (h - 46) as i32;
    for dy in -18i32..=18 { for dx in -18i32..=18 {
        let d2 = dx * dx + dy * dy;
        if d2 <= 324 && d2 >= 144 { r.put_pixel((pcx + dx) as usize, (pcy + dy) as usize, 0x00E04030); }
    }}
    r.fill_rect((pcx - 1) as usize, (pcy - 14) as usize, 3, 12, 0x00E04030);
}

fn welcome(r: &Renderer, w: usize, h: usize) {
    let mut prev = rtc_sec();
    let mut count = 0;
    let mut dots = 0usize;
    loop {
        bg(r, w, h);
        let msg = "Hosgeldiniz";
        r.draw_text(msg, w / 2 - msg.len() * 21 / 2 + 2, h / 2 - 10, 0x00200a04, 3);
        r.draw_text(msg, w / 2 - msg.len() * 21 / 2, h / 2 - 12, 0x00FFE8C8, 3);
        let d = ["", ".", "..", "..."][dots % 4];
        r.draw_text(d, w / 2 - 10, h / 2 + 30, 0x00FFB060, 2);
        r.present();
        loop {
            let s = rtc_sec();
            if s != prev { prev = s; count += 1; dots += 1; break; }
        }
        if count >= 2 { return; }
    }
}

pub fn run(r: &Renderer, w: usize, h: usize) {
    let mut dump = vec![0u8; 8192];
    let n = syscall::sys_reg_list(&mut dump) as usize;
    let text = core::str::from_utf8(&dump[..n.min(dump.len())]).unwrap_or("");
    let mut user = reg_get(text, "Oturum/AktifKullanici");
    if user.is_empty() { user = String::from("User"); }
    let pass = reg_get(text, &format!("Kullanicilar/{}/Sifre", user));
    let has_pw = !pass.is_empty();

    let mut input = String::new();
    let mut pw_mode = false;
    let mut error = false;
    let mut mx = (w / 2) as i32;
    let mut my = (h / 2) as i32;
    let mut prev = false;
    let mut ev: [i32; 4] = [0; 4];
    let cx = (w / 2) as i32;
    let sq_y = (h / 2 - 150) as i32;
    let mut dirty = true;

    loop {
        if dirty {
            scene(r, w, h, &user, pw_mode, has_pw, &input, error);
            arrow(r, mx as usize, my as usize);
            r.present();
            dirty = false;
        }
        while syscall::sys_poll_event(ev.as_mut_ptr() as u64) == 1 {
            let k = unsafe { core::ptr::read_volatile(&ev[0]) };
            if k == 2 {
                mx = unsafe { core::ptr::read_volatile(&ev[1]) }.clamp(0, w as i32 - 12);
                my = unsafe { core::ptr::read_volatile(&ev[2]) }.clamp(0, h as i32 - 19);
                let b = (unsafe { core::ptr::read_volatile(&ev[3]) } & 1) == 1;
                let click = b && !prev;
                prev = b;
                dirty = true;
                if click {
                    let dx = mx - (w as i32 - 46); let dy = my - (h as i32 - 46);
                    if dx * dx + dy * dy <= 400 { syscall::sys_power(0); }
                    if !pw_mode {
                        if mx >= cx - 60 && mx < cx + 60 && my >= sq_y && my < sq_y + 165 {
                            if has_pw { pw_mode = true; error = false; } else { welcome(r, w, h); return; }
                        }
                    } else if mx >= cx - 50 && mx < cx + 50 && my >= sq_y + 208 && my < sq_y + 238 {
                        if input == pass { welcome(r, w, h); return; }
                        error = true; input.clear();
                    }
                }
            } else if k == 1 {
                let ch = unsafe { core::ptr::read_volatile(&ev[1]) };
                dirty = true;
                if pw_mode {
                    match ch {
                        13 | 10 => { if input == pass { welcome(r, w, h); return; } error = true; input.clear(); }
                        27 => { pw_mode = false; input.clear(); error = false; }
                        8 | 127 => { input.pop(); }
                        32..=126 => { if input.len() < 16 { input.push(ch as u8 as char); } }
                        _ => {}
                    }
                } else if ch == 13 || ch == 10 {
                    if has_pw { pw_mode = true; } else { welcome(r, w, h); return; }
                }
            }
        }
        for _ in 0..20000 { unsafe { core::arch::asm!("nop"); } }
    }
}
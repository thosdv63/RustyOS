use crate::renderer::Renderer;
use crate::syscall;
use alloc::string::String;
use alloc::vec;
use alloc::format;

const RENKLER: [u32; 4] = [0x00501808, 0x00103060, 0x00104020, 0x00301040];

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
        match CUR[cy][cx] { 1 => r.put_pixel(mx+cx, my+cy, 0x00FFFFFF), 2 => r.put_pixel(mx+cx, my+cy, 0x00000000), _ => {} }
    }}
}

fn rtc_sec() -> i32 {
    let mut t: [i32; 6] = [0; 6];
    syscall::sys_get_time(t.as_mut_ptr() as u64);
    unsafe { core::ptr::read_volatile(&t[2]) }
}

pub fn needed() -> bool {
    let mut b = vec![0u8; 8192];
    let n = syscall::sys_reg_list(&mut b) as usize;
    if let Ok(t) = core::str::from_utf8(&b[..n.min(b.len())]) {
        for line in t.lines() {
            if let Some(v) = line.trim().strip_prefix("Oturum/IlkKurulumBitti=bool:") {
                return v.trim() != "1";
            }
        }
    }
    true
}

fn bg(r: &Renderer, w: usize, h: usize) {
    r.fill_gradient(0, 0, w, h / 2, 0x00120503, 0x00481006);
    r.fill_gradient(0, h / 2, w, h - h / 2, 0x00481006, 0x00180804);
}

fn scene(r: &Renderer, w: usize, h: usize, step: u32, name: &str, pass: &str, renk: usize) {
    bg(r, w, h);
    let cx = w / 2 - 240; let cy = h / 2 - 170;
    r.fill_rect_alpha(cx + 6, cy + 6, 480, 340, 0x00000000, 90);
    r.fill_rounded(cx, cy, 480, 340, 10, 0x00F5EFE9);
    r.fill_rounded_glossy(cx, cy, 480, 54, 10, 0x00FF8020, 0x00C84810);
    let baslik = match step { 0 => "Rusty OS Kurulumu", 1 => "Kullanici Adi", 2 => "Sifre (istege bagli)", _ => "Masaustu Rengin" };
    r.draw_text(baslik, cx + 20, cy + 18, 0x00FFFFFF, 2);

    match step {
        0 => {
            r.draw_text("Rusty OS'a hosgeldin!", cx + 24, cy + 90, 0x00603018, 2);
            r.draw_text("Birkac adimda hesabini kuracagiz.", cx + 24, cy + 130, 0x00806040, 1);
            r.draw_text("Ileri'ye bas ve baslayalim.", cx + 24, cy + 154, 0x00806040, 1);
        }
        1 => {
            r.draw_text("Adin ne olsun?", cx + 24, cy + 90, 0x00603018, 1);
            r.fill_rounded(cx + 24, cy + 116, 300, 34, 6, 0x00FFFFFF);
            r.draw_rounded_border(cx + 24, cy + 116, 300, 34, 6, 0x00FF8020);
            r.draw_text(name, cx + 34, cy + 126, 0x00201008, 2);
            r.draw_text("_", cx + 34 + name.len() * 14, cy + 126, 0x00C04408, 2);
            r.draw_text("En fazla 8 harf/rakam", cx + 24, cy + 160, 0x00A08060, 1);
        }
        2 => {
            r.draw_text("Sifre belirle (bos birakabilirsin):", cx + 24, cy + 90, 0x00603018, 1);
            r.fill_rounded(cx + 24, cy + 116, 300, 34, 6, 0x00FFFFFF);
            r.draw_rounded_border(cx + 24, cy + 116, 300, 34, 6, 0x00FF8020);
            let mut m = String::new();
            for _ in 0..pass.len() { m.push('*'); }
            r.draw_text(&m, cx + 34, cy + 126, 0x00201008, 2);
            r.draw_text("_", cx + 34 + m.len() * 14, cy + 126, 0x00C04408, 2);
        }
        _ => {
            r.draw_text("Bir renk sec:", cx + 24, cy + 86, 0x00603018, 1);
            for (i, c) in RENKLER.iter().enumerate() {
                let sx = cx + 30 + i * 110;
                r.fill_rounded_glossy(sx, cy + 116, 90, 90, 8, *c | 0x00303030, *c);
                if i == renk { r.draw_rounded_border(sx.saturating_sub(4), cy + 112, 98, 98, 8, 0x00FFD060); }
            }
        }
    }
    if step > 0 {
        r.fill_rounded_glossy(cx + 16, cy + 290, 90, 34, 6, 0x00C0A080, 0x00907050);
        r.draw_text("Geri", cx + 44, cy + 300, 0x00FFFFFF, 1);
    }
    r.fill_rounded_glossy(cx + 374, cy + 290, 90, 34, 6, 0x00FF9030, 0x00C85810);
    let son = if step == 3 { "Bitir" } else { "Ileri" };
    r.draw_text(son, cx + 402, cy + 300, 0x00FFFFFF, 1);
}

fn bekle_1sn() {
    let p = rtc_sec();
    loop { if rtc_sec() != p { return; } }
}

fn finalize(r: &Renderer, w: usize, h: usize, name: &str, pass: &str, renk: usize) {
    bg(r, w, h);
    let msg = "Hazirlaniyor...";
    r.draw_text(msg, w / 2 - msg.len() * 14 / 2, h / 2 - 8, 0x00FFE8C8, 2);
   // r.draw_text("A", 20, 20, 0x0000FF00, 3);   // A
    r.present();

    let _ = syscall::sys_create_dir("Users");
    let _ = syscall::sys_create_dir("Users/Shared");

    let mut b = vec![0u8; 4096];
    let n = syscall::sys_list_dir("Users", &mut b) as usize;
    let mut var = false;
    for i in 0..n {
        let off = i * 40;
        let mut end = off;
        while end < off + 32 && b[end] != 0 { end += 1; }
        if let Ok(nm) = core::str::from_utf8(&b[off..end]) {
            if nm.eq_ignore_ascii_case(name) { var = true; break; }
        }
    }
  //  r.draw_text("B", 60, 20, 0x0000FF00, 3);    // B
    r.present();

    if !var {
        let _ = syscall::sys_create_dir(&format!("Users/{}", name));
        let _ = syscall::sys_create_dir(&format!("Users/{}/Desktop", name));
        let _ = syscall::sys_create_dir(&format!("Users/{}/Documents", name));
        let _ = syscall::sys_create_dir(&format!("Users/{}/Downloads", name));
    }
   // r.draw_text("C", 100, 20, 0x0000FF00, 3);   // C
    r.present();

    let _ = syscall::sys_reg_set_line(&format!("Oturum/AktifKullanici=str:{}", name));
    let _ = syscall::sys_reg_set_line(&format!("Kullanicilar/{}/Sifre=str:{}", name, pass));
    let _ = syscall::sys_reg_set_line(&format!("Kullanicilar/{}/Yetki=u32:1", name));
    let _ = syscall::sys_reg_set_line(&format!("Kullanicilar/{}/AnaKlasor=str:Users/{}", name, name));
    let _ = syscall::sys_reg_set_line(&format!("Kullanicilar/{}/Tema=str:aero", name));
    let _ = syscall::sys_reg_set_line("Kullanicilar/Shared/AnaKlasor=str:Users/Shared");
    let _ = syscall::sys_reg_set_line("Kullanicilar/Shared/Yetki=u32:1");
    let _ = syscall::sys_reg_set_line(&format!("Sistem/Masaustu/Renk=u32:{}", RENKLER[renk]));
   // r.draw_text("D", 140, 20, 0x0000FF00, 3);   // D
    r.present();

    let _ = syscall::sys_reg_set_line("Oturum/IlkKurulumBitti=bool:1");
   // r.draw_text("E", 180, 20, 0x0000FF00, 3);   // E
    r.present();

    bekle_1sn();
 //   r.draw_text("F", 220, 20, 0x0000FF00, 3);   // F
    r.present();
}

pub fn run(r: &Renderer, w: usize, h: usize) {
    let mut step: u32 = 0;
    let mut name = String::new();
    let mut pass = String::new();
    let mut renk = 0usize;
    let mut mx = (w / 2) as i32;
    let mut my = (h / 2) as i32;
    let mut prev = false;
    let mut ev: [i32; 4] = [0; 4];
    let cx = (w / 2 - 240) as i32;
    let cy = (h / 2 - 170) as i32;
    let mut dirty = true;

    loop {
        if dirty {
            scene(r, w, h, step, &name, &pass, renk);
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
                    if mx >= cx + 374 && mx < cx + 464 && my >= cy + 290 && my < cy + 324 {
                        if step == 1 && name.is_empty() { continue; }
                        if step == 3 { finalize(r, w, h, &name, &pass, renk); return; }
                        step += 1;
                    } else if step > 0 && mx >= cx + 16 && mx < cx + 106 && my >= cy + 290 && my < cy + 324 {
                        step -= 1;
                    } else if step == 3 && my >= cy + 116 && my < cy + 206 {
                        for i in 0..4i32 {
                            let sx = cx + 30 + i * 110;
                            if mx >= sx && mx < sx + 90 { renk = i as usize; }
                        }
                    }
                }
            } else if k == 1 {
                let ch = unsafe { core::ptr::read_volatile(&ev[1]) };
                dirty = true;
                match ch {
                    13 | 10 => {
                        if step == 1 && name.is_empty() { continue; }
                        if step == 3 { finalize(r, w, h, &name, &pass, renk); return; }
                        step += 1;
                    }
                    8 | 127 => { if step == 1 { name.pop(); } if step == 2 { pass.pop(); } }
                    c => {
                        if step == 1 && name.len() < 8 {
                            let bb = c as u8;
                            if bb.is_ascii_alphanumeric() { name.push(bb as char); }
                        } else if step == 2 && pass.len() < 16 && (32..=126).contains(&c) {
                            pass.push(c as u8 as char);
                        }
                    }
                }
            }
        }
        for _ in 0..20000 { unsafe { core::arch::asm!("nop"); } }
    }
}

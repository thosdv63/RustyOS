use crate::renderer::Renderer;
use crate::ui::theme;
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;

// kind: 0=Bilgisayar 1=CopKutusu 2=klasor(disk) 3=dosya(disk)
pub struct Icon {
    pub x: i32, pub y: i32,
    pub label: String,
    pub kind: u8,
    pub selected: bool,
    pub ct: u32, pub cb: u32,
}

const ICON_SIZE: i32 = 48;

static mut ICONS: Option<Vec<Icon>> = None;
static mut DRAGGING: i32 = -1;
static mut DOX: i32 = 0; static mut DOY: i32 = 0;
static mut SELECTING: bool = false;
static mut SX: i32 = 0; static mut SY: i32 = 0;
static mut CX: i32 = 0; static mut CY: i32 = 0;
static mut PREV_L: bool = false; static mut PREV_R: bool = false;
static mut LAST_ICON: i32 = -1;
static mut CMENU: bool = false;
static mut CMX: i32 = 0; static mut CMY: i32 = 0;
static mut CTARGET: i32 = -1;
static mut RENAME_ON: bool = false;
static mut RENAME_IDX: i32 = -1;
static mut RENAME_BUF: Option<String> = None;
static mut REQ_KIND: u32 = 0;
static mut REQ_PATH: Option<String> = None;
static mut DESKTOP_PATH: Option<String> = None;

#[allow(static_mut_refs)]
fn icons() -> &'static mut Vec<Icon> { unsafe { ICONS.get_or_insert_with(Vec::new) } }

// Aktif kullaniciyi registry'den oku -> Users/<Ad>/Desktop
#[allow(static_mut_refs)]
fn desktop_dir() -> &'static str {
    unsafe {
        if DESKTOP_PATH.is_none() {
            let mut b = vec![0u8; 8192];
            let n = syscall::sys_reg_list(&mut b) as usize;
            let mut user = String::from("User");
            if let Ok(t) = core::str::from_utf8(&b[..n.min(b.len())]) {
                for line in t.lines() {
                    if let Some(rest) = line.trim().strip_prefix("Oturum/AktifKullanici=str:") {
                        if !rest.is_empty() { user = String::from(rest); }
                        break;
                    }
                }
            }
            DESKTOP_PATH = Some(format!("Users/{}/Desktop", user));
        }
        DESKTOP_PATH.as_deref().unwrap()
    }
}

pub fn init() { rebuild(); }

#[allow(static_mut_refs)]
pub fn rebuild() {
    unsafe { DESKTOP_PATH = None; } // kullanici degistiyse taze oku
    let old: Vec<(String, i32, i32)> = icons().iter().map(|i| (i.label.clone(), i.x, i.y)).collect();
    let v = icons();
    v.clear();
    let mut push = |mut ic: Icon| {
        if let Some(p) = old.iter().find(|o| o.0.eq_ignore_ascii_case(&ic.label)) {
            ic.x = p.1; ic.y = p.2;
        }
        v.push(ic);
    };
    push(Icon { x: 30, y: 30, label: String::from("Bilgisayar"), kind: 0, selected: false, ct: 0x00FFB048, cb: 0x00D86818 });
    push(Icon { x: 30, y: 120, label: String::from("Cop Kutusu"), kind: 1, selected: false, ct: 0x00FF8060, cb: 0x00C04020 });
    let mut buf = vec![0u8; 4096];
    let n = syscall::sys_list_dir(desktop_dir(), &mut buf) as usize;
    let mut disk_i = 0i32;
    for i in 0..n {
        let off = i * 40;
        let mut end = off;
        while end < off + 32 && buf[end] != 0 { end += 1; }
        if let Ok(name) = core::str::from_utf8(&buf[off..end]) {
            let is_dir = buf[off + 32] == 1;
            let col = disk_i / 6; let row = disk_i % 6;
            disk_i += 1;
            let (ct, cb) = if is_dir { (0x00FFC850, 0x00E09020) } else { (0x00E8E0D8, 0x00B0A898) };
            push(Icon {
                x: 130 + col * 100, y: 30 + row * 95,
                label: String::from(name),
                kind: if is_dir { 2 } else { 3 },
                selected: false, ct, cb,
            });
        }
    }
}

fn icon_at(mx: i32, my: i32) -> i32 {
    let v = icons();
    let mut i = v.len() as i32 - 1;
    while i >= 0 {
        let ic = &v[i as usize];
        if mx >= ic.x && mx < ic.x + ICON_SIZE && my >= ic.y && my < ic.y + ICON_SIZE { return i; }
        i -= 1;
    }
    -1
}

fn menu_items() -> &'static [&'static str] {
    unsafe {
        if CTARGET < 0 { return &["Yeni Dosya", "Yeni Klasor", "Yenile"]; }
        match icons()[CTARGET as usize].kind {
            0 | 1 => &["Ac"],
            2 => &["Ac", "Sil", "Ad Degistir"],
            _ => &["Sil", "Ad Degistir"],
        }
    }
}

#[allow(static_mut_refs)]
fn open_icon(i: i32) {
    unsafe {
        let ic = &icons()[i as usize];
        match ic.kind {
            0 => { REQ_KIND = 3; REQ_PATH = Some(String::new()); }
            2 => { REQ_KIND = 3; REQ_PATH = Some(format!("{}/{}", desktop_dir(), ic.label)); }
            _ => {}
        }
    }
}

#[allow(static_mut_refs)]
pub fn take_app_request() -> Option<(u32, String)> {
    unsafe {
        if REQ_KIND == 0 { return None; }
        let k = REQ_KIND; REQ_KIND = 0;
        Some((k, REQ_PATH.take().unwrap_or_default()))
    }
}

#[allow(static_mut_refs)]
pub fn handle_key(ch: i32) -> bool {
    unsafe {
        if !RENAME_ON { return false; }
        let buf = RENAME_BUF.get_or_insert_with(String::new);
        match ch {
            13 | 10 => {
                if RENAME_IDX >= 0 && (RENAME_IDX as usize) < icons().len() && !buf.is_empty() {
                    let old = format!("{}/{}", desktop_dir(), icons()[RENAME_IDX as usize].label);
                    let newn = buf.clone();
                    let _ = syscall::sys_rename(&old, &newn);
                }
                RENAME_ON = false; RENAME_IDX = -1; RENAME_BUF = None;
                rebuild();
            }
            27 => { RENAME_ON = false; RENAME_IDX = -1; RENAME_BUF = None; }
            8 | 127 => { buf.pop(); }
            32..=126 => { if buf.len() < 12 { buf.push(ch as u8 as char); } }
            _ => {}
        }
        true
    }
}

#[allow(static_mut_refs)]
pub fn handle_mouse(mx: i32, my: i32, lbtn: bool, rbtn: bool, height: i32, menu_open: bool, block: bool) -> bool {
    unsafe {
        let pl = PREV_L; PREV_L = lbtn;
        let pr = PREV_R; PREV_R = rbtn;
        if menu_open { return false; }
        let tb_top = height - theme::TASKBAR_HEIGHT as i32;
        let mut changed = false;

        // Acik sag-tik menusu
        if CMENU {
            if lbtn && !pl {
                let items = menu_items();
                let mw = 150; let ih = 26;
                let mh = items.len() as i32 * ih + 8;
                if mx >= CMX && mx < CMX + mw && my >= CMY && my < CMY + mh {
                    let idx = ((my - CMY - 4) / ih) as usize;
                    if idx < items.len() { do_action(items[idx]); }
                }
                CMENU = false;
                return true;
            }
            if !(rbtn && !pr) { return changed; }
            CMENU = false; changed = true;
        }

        // Sag tik: menu ac (pencere ustunde degilse)
        if rbtn && !pr && my < tb_top && !block {
            CTARGET = icon_at(mx, my);
            CMENU = true; CMX = mx; CMY = my;
            return true;
        }

        // Sol basma: sec / surukle basla / cift tik ac
        if lbtn && !pl && my < tb_top && !block {
            let idx = icon_at(mx, my);
            if idx >= 0 {
                if idx == LAST_ICON { open_icon(idx); LAST_ICON = -1; }
                else { LAST_ICON = idx; }
                DRAGGING = idx;
                DOX = mx - icons()[idx as usize].x;
                DOY = my - icons()[idx as usize].y;
                for (i, ic) in icons().iter_mut().enumerate() { ic.selected = i as i32 == idx; }
                changed = true;
            } else {
                LAST_ICON = -1;
                SELECTING = true; SX = mx; SY = my; CX = mx; CY = my;
                for ic in icons().iter_mut() { ic.selected = false; }
                changed = true;
            }
        }

        // Basili: surukleme / secim kutusu
        if lbtn && pl {
            if DRAGGING >= 0 {
                let ic = &mut icons()[DRAGGING as usize];
                ic.x = (mx - DOX).max(0);
                ic.y = (my - DOY).max(0);
                changed = true;
            } else if SELECTING {
                CX = mx; CY = my;
                let x1 = SX.min(CX); let y1 = SY.min(CY);
                let x2 = SX.max(CX); let y2 = SY.max(CY);
                for ic in icons().iter_mut() {
                    ic.selected = ic.x < x2 && ic.x + ICON_SIZE > x1 && ic.y < y2 && ic.y + ICON_SIZE > y1;
                }
                changed = true;
            }
        }

        // Birakma
        if !lbtn && pl { DRAGGING = -1; SELECTING = false; changed = true; }
        changed
    }
}

#[allow(static_mut_refs)]
fn do_action(item: &str) {
    unsafe {
        match item {
            "Yeni Dosya" => {
                for k in 0..20u32 {
                    let name = if k == 0 { String::from("YENI.TXT") } else { format!("YENI{}.TXT", k + 1) };
                    if !icons().iter().any(|i| i.label.eq_ignore_ascii_case(&name)) {
                        let _ = syscall::sys_create_file(&format!("{}/{}", desktop_dir(), name));
                        break;
                    }
                }
                rebuild();
            }
            "Yeni Klasor" => {
                for k in 0..20u32 {
                    let name = if k == 0 { String::from("KLASOR") } else { format!("KLASOR{}", k + 1) };
                    if !icons().iter().any(|i| i.label.eq_ignore_ascii_case(&name)) {
                        let _ = syscall::sys_create_dir(&format!("{}/{}", desktop_dir(), name));
                        break;
                    }
                }
                rebuild();
            }
            "Yenile" => rebuild(),
            "Sil" => {
                let hedef_secili = CTARGET >= 0 && (CTARGET as usize) < icons().len()
                    && icons()[CTARGET as usize].selected;
                if hedef_secili {
                    let paths: Vec<String> = icons().iter()
                        .filter(|i| i.selected && i.kind >= 2)
                        .map(|i| format!("{}/{}", desktop_dir(), i.label))
                        .collect();
                    for p in paths { let _ = syscall::sys_delete_file(&p); }
                } else if CTARGET >= 0 && (CTARGET as usize) < icons().len()
                    && icons()[CTARGET as usize].kind >= 2 {
                    let path = format!("{}/{}", desktop_dir(), icons()[CTARGET as usize].label);
                    let _ = syscall::sys_delete_file(&path);
                }
                rebuild();
            }
            "Ad Degistir" => {
                if CTARGET >= 0 {
                    RENAME_ON = true; RENAME_IDX = CTARGET;
                    RENAME_BUF = Some(String::new());
                }
            }
            "Ac" => { if CTARGET >= 0 { open_icon(CTARGET); } }
            _ => {}
        }
    }
}

#[allow(static_mut_refs)]
pub fn draw_icons(r: &Renderer) {
    for (i, ic) in icons().iter().enumerate() {
        let x = ic.x.max(0) as usize;
        let y = ic.y.max(0) as usize;
        let s = ICON_SIZE as usize;
        if ic.selected {
            r.fill_rect_alpha(x.saturating_sub(4), y.saturating_sub(4), s + 8, s + 22, 0x00FFB050, 70);
        }
        r.fill_rounded_glossy(x, y, s, s, 8, ic.ct, ic.cb);
        r.fill_rect_alpha(x + 4, y + 4, s - 8, 6, 0x00FFFFFF, 60);
        unsafe {
            if RENAME_ON && RENAME_IDX == i as i32 {
                let bw = 90usize;
                let bx = (x + s / 2).saturating_sub(bw / 2);
                let by = y + s + 4;
                r.fill_rect(bx, by, bw, 14, 0x00FFFFFF);
                r.fill_rect(bx, by, bw, 1, 0x00FF8020);
                r.fill_rect(bx, by + 13, bw, 1, 0x00FF8020);
                let t = RENAME_BUF.as_deref().unwrap_or("");
                r.draw_text(t, bx + 3, by + 3, 0x00201008, 1);
                r.draw_text("_", bx + 3 + t.len() * 7, by + 3, 0x00C04408, 1);
                continue;
            }
        }
        let lw = ic.label.len() * 7;
        let lx = (x + s / 2).saturating_sub(lw / 2);
        let ly = y + s + 6;
        r.draw_text(&ic.label, lx + 1, ly + 1, 0x00000000, 1);
        r.draw_text(&ic.label, lx, ly, theme::ICON_TEXT, 1);
    }
}

pub fn draw_selection(r: &Renderer) {
    unsafe {
        if !SELECTING { return; }
        let x1 = SX.min(CX); let y1 = SY.min(CY);
        let x2 = SX.max(CX); let y2 = SY.max(CY);
        if x2 <= x1 || y2 <= y1 { return; }
        let w = (x2 - x1) as usize; let h = (y2 - y1) as usize;
        r.fill_rect_alpha(x1 as usize, y1 as usize, w, h, 0x00FF9030, 50);
        r.fill_rect(x1 as usize, y1 as usize, w, 1, 0x00FFB050);
        r.fill_rect(x1 as usize, y2 as usize, w, 1, 0x00FFB050);
        r.fill_rect(x1 as usize, y1 as usize, 1, h, 0x00FFB050);
        r.fill_rect(x2 as usize, y1 as usize, 1, h, 0x00FFB050);
    }
}

pub fn draw_context_menu(r: &Renderer) {
    unsafe {
        if !CMENU { return; }
        let items = menu_items();
        let mw = 150usize; let ih = 26usize;
        let mh = items.len() * ih + 8;
        let x = CMX.max(0) as usize; let y = CMY.max(0) as usize;
        r.fill_rect_alpha(x + 3, y + 3, mw, mh, 0x00000000, 80);
        r.fill_rounded(x, y, mw, mh, 6, 0x00301008);
        r.draw_rounded_border(x, y, mw, mh, 6, 0x00FF8020);
        let mut iy = y + 8;
        for it in items {
            r.draw_text(it, x + 14, iy, 0x00FFE0C0, 1);
            iy += ih;
        }
    }
}
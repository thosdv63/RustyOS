use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use crate::ui::app_mgr;

const SIDE_W: usize   = 156;
const TOOLBAR_H: usize = 34;
const ADDR_BOT: usize = 62;
const LIST_TOP: usize = 84;
const ROW_H: usize    = 22;
const STATUS_H: usize = 24;

// kind: 0 dosya, 1 klasor, 2 surucu
#[derive(Clone)]
struct Item { name: String, kind: u8, size: u32, dkind: u8 }

pub struct Explorer {
    path: String,
    items: Vec<Item>,
    drives: Vec<String>,
    loaded: bool,
    sel: i32,
    last: i32,
    hist: Vec<String>,
    hidx: usize,
    cm_on: bool, cm_x: i32, cm_y: i32, cm_t: i32,
    clip: Option<String>,
    ren_on: bool, ren_buf: String,
    confirm: Option<String>,
    msg: Option<&'static str>,
    vw: usize,
    vh: usize,
}

impl Explorer {
    pub fn new() -> Self { Self::new_at(String::new()) }

    pub fn new_at(path: String) -> Self {
        let mut hist = Vec::new();
        hist.push(path.clone());
        Explorer {
            path, items: Vec::new(), drives: Vec::new(), loaded: false,
            sel: -1, last: -1, hist, hidx: 0,
            cm_on: false, cm_x: 0, cm_y: 0, cm_t: -1, clip: None,
            ren_on: false, ren_buf: String::new(),
            confirm: None, msg: None, vw: 0, vh: 0,
        }
    }

    fn full(&self, name: &str) -> String {
        if self.path.is_empty() { String::from(name) } else { format!("{}/{}", self.path, name) }
    }

    fn parse(path: &str) -> Vec<Item> {
        let mut buf = vec![0u8; 4096];
        let n = syscall::sys_list_dir(path, &mut buf) as usize;
        let mut out: Vec<Item> = Vec::new();
        for i in 0..n {
            let off = i * 40;
            if off + 40 > buf.len() { break; }
            let mut end = off;
            while end < off + 32 && buf[end] != 0 { end += 1; }
            if let Ok(name) = core::str::from_utf8(&buf[off..end]) {
                let k = buf[off + 33];
                let kind = if k == 2 { 2 } else if buf[off + 32] == 1 { 1 } else { 0 };
                let dkind = buf[off + 34];
                let size = u32::from_le_bytes([buf[off+36], buf[off+37], buf[off+38], buf[off+39]]);
                out.push(Item { name: String::from(name), kind, size, dkind });
            }
        }
        out.sort_by(|a, b| b.kind.cmp(&a.kind));
        out
    }

    fn refresh_drives(&mut self) {
        self.drives.clear();
        for it in Self::parse("").iter() {
            if it.kind == 2 { self.drives.push(it.name.clone()); }
        }
    }

    fn refresh(&mut self) {
        self.items = Self::parse(&self.path);
        self.sel = -1; self.last = -1; self.cm_on = false; self.ren_on = false;
        self.loaded = true;
    }

    fn navigate(&mut self, p: String) {
        if p == self.path { self.refresh(); return; }
        self.hist.truncate(self.hidx + 1);
        self.hist.push(p.clone());
        self.hidx = self.hist.len() - 1;
        self.path = p;
        self.refresh();
    }

    fn back(&mut self) {
        if self.hidx > 0 {
            self.hidx -= 1;
            self.path = self.hist[self.hidx].clone();
            self.refresh();
        }
    }
    fn forward(&mut self) {
        if self.hidx + 1 < self.hist.len() {
            self.hidx += 1;
            self.path = self.hist[self.hidx].clone();
            self.refresh();
        }
    }
    fn up(&mut self) {
        if self.path.is_empty() { return; }
        if self.path.ends_with(':') { self.navigate(String::new()); return; }
        match self.path.rfind('/') {
            Some(i) => { let p = String::from(&self.path[..i]); self.navigate(p); }
            None => self.navigate(String::new()),
        }
    }

    fn open(&mut self, idx: usize) {
        if idx >= self.items.len() { return; }
        let it = self.items[idx].clone();
        match it.kind {
            2 => self.navigate(it.name.clone()),
            1 => { let p = self.full(&it.name); self.navigate(p); }
            _ => {
                let up = it.name.to_ascii_uppercase();
                if up.ends_with(".BMP") {
                    app_mgr::request_app(8, self.full(&it.name));
                    return;
                }
                if up.ends_with(".RAW") {
                    // Ham PCM ses dosyasi -> ses surucusune gonder
                    if syscall::sys_play_file(&self.full(&it.name)) != 0 {
                        self.msg = Some("Ses dosyasi calinamadi.");
                    }
                    return;
                }
                if up.ends_with(".TXT") || up.ends_with(".DAT") || up.ends_with(".LOG")
                    || up.ends_with(".INI") || up.ends_with(".CFG") || !up.contains('.')
                {
                    app_mgr::request_app(6, self.full(&it.name));
                } else {
                    self.msg = Some("Bu dosya turu acilamiyor.");
                }
            }
        }
    }

    fn crumb(&self) -> String {
        if self.path.is_empty() { return String::from("Bilgisayar"); }
        let mut s = String::from("Bilgisayar");
        for p in self.path.split('/') {
            if p.is_empty() { continue; }
            s.push_str(" > ");
            if p.ends_with(':') { s.push_str(&format!("Yerel Disk ({})", p)); }
            else { s.push_str(p); }
        }
        s
    }

    // (etiket, hedef) - hedef None ise tiklanamaz baslik
    fn nav(&self) -> Vec<(String, Option<String>)> {
        let sys = self.drives.get(0).cloned().unwrap_or(String::from("C:"));
        let mut v: Vec<(String, Option<String>)> = Vec::new();
        v.push((String::from("Favoriler"), None));
        v.push((String::from("Masaustu"), Some(format!("{}/Users/User/Desktop", sys))));
        v.push((String::from("Belgeler"), Some(format!("{}/Users/User/Documents", sys))));
        v.push((String::from("Indirilenler"), Some(format!("{}/Users/User/Downloads", sys))));
        v.push((String::new(), None));
        v.push((String::from("Bilgisayar"), Some(String::new())));
        for d in self.drives.iter() {
            v.push((format!("Yerel Disk ({})", d), Some(d.clone())));
        }
        v
    }

    fn menu_items(&self) -> &'static [&'static str] {
        if self.path.is_empty() {
            if self.cm_t >= 0 { return &["Ac"]; }
            return &["Yenile"];
        }
        if self.cm_t < 0 { return &["Yapistir", "Yenile"]; }
        match self.items.get(self.cm_t as usize).map(|e| e.kind).unwrap_or(0) {
            2 => &["Ac"],
            1 => &["Ac", "Kes", "Sil", "Ad Degistir"],
            _ => {
                let raw = self.items.get(self.cm_t as usize)
                    .map(|e| e.name.to_ascii_uppercase().ends_with(".RAW"))
                    .unwrap_or(false);
                if raw { &["Cal", "Dur", "Kes", "Sil", "Ad Degistir"] }
                else { &["Kes", "Sil", "Ad Degistir"] }
            }
        }
    }

    fn action(&mut self, item: &str) {
        match item {
            "Cal" => {
                if self.cm_t >= 0 {
                    let name = self.items[self.cm_t as usize].name.clone();
                    if syscall::sys_play_file(&self.full(&name)) != 0 {
                        self.msg = Some("Ses dosyasi calinamadi.");
                    }
                }
            }
            "Dur" => { syscall::sys_stop_sound(); }
            "Ac" => { if self.cm_t >= 0 { self.open(self.cm_t as usize); } }
            "Kes" => {
                if self.cm_t >= 0 {
                    let name = self.items[self.cm_t as usize].name.clone();
                    self.clip = Some(self.full(&name));
                }
            }
            "Yapistir" => {
                if self.path.is_empty() { self.msg = Some("Buraya yapistirilamaz!"); return; }
                if let Some(src) = self.clip.take() {
                    let rc = syscall::sys_move(&src, &self.path);
                    if rc == 3 { self.msg = Some("Farkli surucular arasi tasima yok!"); }
                    else if rc == 2 { self.msg = Some("Bu ogeyi tasima izniniz yok!"); }
                    else if rc != 0 { self.msg = Some("Tasima basarisiz!"); }
                    self.refresh();
                }
            }
            "Sil" => {
                if self.cm_t >= 0 {
                    let name = self.items[self.cm_t as usize].name.clone();
                    let full = self.full(&name);
                    if is_critical(&full) {
                        let yetki = syscall::sys_reg_get_id(2);
                        if yetki == 1 { self.confirm = Some(full); }
                        else { self.msg = Some("Bu dosyayi silme izniniz yok!"); }
                    } else {
                        let rc = syscall::sys_delete_file(&full);
                        if rc == 2 { self.msg = Some("Bu dosyayi silme izniniz yok!"); }
                        self.refresh();
                    }
                }
            }
            "Ad Degistir" => {
                if self.cm_t >= 0 { self.sel = self.cm_t; self.ren_on = true; self.ren_buf.clear(); }
            }
            "Yenile" => { self.refresh_drives(); self.refresh(); }
            _ => {}
        }
    }
}

fn round_btn(r: &Renderer, x: usize, y: usize, s: usize, label: &str, on: bool) {
    let (a, b) = if on { (0x00FFA850, 0x00D86818) } else { (0x00E2DAD2, 0x00BAB2AA) };
    r.fill_rounded_glossy(x, y, s, s, s / 2, a, b);
    let lw = label.len() * 7;
    r.draw_text(label, x + s / 2 - lw / 2, y + s / 2 - 4, 0x00FFFFFF, 1);
}

fn disp(it: &Item) -> String {
    if it.kind == 2 {
        if it.dkind == 1 { format!("Cikarilabilir Disk ({})", it.name) }
        else { format!("Yerel Disk ({})", it.name) }
    } else { it.name.clone() }
}

fn type_str(it: &Item) -> &'static str {
    match it.kind {
        2 => if it.dkind == 1 { "Cikarilabilir Disk" } else { "Yerel Disk" },
        1 => "Dosya klasoru",
        _ => {
            let up = it.name.to_ascii_uppercase();
            if up.ends_with(".TXT") { "Metin Belgesi" }
            else if up.ends_with(".BIN") { "Ikili Dosya" }
            else if up.ends_with(".DAT") { "Veri Dosyasi" }
            else if up.ends_with(".ELF") { "Sistem Dosyasi" }
            else if up.ends_with(".BMP") { "Resim Dosyasi" }
            else if up.ends_with(".RAW") { "Ses Dosyasi" }
            else { "Dosya" }
        }
    }
}
fn size_str(it: &Item) -> String {
    match it.kind {
        2 => format!("{} MB", it.size),
        1 => String::new(),
        _ => {
            if it.size < 1024 { format!("{} B", it.size) }
            else { format!("{} KB", (it.size + 1023) / 1024) }
        }
    }
}

impl App for Explorer {
    fn title(&self) -> &'static str { "Gezgin" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w;
        self.vh = h;
        if !self.loaded { self.refresh_drives(); self.refresh(); }
        if w < 340 || h < 200 { return; }

        let status_top = h - STATUS_H;
        let lw = w - SIDE_W;

        r.fill_rect(x, y, w, h, 0x00FFFFFF);

        // === Toolbar ===
        r.fill_gradient(x, y, w, TOOLBAR_H, 0x00F9F4EF, 0x00E5DACE);
        r.fill_rect(x, y + TOOLBAR_H - 1, w, 1, 0x00C9B9A9);

        let can_back = self.hidx > 0;
        let can_fwd = self.hidx + 1 < self.hist.len();
        round_btn(r, x + 8,  y + 6, 22, "<", can_back);
        round_btn(r, x + 34, y + 6, 22, ">", can_fwd);
        round_btn(r, x + 66, y + 6, 22, "^", !self.path.is_empty());

        r.fill_rounded_glossy(x + 96, y + 6, 62, 22, 5, 0x00FFA850, 0x00D86818);
        r.draw_text("Yenile", x + 103, y + 13, 0x00FFFFFF, 1);

        if let Some(c) = &self.clip {
            let short = c.rsplit('/').next().unwrap_or(c);
            r.draw_text("Pano:", x + 170, y + 13, 0x00A05010, 1);
            r.draw_text(short, x + 210, y + 13, 0x00A05010, 1);
        }

        // === Adres cubugu ===
        r.fill_gradient(x, y + TOOLBAR_H, w, ADDR_BOT - TOOLBAR_H, 0x00F3EDE7, 0x00E9E1D9);
        let abw = lw + SIDE_W - 160;
        r.fill_rect(x + 8, y + TOOLBAR_H + 3, abw, 22, 0x00FFFFFF);
        r.draw_rounded_border(x + 8, y + TOOLBAR_H + 3, abw, 22, 3, 0x00C4B4A4);
        let cr = self.crumb();
        let maxc = (abw - 16) / 7;
        let shown = if cr.len() > maxc { &cr[cr.len() - maxc..] } else { &cr[..] };
        r.draw_text(shown, x + 14, y + TOOLBAR_H + 10, 0x00403028, 1);

        r.fill_rect(x + w - 146, y + TOOLBAR_H + 3, 138, 22, 0x00FFFFFF);
        r.draw_rounded_border(x + w - 146, y + TOOLBAR_H + 3, 138, 22, 3, 0x00C4B4A4);
        r.draw_text("Ara...", x + w - 138, y + TOOLBAR_H + 10, 0x00A89888, 1);

        // === Sol nav paneli ===
        r.fill_gradient(x, y + ADDR_BOT, SIDE_W, status_top - ADDR_BOT, 0x00F7F1EB, 0x00EDE4DA);
        r.fill_rect(x + SIDE_W - 1, y + ADDR_BOT, 1, status_top - ADDR_BOT, 0x00D9C9B9);
        let nav = self.nav();
        let mut ny = y + ADDR_BOT + 6;
        for (label, target) in nav.iter() {
            if label.is_empty() { ny += 12; continue; }
            match target {
                None => { r.draw_text(label, x + 10, ny + 6, 0x00907058, 1); }
                Some(t) => {
                    if *t == self.path {
                        r.fill_rect_alpha(x + 4, ny, SIDE_W - 10, 20, 0x00FF9030, 70);
                        r.draw_rounded_border(x + 4, ny, SIDE_W - 10, 20, 3, 0x00E08830);
                    }
                    r.fill_rounded_glossy(x + 12, ny + 5, 10, 10, 2, 0x00FFC850, 0x00E09020);
                    r.draw_text(label, x + 28, ny + 6, 0x00403028, 1);
                }
            }
            ny += ROW_H;
        }

        // === Sutun basliklari ===
        let x0 = x + SIDE_W;
        r.fill_gradient(x0, y + ADDR_BOT, lw, LIST_TOP - ADDR_BOT, 0x00FCF8F4, 0x00EDE6DF);
        r.fill_rect(x0, y + LIST_TOP - 1, lw, 1, 0x00D9C9B9);
        let type_x = x0 + lw - 210;
        let size_x = x0 + lw - 96;
        r.draw_text("Ad", x0 + 34, y + ADDR_BOT + 7, 0x00706050, 1);
        r.draw_text("Tur", type_x, y + ADDR_BOT + 7, 0x00706050, 1);
        r.draw_text("Boyut", size_x, y + ADDR_BOT + 7, 0x00706050, 1);
        r.fill_rect(type_x - 10, y + ADDR_BOT + 4, 1, 14, 0x00D9C9B9);
        r.fill_rect(size_x - 10, y + ADDR_BOT + 4, 1, 14, 0x00D9C9B9);

        // === Liste ===
        let rows = (status_top - LIST_TOP) / ROW_H;
        let mut ry = y + LIST_TOP + 2;
        for (i, it) in self.items.iter().enumerate() {
            if i >= rows { break; }
            if i as i32 == self.sel {
                r.fill_rect_alpha(x0 + 2, ry - 2, lw - 6, 20, 0x00FF9030, 70);
                r.draw_rounded_border(x0 + 2, ry - 2, lw - 6, 20, 3, 0x00E08830);
            }
            match it.kind {
                2 => {
                    if it.dkind == 1 {
                        r.fill_rounded_glossy(x0 + 10, ry, 16, 16, 3, 0x00FFC060, 0x00C07010);
                        r.fill_rect(x0 + 15, ry + 3, 6, 5, 0x00FFFFFF);
                    } else {
                        r.fill_rounded_glossy(x0 + 8, ry + 1, 20, 14, 3, 0x00E8E2DC, 0x00988E84);
                        r.fill_rect(x0 + 11, ry + 10, 14, 3, 0x00FF9030);
                    }
                }
                1 => { r.fill_rounded_glossy(x0 + 9, ry + 1, 18, 14, 3, 0x00FFC850, 0x00E09020); }
                _ => {
                    r.fill_rounded_glossy(x0 + 11, ry, 14, 16, 2, 0x00FFFFFF, 0x00DCD4CC);
                    r.draw_rounded_border(x0 + 11, ry, 14, 16, 2, 0x00A89E94);
                }
            }

            if self.ren_on && i as i32 == self.sel {
                r.fill_rect(x0 + 34, ry, 130, 16, 0x00FFFFFF);
                r.draw_rounded_border(x0 + 34, ry, 130, 16, 2, 0x00FF8020);
                r.draw_text(&self.ren_buf, x0 + 37, ry + 4, 0x00201008, 1);
                r.draw_text("_", x0 + 37 + self.ren_buf.len() * 7, ry + 4, 0x00C04408, 1);
            } else {
                let d = disp(it);
                let maxn = (type_x - (x0 + 34)) / 7;
                let nm = if d.len() > maxn { &d[..maxn] } else { &d[..] };
                r.draw_text(nm, x0 + 34, ry + 4, 0x00403028, 1);
            }

            r.draw_text(type_str(it), type_x, ry + 4, 0x00887868, 1);
            let ss = size_str(it);
            if !ss.is_empty() {
                r.draw_text(&ss, size_x + 80 - ss.len() * 7, ry + 4, 0x00887868, 1);
            }
            ry += ROW_H;
        }
        if self.items.is_empty() {
            r.draw_text("(bu klasor bos)", x0 + 34, y + LIST_TOP + 10, 0x00A09080, 1);
        }

        // === Durum cubugu ===
        r.fill_gradient(x, y + status_top, w, STATUS_H, 0x00F5EFE9, 0x00E3D9CF);
        r.fill_rect(x, y + status_top, w, 1, 0x00D9C9B9);
        let st = if self.sel >= 0 && (self.sel as usize) < self.items.len() {
            let it = &self.items[self.sel as usize];
            format!("{} oge  |  {}  {}", self.items.len(), disp(it), size_str(it))
        } else {
            format!("{} oge", self.items.len())
        };
        r.draw_text(&st, x + 10, y + status_top + 8, 0x00605040, 1);

        // === Sag tik menusu ===
        if self.cm_on {
            let items = self.menu_items();
            let mw = 130usize; let ih = 24usize;
            let mh = items.len() * ih + 8;
            let mx2 = x + self.cm_x.max(0) as usize;
            let my2 = y + self.cm_y.max(0) as usize;
            r.fill_rect_alpha(mx2 + 3, my2 + 3, mw, mh, 0x00000000, 80);
            r.fill_rounded(mx2, my2, mw, mh, 6, 0x00301008);
            r.draw_rounded_border(mx2, my2, mw, mh, 6, 0x00FF8020);
            let mut iy = my2 + 6;
            for it in items { r.draw_text(it, mx2 + 12, iy, 0x00FFE0C0, 1); iy += ih; }
        }

        if self.confirm.is_some() {
            let dx = x + w / 2 - 150; let dy = y + h / 2 - 50;
            r.fill_rect_alpha(x, y, w, h, 0x00000000, 90);
            r.fill_rounded(dx, dy, 300, 100, 8, 0x00301008);
            r.draw_rounded_border(dx, dy, 300, 100, 8, 0x00FF8020);
            r.draw_text("SISTEM DOSYASI!", dx + 16, dy + 12, 0x00FF6040, 1);
            r.draw_text("Silmek istediginize emin misiniz?", dx + 16, dy + 32, 0x00FFE0C0, 1);
            r.fill_rounded_glossy(dx + 30, dy + 60, 100, 28, 5, 0x00F04020, 0x00A01808);
            r.draw_text("Evet", dx + 62, dy + 68, 0x00FFFFFF, 1);
            r.fill_rounded_glossy(dx + 170, dy + 60, 100, 28, 5, 0x00FFA850, 0x00D86818);
            r.draw_text("Hayir", dx + 200, dy + 68, 0x00FFFFFF, 1);
        }

        if let Some(m) = self.msg {
            let dx = x + w / 2 - 150; let dy = y + h / 2 - 40;
            r.fill_rounded(dx, dy, 300, 70, 8, 0x00301008);
            r.draw_rounded_border(dx, dy, 300, 70, 8, 0x00F04020);
            r.draw_text("ERISIM ENGELLENDI", dx + 16, dy + 12, 0x00FF6040, 1);
            r.draw_text(m, dx + 16, dy + 36, 0x00FFE0C0, 1);
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        let vh = self.vh as i32;
        let status_top = vh - STATUS_H as i32;

        match ev {
            AppEvent::Key { ch } => {
                if !self.ren_on { return false; }
                match *ch {
                    '\n' => {
                        if self.sel >= 0 && !self.ren_buf.is_empty() {
                            let old = self.items[self.sel as usize].name.clone();
                            let newn = self.ren_buf.clone();
                            let rc = syscall::sys_rename(&self.full(&old), &newn);
                            if rc == 2 { self.msg = Some("Ad degistirme izniniz yok!"); }
                        }
                        self.ren_on = false; self.refresh();
                    }
                    '\u{1b}' => { self.ren_on = false; }
                    '\u{8}' => { self.ren_buf.pop(); }
                    c if c as u32 >= 32 => { if self.ren_buf.len() < 12 { self.ren_buf.push(c); } }
                    _ => {}
                }
                true
            }

            AppEvent::RClick { x, y } => {
                self.ren_on = false;
                self.cm_t = -1;
                if *x >= SIDE_W as i32 && *y >= LIST_TOP as i32 && *y < status_top {
                    let idx = ((*y - LIST_TOP as i32) / ROW_H as i32) as usize;
                    if idx < self.items.len() { self.sel = idx as i32; self.cm_t = idx as i32; }
                    else { self.sel = -1; }
                }
                self.cm_on = true; self.cm_x = *x; self.cm_y = *y;
                true
            }

            AppEvent::Click { x, y } => {
                if let Some(path) = self.confirm.clone() {
                    let dx = (self.vw as i32) / 2 - 150;
                    let dy = vh / 2 - 50;
                    if *y >= dy + 60 && *y < dy + 88 {
                        if *x >= dx + 30 && *x < dx + 130 {
                            let rc = syscall::sys_delete_file(&path);
                            self.confirm = None;
                            if rc == 2 { self.msg = Some("Bu dosyayi silme izniniz yok!"); }
                            self.refresh();
                            return true;
                        }
                        if *x >= dx + 170 && *x < dx + 270 { self.confirm = None; return true; }
                    }
                    return true;
                }
                if self.msg.is_some() { self.msg = None; return true; }

                if self.cm_on {
                    let items = self.menu_items();
                    let mw = 130; let ih = 24;
                    let mh = items.len() as i32 * ih + 8;
                    if *x >= self.cm_x && *x < self.cm_x + mw && *y >= self.cm_y && *y < self.cm_y + mh {
                        let idx = ((*y - self.cm_y - 4) / ih) as usize;
                        if idx < items.len() {
                            let it = items[idx];
                            self.cm_on = false;
                            self.action(it);
                            return true;
                        }
                    }
                    self.cm_on = false;
                    return true;
                }

                // Toolbar
                if *y < TOOLBAR_H as i32 {
                    self.last = -1;
                    if *y >= 6 && *y < 28 {
                        if *x >= 8  && *x < 30  { self.back(); return true; }
                        if *x >= 34 && *x < 56  { self.forward(); return true; }
                        if *x >= 66 && *x < 88  { self.up(); return true; }
                        if *x >= 96 && *x < 158 { self.refresh_drives(); self.refresh(); return true; }
                    }
                    return false;
                }

                // Adres cubugu bandi
                if *y < ADDR_BOT as i32 { return false; }

                // Sol panel
                if *x < SIDE_W as i32 && *y < status_top {
                    if self.ren_on { self.ren_on = false; }
                    let nav = self.nav();
                    let mut ny = ADDR_BOT as i32 + 6;
                    for (label, target) in nav.iter() {
                        if label.is_empty() { ny += 12; continue; }
                        if *y >= ny && *y < ny + ROW_H as i32 {
                            if let Some(t) = target { let t = t.clone(); self.navigate(t); }
                            return true;
                        }
                        ny += ROW_H as i32;
                    }
                    return true;
                }

                if self.ren_on { self.ren_on = false; return true; }

                // Liste
                if *y >= LIST_TOP as i32 && *y < status_top {
                    let idx = ((*y - LIST_TOP as i32) / ROW_H as i32) as usize;
                    if idx < self.items.len() {
                        if self.last == idx as i32 {
                            self.last = -1;
                            self.open(idx);
                            return true;
                        }
                        self.sel = idx as i32; self.last = idx as i32;
                        return true;
                    }
                    self.sel = -1; self.last = -1;
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

fn is_critical(path: &str) -> bool {
    let up = path.to_ascii_uppercase();
    let b = up.as_bytes();
    let p: &str = if b.len() >= 2 && b[1] == b':' {
        if b.len() > 2 { &up[3..] } else { "" }
    } else { &up[..] };
    if p.is_empty() { return true; }
    p == "RSYS" || p.starts_with("RSYS/") || p == "USERS" || p == "APPS"
        || p.ends_with("CORE.BIN") || p.ends_with("KERNEL.ELF") || p.ends_with("REGISTRY.DAT")
}
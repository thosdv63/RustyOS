use crate::renderer::Renderer;
use crate::apps::app_compiler::{App, AppEvent};
use crate::syscall;
use crate::ui::app_mgr;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::vec;
use alloc::format;
use crate::alloc::string::ToString;


const LINE_H: usize = 13;
const MAX_LINES: usize = 300;
const MAX_INPUT: usize = 96;

const PALETTE: [u32; 16] = [
    0x00000000, 0x00000080, 0x00008000, 0x00008080,
    0x00800000, 0x00800080, 0x00808000, 0x00C0C0C0,
    0x00808080, 0x000000FF, 0x0000FF00, 0x0000FFFF,
    0x00FF0000, 0x00FF00FF, 0x00FFFF00, 0x00FFFFFF,
];

pub struct Cmd {
    lines: Vec<String>,
    input: String,
    cwd: String,
    fg: u32,
    bg: u32,
    scroll: usize, 
    init: bool,
    ver: String,
    vw: usize,
    vh: usize,
}

impl Cmd {
    pub fn new() -> Self {
        Cmd {
            lines: Vec::new(), input: String::new(), cwd: String::new(),
            fg: PALETTE[7], bg: PALETTE[0], scroll: 0, init: false,
            ver: String::new(), vw: 0, vh: 0,
        }
    }

    fn out(&mut self, s: &str) {
        for l in s.split('\n') { self.lines.push(String::from(l)); }
        if self.lines.len() > MAX_LINES {
            let cut = self.lines.len() - MAX_LINES;
            self.lines.drain(..cut);
        }
    }

    // sys_list_dir 
    fn list(path: &str) -> Vec<(String, u8, u32)> {
        let mut buf = vec![0u8; 4096];
        let n = syscall::sys_list_dir(path, &mut buf) as usize;
        let mut out = Vec::new();
        for i in 0..n {
            let off = i * 40;
            if off + 40 > buf.len() { break; }
            let mut end = off;
            while end < off + 32 && buf[end] != 0 { end += 1; }
            if let Ok(name) = core::str::from_utf8(&buf[off..end]) {
                let kind = if buf[off + 33] == 2 { 2 } else if buf[off + 32] == 1 { 1 } else { 0 };
                let size = u32::from_le_bytes([buf[off+36], buf[off+37], buf[off+38], buf[off+39]]);
                out.push((String::from(name), kind, size));
            }
        }
        out
    }

    fn normalize(p: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in p.split('/') {
            if part.is_empty() || part == "." { continue; }
            if part == ".." {
                if parts.len() > 1 { parts.pop(); } // "C:" always
            } else { parts.push(part); }
        }
        parts.join("/")
    }

    fn resolve(&self, arg: &str) -> String {
        let a = arg.trim().replace('\\', "/");
        let a = a.trim_end_matches('/');
        let b = a.as_bytes();
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            return Self::normalize(&(a.to_ascii_uppercase()[..2].to_string() + &a[2..]));
        }
        if a.starts_with('/') {
            let drive = if self.cwd.len() >= 2 { &self.cwd[..2] } else { "C:" };
            return Self::normalize(&format!("{}{}", drive, a));
        }
        Self::normalize(&format!("{}/{}", self.cwd, a))
    }

    fn dir_exists(path: &str) -> bool {
        if path.len() == 2 && path.ends_with(':') {
            return Self::list("").iter().any(|e| e.1 == 2 && e.0.eq_ignore_ascii_case(path));
        }
        match path.rfind('/') {
            Some(i) => Self::list(&path[..i]).iter()
                .any(|e| e.1 == 1 && e.0.eq_ignore_ascii_case(&path[i+1..])),
            None => false,
        }
    }

    fn startup(&mut self) {
        // System disk (first disk)
        let drives = Self::list("");
        self.cwd = drives.iter().find(|e| e.1 == 2)
            .map(|e| e.0.clone()).unwrap_or_else(|| String::from("C:"));

        // Name from registry, version
        let mut dump = vec![0u8; 8192];
        let n = syscall::sys_reg_list(&mut dump) as usize;
        let mut ad = String::from("Rusty OS");
        let mut sr = String::from("0.1");
        if let Ok(t) = core::str::from_utf8(&dump[..n.min(8192)]) {
            for line in t.lines() {
                if let Some(v) = line.trim().strip_prefix("Sistem/Ad=str:") { ad = String::from(v); }
                if let Some(v) = line.trim().strip_prefix("Sistem/Surum=str:") { sr = String::from(v); }
            }
        }
        self.ver = format!("{} [Surum {}]", ad, sr);
        let banner = self.ver.clone();
        self.out(&banner);
        self.out("(c) Rusty. Tum haklari saklidir.");
        self.out("");
        self.init = true;
    }

    fn exec(&mut self, raw: String) {
        let line = raw.trim();
        let echo = format!("{}>{}", self.cwd, line);
        self.out(&echo);
        if line.is_empty() { return; }

        let (head, rest) = match line.find(' ') {
            Some(i) => (&line[..i], line[i+1..].trim()),
            None => (line, ""),
        };
        let cmd = head.to_ascii_lowercase();

        if cmd.len() == 2 && cmd.as_bytes()[1] == b':' && cmd.as_bytes()[0].is_ascii_alphabetic() {
            let d = cmd.to_ascii_uppercase();
            if Self::dir_exists(&d) { self.cwd = d; }
            else { self.out("Sistem belirtilen surucuyu bulamiyor."); }
            return;
        }

        match cmd.as_str() {
            "help" => {
                self.out("CD       Dizin degistir           (cd .., cd /, cd Users)");
                self.out("CLS      Ekrani temizle");
                self.out("COLOR    Renk ayarla              (color 0A, color = sifirla)");
                self.out("COPY     Dosya kopyala            (copy a.txt b.txt)");
                self.out("DATE     Tarihi goster");
                self.out("DEL/RD   Dosya/klasor sil");
                self.out("DIR      Dizin icerigini listele");
                self.out("ECHO     Metin yaz");
                self.out("EXIT     Pencereyi kapat");
                self.out("MKDIR    Klasor olustur");
                self.out("MOVE     Tasi                     (move a.txt C:/Users)");
                self.out("REG      Kayit defterini listele  (reg [filtre])");
                self.out("REN      Yeniden adlandir         (ren eski.txt YENI.TXT)");
                self.out("SHUTDOWN Kapat (/r = yeniden baslat)");
                self.out("START    Uygulama ac              (start paint, start notepad X.TXT)");
                self.out("TASKLIST Acik pencereleri listele");
                self.out("TASKKILL Pencere kapat            (taskkill 3)");
                self.out("TIME     Saati goster");
                self.out("TYPE     Dosya icerigini yaz");
                self.out("VER      Surum bilgisi");
                self.out("");
                self.out("Kaydirma: ust kenara tikla = yukari, alt kenara = asagi");
            }
            "ver" => { let v = self.ver.clone(); self.out(&v); }
            "cls" => { self.lines.clear(); self.scroll = 0; }
            "echo" => { self.out(rest); }
            "time" => {
                let mut t: [i32; 6] = [0; 6];
                syscall::sys_get_time(t.as_mut_ptr() as u64);
                let (h, m, s) = unsafe {
                    (core::ptr::read_volatile(&t[0]),
                     core::ptr::read_volatile(&t[1]),
                     core::ptr::read_volatile(&t[2]))
                };
                self.out(&format!("Gecerli saat: {:02}:{:02}:{:02}", h, m, s));
            }
            "date" => {
                let mut t: [i32; 6] = [0; 6];
                syscall::sys_get_time(t.as_mut_ptr() as u64);
                let (d, mo, y) = unsafe {
                    (core::ptr::read_volatile(&t[3]),
                     core::ptr::read_volatile(&t[4]),
                     core::ptr::read_volatile(&t[5]))
                };
                self.out(&format!("Gecerli tarih: {:02}.{:02}.20{:02}", d, mo, y));
            }
            "cd" | "chdir" => {
                if rest.is_empty() { let c = self.cwd.clone(); self.out(&c); }
                else {
                    let t = self.resolve(rest);
                    if Self::dir_exists(&t) { self.cwd = t; }
                    else { self.out("Sistem belirtilen yolu bulamiyor."); }
                }
            }
            "dir" => {
                let target = if rest.is_empty() { self.cwd.clone() } else { self.resolve(rest) };
                if !rest.is_empty() && !Self::dir_exists(&target) {
                    self.out("Sistem belirtilen yolu bulamiyor.");
                    return;
                }
                let items = Self::list(&target);
                self.out(&format!(" {} dizini", target));
                self.out("");
                let (mut nf, mut nd, mut total) = (0u32, 0u32, 0u64);
                for it in items.iter() {
                    if it.1 == 1 { self.out(&format!("{:<14}{}", "<DIR>", it.0)); nd += 1; }
                    else { self.out(&format!("{:>12}  {}", it.2, it.0)); nf += 1; total += it.2 as u64; }
                }
                if items.is_empty() { self.out("(bos dizin)"); }
                self.out("");
                self.out(&format!("{} dosya ({} bayt), {} dizin", nf, total, nd));
            }
            "type" => {
                if rest.is_empty() { self.out("Kullanim: type <dosya>"); return; }
                let path = self.resolve(rest);
                let mut buf = vec![0u8; 16384];
                let n = syscall::sys_read_file(&path, &mut buf) as usize;
                if n == 0 { self.out("Dosya bulunamadi veya bos."); }
                else {
                    let n2 = n.min(buf.len());
                    match core::str::from_utf8(&buf[..n2]) {
                        Ok(t) => { let owned = String::from(t); for l in owned.lines() { self.out(l); } }
                        Err(_) => self.out("(ikili dosya - metin olarak gosterilemiyor)"),
                    }
                    if n >= 16384 { self.out("... (dosya 16KB'de kirpildi)"); }
                }
            }
            "copy" => {
                let mut it = rest.split_whitespace();
                let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if a.is_empty() || b.is_empty() { self.out("Kullanim: copy <kaynak> <hedef>"); return; }
                let src = self.resolve(a);
                let dst = self.resolve(b);
                let mut buf = vec![0u8; 65536];
                let n = syscall::sys_read_file(&src, &mut buf) as usize;
                if n == 0 { self.out("Kaynak dosya okunamadi."); return; }
                match syscall::sys_write_file(&dst, &buf[..n]) {
                    0 => {
                        self.out("        1 dosya kopyalandi.");
                        if n >= 65536 { self.out("UYARI: 64KB'den buyuk kisim kopyalanmadi."); }
                    }
                    2 => self.out("Erisim engellendi."),
                    _ => self.out("Kopyalama basarisiz."),
                }
            }
            "del" | "erase" | "rmdir" | "rd" => {
                if rest.is_empty() { self.out("Kullanim: del <dosya>"); return; }
                let p = self.resolve(rest);
                match syscall::sys_delete_file(&p) {
                    0 => {}
                    2 => self.out("Erisim engellendi."),
                    _ => self.out("Silinemedi (bulunamadi?)."),
                }
            }
            "ren" | "rename" => {
                let mut it = rest.split_whitespace();
                let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if a.is_empty() || b.is_empty() { self.out("Kullanim: ren <eski> <yeniAd>"); return; }
                let old = self.resolve(a);
                match syscall::sys_rename(&old, b) {
                    0 => {}
                    2 => self.out("Erisim engellendi."),
                    _ => self.out("Yeniden adlandirilamadi (8.3 ad kuralina uyun)."),
                }
            }
            "mkdir" | "md" => {
                if rest.is_empty() { self.out("Kullanim: mkdir <ad>"); return; }
                let p = self.resolve(rest);
                if syscall::sys_create_dir(&p) != 0 { self.out("Klasor olusturulamadi."); }
            }
            "move" => {
                let mut it = rest.split_whitespace();
                let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
                if a.is_empty() || b.is_empty() { self.out("Kullanim: move <kaynak> <hedefDizin>"); return; }
                let src = self.resolve(a);
                let dst = self.resolve(b);
                match syscall::sys_move(&src, &dst) {
                    0 => self.out("        1 oge tasindi."),
                    2 => self.out("Erisim engellendi."),
                    3 => self.out("Farkli suruculer arasi tasima desteklenmiyor."),
                    _ => self.out("Tasima basarisiz."),
                }
            }
            "reg" => {
                let mut dump = vec![0u8; 8192];
                let n = syscall::sys_reg_list(&mut dump) as usize;
                let filt = rest.to_ascii_lowercase();
                if let Ok(t) = core::str::from_utf8(&dump[..n.min(8192)]) {
                    let owned = String::from(t);
                    for l in owned.lines() {
                        if filt.is_empty() || l.to_ascii_lowercase().contains(&filt) {
                            self.out(l);
                        }
                    }
                }
            }
            "tasklist" => {
                self.out("  ID  Pencere");
                self.out("----  -------");
                for (id, t) in app_mgr::tasks().iter() {
                    self.out(&format!("{:>4}  {}", id, t));
                }
            }
            "taskkill" => {
                match rest.split_whitespace().next().and_then(|s| s.parse::<u32>().ok()) {
                    Some(id) => { app_mgr::request_kill(id); self.out("Kapatma istegi gonderildi."); }
                    None => self.out("Kullanim: taskkill <id>  (id icin: tasklist)"),
                }
            }
            "start" => {
                let mut it = rest.split_whitespace();
                let name = it.next().unwrap_or("").to_ascii_lowercase();
                let path = it.next().map(|p| self.resolve(p)).unwrap_or_default();
                let kind = match name.as_str() {
                    "gezgin" | "explorer" => 3,
                    "paint" | "mspaint" => 2,
                    "notepad" | "notdefteri" => 6,
                    "regedit" => 4,
                    "taskmgr" | "gorevmgr" => 5,
                    "hakkinda" | "winver" => 1,
                    "cmd" => 7,
                    _ => { self.out("Bilinmeyen uygulama. (gezgin, paint, notepad, regedit, taskmgr, cmd)"); return; }
                };
                app_mgr::request_app(kind, path);
            }
            "color" => {
                if rest.is_empty() { self.fg = PALETTE[7]; self.bg = PALETTE[0]; return; }
                let hx: Vec<u32> = rest.chars().filter_map(|c| c.to_digit(16)).collect();
                match hx.len() {
                    1 => { self.fg = PALETTE[hx[0] as usize]; self.bg = PALETTE[0]; }
                    2 => {
                        if hx[0] == hx[1] { self.out("On plan ve arka plan ayni olamaz."); return; }
                        self.bg = PALETTE[hx[0] as usize];
                        self.fg = PALETTE[hx[1] as usize];
                    }
                    _ => self.out("Kullanim: color <AF>  (A=arkaplan F=yazi, hex 0-F)"),
                }
            }
            "shutdown" => {
                if rest.contains("/r") { syscall::sys_power(1); } else { syscall::sys_power(0); }
            }
            "exit" => {
                // NOT: birden fazla cmd penceresi aciksa ilk bulunani kapatir
                for (id, t) in app_mgr::tasks().iter() {
                    if t == "Komut Istemi" { app_mgr::request_kill(*id); break; }
                }
            }
            _ => {
                self.out(&format!(
                    "'{}' ic ya da dis komut, calistirilabilir program olarak taninmiyor.", head));
                self.out("Komut listesi icin 'help' yazin.");
            }
        }
    }
}

fn clip(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

impl App for Cmd {
    fn title(&self) -> &'static str { "Komut Istemi" }

    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize) {
        self.vw = w;
        self.vh = h;
        if !self.init { self.startup(); }
        if w < 200 || h < 80 { return; }

        r.fill_rect(x, y, w, h, self.bg);

        let cols = (w - 12) / 7;
        let total_rows = (h - 10) / LINE_H;
        if total_rows < 2 || cols < 10 { return; }
        let view = total_rows - 1;

        let max_scroll = self.lines.len().saturating_sub(view);
        if self.scroll > max_scroll { self.scroll = max_scroll; }

        let end = self.lines.len() - self.scroll;
        let start = end.saturating_sub(view);

        let mut cy = y + 5;
        for l in self.lines[start..end].iter() {
            r.draw_text(clip(l, cols), x + 6, cy, self.fg, 1);
            cy += LINE_H;
        }

        if self.scroll == 0 {
            let p = format!("{}>{}_", self.cwd, self.input);
            let cc = p.chars().count();
            let shown: String = if cc > cols { p.chars().skip(cc - cols).collect() } else { p };
            r.draw_text(&shown, x + 6, cy, self.fg, 1);
        } else {
            r.draw_text("-- gecmis: alt kenara tiklayarak donun --", x + 6, cy, 0x00808080, 1);
        }
    }

    fn on_event(&mut self, ev: &AppEvent) -> bool {
        match ev {
            AppEvent::Key { ch } => {
                match *ch {
                    '\n' => {
                        let line = core::mem::take(&mut self.input);
                        self.scroll = 0;
                        self.exec(line);
                    }
                    '\u{8}' => { self.input.pop(); }
                    '\u{1b}' => { self.input.clear(); }
                    c if c as u32 >= 32 => {
                        if self.input.len() < MAX_INPUT { self.input.push(c); }
                    }
                    _ => {}
                }
                true
            }
            AppEvent::Click { x: _, y } => {
                if *y < 20 { self.scroll += 5; return true; }            
                if *y > self.vh as i32 - 20 { 
                    self.scroll = self.scroll.saturating_sub(5);
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

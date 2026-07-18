pub mod disk;
pub mod fsops;
pub mod recovery;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use spin::Mutex;
use crate::fs::fat32::Fat32FileSystem;
use core::sync::atomic::{AtomicU32, Ordering};

pub static CACHE_DESKTOP: AtomicU32 = AtomicU32::new(0);
pub static CACHE_TASKBAR: AtomicU32 = AtomicU32::new(0);
pub static CACHE_YETKI: AtomicU32 = AtomicU32::new(2);

// driver table C: D:
pub struct Drive {
    pub letter: u8,
    pub label: &'static str,
    pub kind: u8,          // 0 = local disk, 1 = usb
    pub size_mb: u32,
    pub fs: Fat32FileSystem,
}

static mut DRIVES: Vec<Drive> = Vec::new();

pub fn add_drive(letter: u8, label: &'static str, kind: u8, size_mb: u32, fs: Fat32FileSystem) {
    unsafe {
        #[allow(static_mut_refs)]
        DRIVES.push(Drive { letter: letter.to_ascii_uppercase(), label, kind, size_mb, fs });
    }
}

pub fn drives() -> &'static [Drive] {
    unsafe {
        #[allow(static_mut_refs)]
        DRIVES.as_slice()
    }
}

pub fn drive_count() -> usize { drives().len() }

pub fn fs_by_letter(l: u8) -> Option<&'static Fat32FileSystem> {
    let l = l.to_ascii_uppercase();
    drives().iter().find(|d| d.letter == l).map(|d| &d.fs)
}

// System disk = first mount (registry is here)
pub fn system_letter() -> u8 {
    drives().first().map(|d| d.letter).unwrap_or(b'C')
}

pub fn refresh_cache() {
    let r = REGISTRY.lock();
    CACHE_DESKTOP.store(r.get_u32("Sistem/Masaustu/Renk", 0), Ordering::Relaxed);
    CACHE_TASKBAR.store(r.get_u32("Sistem/Taskbar/Renk", 0), Ordering::Relaxed);
    let user = match r.get("Oturum/AktifKullanici") {
        Some(RegData::Str(s)) => s,
        _ => String::from("User"),
    };
    let mut key = String::from("Kullanicilar/");
    key.push_str(&user);
    key.push_str("/Yetki");
    CACHE_YETKI.store(r.get_u32(&key, 2), Ordering::Relaxed);
}

pub fn fs_ref() -> Option<&'static Fat32FileSystem> {
    unsafe {
        #[allow(static_mut_refs)]
        REG_FS.as_ref()
    }
}

#[derive(Clone)]
pub enum RegData {
    U32(u32),
    Str(String),
    Bool(bool),
}

#[derive(Clone)]
pub struct RegEntry {
    pub path: String,
    pub data: RegData,
}

pub struct Registry {
    pub entries: Vec<RegEntry>,
    pub dirty: bool,
}

impl Registry {
    pub const fn new() -> Self {
        Registry { entries: Vec::new(), dirty: false }
    }
    pub fn set(&mut self, path: &str, data: RegData) {
        for e in self.entries.iter_mut() {
            if e.path == path {
                e.data = data;
                self.dirty = true;
                return;
            }
        }
        self.entries.push(RegEntry { path: String::from(path), data });
        self.dirty = true;
    }
    pub fn get(&self, path: &str) -> Option<RegData> {
        for e in self.entries.iter() {
            if e.path == path {
                return Some(e.data.clone());
            }
        }
        None
    }
    pub fn get_u32(&self, path: &str, default: u32) -> u32 {
        match self.get(path) {
            Some(RegData::U32(v)) => v,
            _ => default,
        }
    }
    pub fn list_prefix(&self, prefix: &str) -> Vec<RegEntry> {
        self.entries.iter().filter(|e| e.path.starts_with(prefix)).cloned().collect()
    }
    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for e in self.entries.iter() {
            let line = match &e.data {
                RegData::U32(v) => format!("{}=u32:{}\n", e.path, v),
                RegData::Str(s) => format!("{}=str:{}\n", e.path, s),
                RegData::Bool(b) => format!("{}=bool:{}\n", e.path, if *b {1} else {0}),
            };
            out.push_str(&line);
        }
        out
    }
    pub fn deserialize(&mut self, text: &str) {
        self.entries.clear();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if let Some(eq) = line.find('=') {
                let path = &line[..eq];
                let rest = &line[eq+1..];
                if let Some(colon) = rest.find(':') {
                    let tip = &rest[..colon];
                    let val = &rest[colon+1..];
                    let data = match tip {
                        "u32" => RegData::U32(val.parse::<u32>().unwrap_or(0)),
                        "bool" => RegData::Bool(val == "1"),
                        _ => RegData::Str(String::from(val)),
                    };
                    self.entries.push(RegEntry { path: String::from(path), data });
                }
            }
        }
        self.dirty = false;
    }
    pub fn load_defaults(&mut self) {
        self.entries.clear();
        self.set("Sistem/Ad", RegData::Str(String::from("Rusty OS")));
        self.set("Sistem/Surum", RegData::Str(String::from("0.1")));
        self.set("Sistem/Dil", RegData::Str(String::from("tr")));
        self.set("Sistem/Masaustu/Renk", RegData::U32(0x00501808));
        self.set("Sistem/Taskbar/Renk", RegData::U32(0x00201008));
        self.set("Sistem/Saat/UTC", RegData::U32(3));
        self.set("Sistem/Saat/24Saat", RegData::Bool(true));
        self.set("Sistem/Ses/Seviye", RegData::U32(100));
        self.set("Oturum/AktifKullanici", RegData::Str(String::from("User")));
        self.set("Oturum/IlkKurulumBitti", RegData::Bool(false));
        self.set("Kullanicilar/User/Sifre", RegData::Str(String::new()));
        self.set("Kullanicilar/User/Yetki", RegData::U32(1));
        self.set("Kullanicilar/User/AnaKlasor", RegData::Str(String::from("Users/User")));
        self.set("Kullanicilar/User/Tema", RegData::Str(String::from("aero")));
        self.set("Kullanicilar/User/Avatar", RegData::Str(String::new()));
        self.dirty = true;
    }
}

pub static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

pub fn get_u32(path: &str, default: u32) -> u32 {
    REGISTRY.lock().get_u32(path, default)
}

pub fn set_u32(path: &str, val: u32) {
    REGISTRY.lock().set(path, RegData::U32(val));
    refresh_cache();
    save_to_disk();
}

static mut REG_FS: Option<Fat32FileSystem> = None;

pub fn set_fs(fs: Fat32FileSystem) {
    unsafe { REG_FS = Some(fs); }
}

pub fn save_to_disk() {
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(fs) = REG_FS.as_ref() {
            let _ = disk::save(fs);
        }
    }
}

pub fn init_from_disk() {
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(fs) = REG_FS.as_ref() {
            if disk::load(fs).is_err() {
                REGISTRY.lock().load_defaults();
                let _ = disk::save(fs);
            }
        } else {
            REGISTRY.lock().load_defaults();
        }
    }
    refresh_cache();
}

// Syscall 14
pub fn list_call(buf: &mut [u8]) -> u64 {
    let text = REGISTRY.lock().serialize();
    let data = text.as_bytes();
    let n = data.len().min(buf.len());
    buf[..n].copy_from_slice(&data[..n]);
    n as u64
}

// Syscall 15
pub fn set_call(buf: &mut [u8]) -> u64 {
    if buf.len() < 2 { return 1; }
    let l = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if 2 + l > buf.len() { return 1; }
    let Ok(line) = core::str::from_utf8(&buf[2..2+l]) else { return 1 };
    let line = line.trim();
    let Some(eq) = line.find('=') else { return 1 };
    let path = String::from(&line[..eq]);
    let rest = &line[eq+1..];
    let Some(colon) = rest.find(':') else { return 1 };
    let tip = &rest[..colon];
    let val = &rest[colon+1..];
    let data = match tip {
        "u32" => RegData::U32(val.parse::<u32>().unwrap_or(0)),
        "bool" => RegData::Bool(val == "1"),
        _ => RegData::Str(String::from(val)),
    };
    REGISTRY.lock().set(&path, data);
    refresh_cache();
    save_to_disk();
    0
}
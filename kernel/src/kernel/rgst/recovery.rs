// Rusty RECOVERY
use alloc::vec::Vec;
use alloc::vec;
use crate::fs::BlockDevice;
use crate::fs::fat32::{Fat32, DirEntry};

static mut EMBEDDED_CORE: &[u8] = &[];
pub fn set_embedded_core(data: &'static [u8]) { unsafe { EMBEDDED_CORE = data; } }
fn embedded_core() -> &'static [u8] { unsafe { EMBEDDED_CORE } }

#[derive(Clone, Copy, PartialEq)]
pub enum Missing {
    Registry,
    CoreBin,
    DirUsers,      // only controls if oobe end
    DirDesktop,
    DirDocuments,
    DirDownloads,
}

fn status(msg: &str) {
    let r = unsafe { crate::renderer() };
    let w = r.width;
    let h = r.height;
    r.fill_rect(0, h - 48, w, 20, 0x00000000);
    r.set_color(0x00FF8020);
    let px = w / 2usize;
    let len = msg.len() * 7; // 7px/char
    r.text_at(px.saturating_sub(len / 2), h - 40, msg);
}

fn delay() {
    for _ in 0..60_000_000 { unsafe { core::arch::asm!("nop"); } }
}

pub fn check() -> Vec<Missing> {
    let mut miss = Vec::new();
    let Some(fs) = super::fs_ref() else { return miss; };

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();

    let root = match fat.list_root(&mut *dev) { Ok(r) => r, Err(_) => return miss };

    let dir_of = |name: &str, list: &Vec<DirEntry>| -> Option<u32> {
        list.iter().find(|e| e.is_dir && e.name.eq_ignore_ascii_case(name))
            .map(|e| e.first_cluster)
    };
    let has_file = |name: &str, list: &Vec<DirEntry>| -> bool {
        list.iter().any(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name))
    };

    // --- IMPORTANT: RSYS/CORE.BIN + REGISTRY.DAT (always) ---
    match dir_of("RSYS", &root) {
        None => {
            miss.push(Missing::Registry);
            miss.push(Missing::CoreBin);
        }
        Some(c) => {
            let rsys = fat.list_dir(&mut *dev, c).unwrap_or_default();
            if !has_file("REGISTRY.DAT", &rsys) { miss.push(Missing::Registry); }
            if !has_file("CORE.BIN", &rsys) { miss.push(Missing::CoreBin); }
        }
    }
     miss
}

// fat helpers
fn next_cluster(fat: &Fat32, dev: &mut dyn BlockDevice, c: u32) -> Result<u32, ()> {
    let off = c * 4;
    let sector = fat.fat_start_lba + (off / fat.bytes_per_sector) as u64;
    let o = (off % fat.bytes_per_sector) as usize;
    let mut buf = vec![0u8; fat.bytes_per_sector as usize];
    dev.read_block(sector, &mut buf).map_err(|_| ())?;
    Ok(u32::from_le_bytes([buf[o], buf[o+1], buf[o+2], buf[o+3]]) & 0x0FFF_FFFF)
}
fn set_fat(fat: &Fat32, dev: &mut dyn BlockDevice, c: u32, val: u32) -> Result<(), ()> {
    let off = c * 4;
    let sector = fat.fat_start_lba + (off / fat.bytes_per_sector) as u64;
    let o = (off % fat.bytes_per_sector) as usize;
    let mut buf = vec![0u8; fat.bytes_per_sector as usize];
    dev.read_block(sector, &mut buf).map_err(|_| ())?;
    buf[o..o+4].copy_from_slice(&val.to_le_bytes());
    dev.write_block(sector, &buf).map_err(|_| ())?;
    if fat.num_fats > 1 {
        let backup = sector + fat.fat_size_sectors as u64;
        dev.write_block(backup, &buf).map_err(|_| ())?;
    }
    Ok(())
}
fn lba(fat: &Fat32, c: u32) -> u64 {
    fat.data_start_lba + ((c as u64 - 2) * fat.sectors_per_cluster as u64)
}
fn alloc_cluster(fat: &Fat32, dev: &mut dyn BlockDevice) -> Result<u32, ()> {
    let total = (fat.fat_size_sectors * fat.bytes_per_sector) / 4;
    for c in 3u32..total {
        if next_cluster(fat, dev, c)? == 0 {
            set_fat(fat, dev, c, 0x0FFF_FFFF)?;
            let zero = vec![0u8; fat.bytes_per_sector as usize];
            let base = lba(fat, c);
            for s in 0..fat.sectors_per_cluster {
                dev.write_block(base + s as u64, &zero).map_err(|_| ())?;
            }
            return Ok(c);
        }
    }
    Err(())
}
fn sn83(name: &str) -> Option<[u8; 11]> {
    let mut out = [b' '; 11];
    let up: Vec<u8> = name.bytes().map(|c| c.to_ascii_uppercase()).collect();
    let (base, ext) = match name.rfind('.') {
        Some(d) => (&up[..d], &up[d+1..]),
        None => (&up[..], &b""[..]),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 { return None; }
    out[..base.len()].copy_from_slice(base);
    out[8..8+ext.len()].copy_from_slice(ext);
    Some(out)
}
fn insert_entry(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32,
                sn: [u8; 11], attr: u8, first: u32, size: u32) -> Result<(), ()> {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent; let mut guard = 0;
    loop {
        let base = lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            dev.read_block(base + s as u64, &mut sec).map_err(|_| ())?;
            for off in (0..bps).step_by(32) {
                let fb = sec[off];
                if fb == 0x00 || fb == 0xE5 {
                    for b in sec[off..off+32].iter_mut() { *b = 0; }
                    sec[off..off+11].copy_from_slice(&sn);
                    sec[off+11] = attr;
                    sec[off+20..off+22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
                    sec[off+26..off+28].copy_from_slice(&(first as u16).to_le_bytes());
                    sec[off+28..off+32].copy_from_slice(&size.to_le_bytes());
                    dev.write_block(base + s as u64, &sec).map_err(|_| ())?;
                    return Ok(());
                }
            }
        }
        c = next_cluster(fat, dev, c)?;
        if c < 2 || c >= 0x0FFF_FFF8 { return Err(()); }
        guard += 1; if guard > 128 { return Err(()); }
    }
}
fn ensure_dirs(fat: &Fat32, dev: &mut dyn BlockDevice, path: &str) -> Result<u32, ()> {
    let mut cluster = fat.root_cluster;
    for part in path.split('/') {
        if part.is_empty() { continue; }
        let entries = fat.list_dir(dev, cluster).map_err(|_| ())?;
        match entries.iter().find(|e| e.is_dir && e.name.eq_ignore_ascii_case(part)) {
            Some(e) => cluster = e.first_cluster,
            None => {
                let nc = alloc_cluster(fat, dev)?;
                let sn = sn83(part).ok_or(())?;
                insert_entry(fat, dev, cluster, sn, 0x10, nc, 0)?;
                cluster = nc;
            }
        }
    }
    Ok(cluster)
}
fn write_file(fat: &Fat32, dev: &mut dyn BlockDevice,
              dir_cluster: u32, name: &str, data: &[u8]) -> Result<(), ()> {
    let sn = sn83(name).ok_or(())?;
    let bps = fat.bytes_per_sector as usize;
    let spc = fat.sectors_per_cluster as usize;
    let cbytes = bps * spc;

    let mut first = 0u32;
    let mut prev = 0u32;
    let need = if data.is_empty() { 0 } else { (data.len() + cbytes - 1) / cbytes };
    for i in 0..need {
        let c = alloc_cluster(fat, dev)?;
        if first == 0 { first = c; } else { set_fat(fat, dev, prev, c)?; }
        let base = lba(fat, c);
        for s in 0..spc {
            let off = i * cbytes + s * bps;
            let mut sec = vec![0u8; bps];
            if off < data.len() {
                let end = (off + bps).min(data.len());
                sec[..end - off].copy_from_slice(&data[off..end]);
            }
            dev.write_block(base + s as u64, &sec).map_err(|_| ())?;
        }
        prev = c;
    }
    if prev != 0 { set_fat(fat, dev, prev, 0x0FFF_FFFF)?; }

    let entries = fat.list_dir(dev, dir_cluster).map_err(|_| ())?;
    if entries.iter().any(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name)) {
        patch_delete(fat, dev, dir_cluster, sn)?;
    }
    insert_entry(fat, dev, dir_cluster, sn, 0x20, first, data.len() as u32)
}
fn patch_delete(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, sn: [u8; 11]) -> Result<(), ()> {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent; let mut guard = 0;
    loop {
        let base = lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            dev.read_block(base + s as u64, &mut sec).map_err(|_| ())?;
            for off in (0..bps).step_by(32) {
                if sec[off] == 0x00 { return Ok(()); }
                if sec[off] != 0xE5 && sec[off+11] != 0x0F && sec[off..off+11] == sn {
                    sec[off] = 0xE5;
                    dev.write_block(base + s as u64, &sec).map_err(|_| ())?;
                    return Ok(());
                }
            }
        }
        c = next_cluster(fat, dev, c)?;
        if c < 2 || c >= 0x0FFF_FFF8 { return Ok(()); }
        guard += 1; if guard > 128 { return Ok(()); }
    }
}

pub fn run(missing: &[Missing]) -> ! {
    status("Rusty is going into rescue mode...");
    delay();
    status("Disk scanning...");
    delay();

    let reg_text = {
        let mut t = super::REGISTRY.lock().serialize();
        if t.len() < 4096 { while t.len() < 4096 { t.push('\n'); } }
        t
    };

    let ok = {
        let Some(fs) = super::fs_ref() else {
            status("ERROR: No system disk!");
            delay(); delay();
            crate::drivers::power::reboot();
        };
        let fat = fs.fat.lock();
        let mut dev = fs.dev.lock();
        let mut all_ok = true;

        status("Klasorler onariliyor...");
        if ensure_dirs(&fat, &mut *dev, "RSYS").is_err() { all_ok = false; }
        if missing.iter().any(|m| matches!(m,
            Missing::DirUsers | Missing::DirDesktop | Missing::DirDocuments | Missing::DirDownloads))
        {
            let _ = ensure_dirs(&fat, &mut *dev, "Users/Shared");
            if ensure_dirs(&fat, &mut *dev, "Users/User/Desktop").is_err()   { all_ok = false; }
            if ensure_dirs(&fat, &mut *dev, "Users/User/Documents").is_err() { all_ok = false; }
            if ensure_dirs(&fat, &mut *dev, "Users/User/Downloads").is_err() { all_ok = false; }
        }
        delay();

        // REGISTRY.DAT
        if missing.contains(&Missing::Registry) {
            status("REGISTRY.DAT creating...");
            match ensure_dirs(&fat, &mut *dev, "RSYS") {
                Ok(rsys) => {
                    if write_file(&fat, &mut *dev, rsys, "REGISTRY.DAT", reg_text.as_bytes()).is_err() {
                        all_ok = false;
                    }
                }
                Err(_) => all_ok = false,
            }
            delay();
        }

        // CORE.BIN
        if missing.contains(&Missing::CoreBin) {
            status("CORE.BIN loading...");
            let core = embedded_core();
            if core.is_empty() { all_ok = false; }
            else {
                match ensure_dirs(&fat, &mut *dev, "RSYS") {
                    Ok(rsys) => {
                        if write_file(&fat, &mut *dev, rsys, "CORE.BIN", core).is_err() {
                            all_ok = false;
                        }
                    }
                    Err(_) => all_ok = false,
                }
            }
            delay();
        }

        all_ok
    }; // kilitler burada birakildi

    if ok { status("Repair complete. Restarting..."); }
    else  { status("Repair PARTIALLY failed. Restarting..."); }
    delay(); delay();

    crate::drivers::power::reboot();
}
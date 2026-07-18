use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::fs::BlockDevice;
use crate::fs::fat32::{Fat32, Fat32FileSystem};
use core::sync::atomic::AtomicU32;
static FREE_HINT: AtomicU32 = AtomicU32::new(3);

fn split_drive(path: &str) -> Option<(u8, &str)> {
    let b = path.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        if b.len() == 2 { return Some((b[0].to_ascii_uppercase(), "")); }
        if b[2] == b'/' { return Some((b[0].to_ascii_uppercase(), &path[3..])); }
    }
    None
}

fn resolve<'a>(path: &'a str) -> Option<(&'static Fat32FileSystem, &'a str)> {
    match split_drive(path) {
        Some((l, rest)) => super::fs_by_letter(l).map(|f| (f, rest)),
        None => super::fs_ref().map(|f| (f, path)),
    }
}

fn letter_of(path: &str) -> u8 {
    match split_drive(path) {
        Some((l, _)) => l,
        None => super::system_letter(),
    }
}

// Fat Helpers
fn cluster_to_lba(fat: &Fat32, c: u32) -> u64 {
    fat.data_start_lba + ((c as u64 - 2) * fat.sectors_per_cluster as u64)
}
fn next_cluster(fat: &Fat32, dev: &mut dyn BlockDevice, c: u32) -> Result<u32, &'static str> {
    let off = c * 4;
    let sector = fat.fat_start_lba + (off / fat.bytes_per_sector) as u64;
    let o = (off % fat.bytes_per_sector) as usize;
    let mut buf = vec![0u8; fat.bytes_per_sector as usize];
    dev.read_block(sector, &mut buf)?;
    Ok(u32::from_le_bytes([buf[o], buf[o+1], buf[o+2], buf[o+3]]) & 0x0FFF_FFFF)
}
fn set_fat_entry(fat: &Fat32, dev: &mut dyn BlockDevice, c: u32, val: u32) -> Result<(), &'static str> {
    let off = c * 4;
    let sector = fat.fat_start_lba + (off / fat.bytes_per_sector) as u64;
    let o = (off % fat.bytes_per_sector) as usize;
    let mut buf = vec![0u8; fat.bytes_per_sector as usize];
    dev.read_block(sector, &mut buf)?;
    buf[o..o+4].copy_from_slice(&val.to_le_bytes());
    dev.write_block(sector, &buf)?;
    if fat.num_fats > 1 {
        let backup = sector + fat.fat_size_sectors as u64;
        dev.write_block(backup, &buf)?;
    }
    Ok(())
}
fn alloc_cluster(fat: &Fat32, dev: &mut dyn BlockDevice) -> Result<u32, &'static str> {
    let bps = fat.bytes_per_sector as usize;
    if bps < 512 { return Err("bps is small"); }
    let eps = (bps / 4) as u32; // entry/sector (128)
    let total = (fat.fat_size_sectors.saturating_mul(eps)).min(0x0FFF_FFF0).max(4);
    let hint = FREE_HINT.load(core::sync::atomic::Ordering::Relaxed).clamp(3, total - 1);

    // two passages: hint..total, then 3..hint. Read the FAT section once, scan it thoroughly.
    for pass in 0..2 {
        let (lo, hi) = if pass == 0 { (hint, total) } else { (3u32, hint) };
        let mut c = lo;
        while c < hi {
            let sector = fat.fat_start_lba + (c as u64 * 4 / bps as u64);
            let mut buf = vec![0u8; bps];
            if dev.read_block(sector, &mut buf).is_err() { return Err("FAT okunamadi"); }
            let mut e = c;
            while e < hi {
                let esec = fat.fat_start_lba + (e as u64 * 4 / bps as u64);
                if esec != sector { break; }
                let o = (e as usize * 4) % bps;
                let val = u32::from_le_bytes([buf[o], buf[o+1], buf[o+2], buf[o+3]]) & 0x0FFF_FFFF;
                if val == 0 {
                    set_fat_entry(fat, dev, e, 0x0FFF_FFFF)?;
                    let lba = cluster_to_lba(fat, e);
                    let zero = vec![0u8; bps];
                    for s in 0..fat.sectors_per_cluster {
                        dev.write_block(lba + s as u64, &zero)?;
                    }
                    FREE_HINT.store(e + 1, core::sync::atomic::Ordering::Relaxed);
                    return Ok(e);
                }
                e += 1;
            }
            c = e;
        }
    }
    Err("no empty cluster")
}
fn free_chain(fat: &Fat32, dev: &mut dyn BlockDevice, start: u32) {
    let mut c = start;
    let mut guard = 0;
    while c >= 2 && c < 0x0FFF_FFF8 {
        let n = match next_cluster(fat, dev, c) { Ok(v) => v, Err(_) => return };
        if set_fat_entry(fat, dev, c, 0).is_err() { return; }
        c = n;
        guard += 1;
        if guard > 65536 { return; }
    }
}
fn walk_dir(fat: &Fat32, dev: &mut dyn BlockDevice, path: &str) -> Result<u32, &'static str> {
    let mut cluster = fat.root_cluster;
    for part in path.split('/') {
        if part.is_empty() { continue; }
        let entries = fat.list_dir(dev, cluster)?;
        let hit = entries.iter()
            .find(|e| e.is_dir && e.name.eq_ignore_ascii_case(part))
            .ok_or("klasor yok")?;
        cluster = hit.first_cluster;
    }
    Ok(cluster)
}
fn short_name_83(name: &str) -> Option<[u8; 11]> {
    if name.is_empty() || name.contains(':') || name.contains('/') || name.contains('\\') {
        return None;
    }
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
fn split_parent(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..i], &path[i+1..]),
        None => ("", path),
    }
}
fn read_path(buf: &[u8], start: usize) -> Option<(String, usize)> {
    if start + 2 > buf.len() { return None; }
    let l = u16::from_le_bytes([buf[start], buf[start+1]]) as usize;
    if start + 2 + l > buf.len() { return None; }
    match core::str::from_utf8(&buf[start+2..start+2+l]) {
        Ok(s) => Some((String::from(s), start + 2 + l)),
        Err(_) => None,
    }
}
fn insert_entry(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, sn: [u8; 11], attr: u8, first_cluster: u32) -> u64 {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent;
    let mut guard = 0;
    loop {
        let lba = cluster_to_lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { return 1; }
            for off in (0..bps).step_by(32) {
                let fb = sec[off];
                if fb == 0x00 || fb == 0xE5 {
                    for b in sec[off..off+32].iter_mut() { *b = 0; }
                    sec[off..off+11].copy_from_slice(&sn);
                    sec[off+11] = attr;
                    sec[off+20..off+22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
                    sec[off+26..off+28].copy_from_slice(&(first_cluster as u16).to_le_bytes());
                    if dev.write_block(lba + s as u64, &sec).is_err() { return 1; }
                    return 0;
                }
            }
        }
        c = match next_cluster(fat, dev, c) { Ok(n) => n, Err(_) => return 1 };
        if c < 2 || c >= 0x0FFF_FFF8 { return 1; }
        guard += 1; if guard > 64 { return 1; }
    }
}

fn patch_meta(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, sn: [u8; 11], size: u32, first: u32) -> u64 {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent;
    let mut guard = 0;
    loop {
        let lba = cluster_to_lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { return 1; }
            for off in (0..bps).step_by(32) {
                if sec[off] == 0x00 { return 1; }
                if sec[off] != 0xE5 && sec[off+11] != 0x0F && sec[off..off+11] == sn {
                    sec[off+28..off+32].copy_from_slice(&size.to_le_bytes());
                    sec[off+20..off+22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
                    sec[off+26..off+28].copy_from_slice(&(first as u16).to_le_bytes());
                    if dev.write_block(lba + s as u64, &sec).is_err() { return 1; }
                    return 0;
                }
            }
        }
        c = match next_cluster(fat, dev, c) { Ok(n) => n, Err(_) => return 1 };
        if c < 2 || c >= 0x0FFF_FFF8 { return 1; }
        guard += 1; if guard > 64 { return 1; }
    }
}

// list dir
pub fn list_dir_call(buf: &mut [u8]) -> u64 {
    let Some((path, _)) = read_path(buf, 0) else { return 0 };

    if path.is_empty() {
        let ds = super::drives();
        let mut count = 0usize;
        for d in ds.iter() {
            let off = count * 40;
            if off + 40 > buf.len() || count >= 26 { break; }
            for b in buf[off..off+40].iter_mut() { *b = 0; }
            buf[off] = d.letter;
            buf[off + 1] = b':';
            buf[off + 32] = 1;
            buf[off + 33] = 2;
            buf[off + 34] = d.kind;
            buf[off+36..off+40].copy_from_slice(&d.size_mb.to_le_bytes());
            count += 1;
        }
        return count as u64;
    }

    let Some((fs, rest)) = resolve(&path) else { return 0 };
    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster = match walk_dir(&fat, &mut *dev, rest) { Ok(c) => c, Err(_) => return 0 };
    let entries = match fat.list_dir(&mut *dev, cluster) { Ok(e) => e, Err(_) => return 0 };
    let mut count = 0usize;
    for e in entries.iter() {
        if e.name == "." || e.name == ".." { continue; }
        let off = count * 40;
        if off + 40 > buf.len() || count >= 64 { break; }
        for b in buf[off..off+40].iter_mut() { *b = 0; }
        let nb = e.name.as_bytes();
        let n = nb.len().min(31);
        buf[off..off+n].copy_from_slice(&nb[..n]);
        buf[off+32] = if e.is_dir { 1 } else { 0 };
        buf[off+33] = if e.is_dir { 1 } else { 0 };
        buf[off+34] = 0;
        buf[off+36..off+40].copy_from_slice(&e.size.to_le_bytes());
        count += 1;
    }
    count as u64
}

// create delete rename move
pub fn create_file_call(buf: &mut [u8]) -> u64 {
    let Some((full, _)) = read_path(buf, 0) else { return 1 };
    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };
    let Some(sn) = short_name_83(name) else { return 1 };
    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };
    insert_entry(&fat, &mut *dev, cluster, sn, 0x20, 0)
}

pub fn create_dir_call(buf: &mut [u8]) -> u64 {
    let Some((full, _)) = read_path(buf, 0) else { return 1 };
    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };
    let Some(sn) = short_name_83(name) else { return 1 };
    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let pcluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };
    let dcluster = match alloc_cluster(&fat, &mut *dev) { Ok(c) => c, Err(_) => return 1 };
    insert_entry(&fat, &mut *dev, pcluster, sn, 0x10, dcluster)
}

pub fn delete_file_call(buf: &mut [u8]) -> u64 {
    let Some((full, _)) = read_path(buf, 0) else { return 1 };
    if split_drive(&full).map(|(_, r)| r.is_empty()).unwrap_or(false) { return 2; }
    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };
    let Some(sn) = short_name_83(name) else { return 1 };

    use core::sync::atomic::Ordering;
    let yetki = super::CACHE_YETKI.load(Ordering::Relaxed);
    if yetki >= 3 { return 2; }
    if is_critical(&full) && yetki != 1 { return 2; }

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };

    // veri cluster'larini serbest birak
    if let Ok(es) = fat.list_dir(&mut *dev, cluster) {
        if let Some(e) = es.iter().find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name)) {
            if e.first_cluster >= 2 { free_chain(&fat, &mut *dev, e.first_cluster); }
        }
    }
    patch_entry(&fat, &mut *dev, cluster, sn, None)
}

pub fn rename_call(buf: &mut [u8]) -> u64 {
    let Some((full, next)) = read_path(buf, 0) else { return 1 };
    let Some((newname, _)) = read_path(buf, next) else { return 1 };
    if split_drive(&full).map(|(_, r)| r.is_empty()).unwrap_or(false) { return 2; }
    let (parent, oldname) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };
    let Some(old_sn) = short_name_83(oldname) else { return 1 };
    let Some(new_sn) = short_name_83(&newname) else { return 1 };

    use core::sync::atomic::Ordering;
    let yetki = super::CACHE_YETKI.load(Ordering::Relaxed);
    if is_critical(&full) && yetki != 1 { return 2; }

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };
    patch_entry(&fat, &mut *dev, cluster, old_sn, Some(new_sn))
}

fn patch_entry(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, sn: [u8; 11], new: Option<[u8; 11]>) -> u64 {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent;
    let mut guard = 0;
    loop {
        let lba = cluster_to_lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { return 1; }
            for off in (0..bps).step_by(32) {
                if sec[off] == 0x00 { return 1; }
                if sec[off] != 0xE5 && sec[off+11] != 0x0F && sec[off..off+11] == sn {
                    match new {
                        None => sec[off] = 0xE5,
                        Some(ns) => sec[off..off+11].copy_from_slice(&ns),
                    }
                    if dev.write_block(lba + s as u64, &sec).is_err() { return 1; }
                    return 0;
                }
            }
        }
        c = match next_cluster(fat, dev, c) { Ok(n) => n, Err(_) => return 1 };
        if c < 2 || c >= 0x0FFF_FFF8 { return 1; }
        guard += 1; if guard > 64 { return 1; }
    }
}

// write and read file
// Syscall 16: buf = [path]
pub fn read_file_call(buf: &mut [u8]) -> u64 {
    let Some((full, _)) = read_path(buf, 0) else { return 0 };
    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 0 };

    let data = {
        let fat = fs.fat.lock();
        let mut dev = fs.dev.lock();
        let cluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 0 };
        let entries = match fat.list_dir(&mut *dev, cluster) { Ok(e) => e, Err(_) => return 0 };
        let Some(e) = entries.into_iter().find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name))
            else { return 0 };
        if e.first_cluster < 2 || e.size == 0 { return 0; }
        match fat.read_file(&mut *dev, &e) { Ok(d) => d, Err(_) => return 0 }
    };

    let n = data.len().min(buf.len());
    buf[..n].copy_from_slice(&data[..n]);
    n as u64
}

// Syscall 17: buf = [u16 plen][path][u32 dlen][data]
// 0 = ok, 1 = error, 2 = permission
pub fn write_file_call(buf: &mut [u8]) -> u64 {
    let Some((full, next)) = read_path(buf, 0) else { return 1 };
    if next + 4 > buf.len() { return 1; }
    let dlen = u32::from_le_bytes([buf[next], buf[next+1], buf[next+2], buf[next+3]]) as usize;
    if next + 4 + dlen > buf.len() { return 1; }
    if dlen > 1_000_000 { return 1; }

    use core::sync::atomic::Ordering;
    let yetki = super::CACHE_YETKI.load(Ordering::Relaxed);
    if yetki >= 3 { return 2; }
    if is_critical(&full) { return 2; }

    let data: Vec<u8> = buf[next+4..next+4+dlen].to_vec();

    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };
    let Some(sn) = short_name_83(name) else { return 1 };

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let pcluster = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };

    let old_first = {
        let entries = match fat.list_dir(&mut *dev, pcluster) { Ok(e) => e, Err(_) => return 1 };
        match entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)) {
            Some(e) if e.is_dir => return 1,
            Some(e) => e.first_cluster,
            None => {
                if insert_entry(&fat, &mut *dev, pcluster, sn, 0x20, 0) != 0 { return 1; }
                0
            }
        }
    };

    if old_first >= 2 { free_chain(&fat, &mut *dev, old_first); }

    if data.is_empty() {
        return patch_meta(&fat, &mut *dev, pcluster, sn, 0, 0);
    }

    let bps = fat.bytes_per_sector as usize;
    let spc = fat.sectors_per_cluster as usize;
    let cbytes = bps * spc;
    let need = (data.len() + cbytes - 1) / cbytes;

    let mut first: u32 = 0;
    let mut prev: u32 = 0;
    for i in 0..need {
        let c = match alloc_cluster(&fat, &mut *dev) { Ok(c) => c, Err(_) => return 1 };
        if first == 0 { first = c; }
        else if set_fat_entry(&fat, &mut *dev, prev, c).is_err() { return 1; }

        let lba = cluster_to_lba(&fat, c);
        for s in 0..spc {
            let off = i * cbytes + s * bps;
            let mut sec = vec![0u8; bps];
            if off < data.len() {
                let end = (off + bps).min(data.len());
                sec[..end - off].copy_from_slice(&data[off..end]);
            }
            if dev.write_block(lba + s as u64, &sec).is_err() { return 1; }
        }
        prev = c;
    }
    if set_fat_entry(&fat, &mut *dev, prev, 0x0FFF_FFFF).is_err() { return 1; }

    patch_meta(&fat, &mut *dev, pcluster, sn, data.len() as u32, first)
}

// for boot
pub fn read_system_file(name: &str) -> Option<Vec<u8>> {
    let fs = super::fs_ref()?;
    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster = walk_dir(&fat, &mut *dev, "RSYS").ok()?;
    let entries = fat.list_dir(&mut *dev, cluster).ok()?;
    let e = entries.into_iter().find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name))?;
    fat.read_file(&mut *dev, &e).ok()
}

// Audio play
pub fn play_file_call(buf: &mut [u8]) -> u64 {
    let Some((full, _)) = read_path(buf, 0) else { return 1 };
    let (parent, name) = split_parent(&full);
    let Some((fs, prest)) = resolve(parent) else { return 1 };

    let written = {
        let fat = fs.fat.lock();
        let mut dev = fs.dev.lock();

        let dir = match walk_dir(&fat, &mut *dev, prest) { Ok(c) => c, Err(_) => return 1 };
        let entries = match fat.list_dir(&mut *dev, dir) { Ok(e) => e, Err(_) => return 1 };
        let Some(e) = entries.into_iter().find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(name))
            else { return 1 };
        if e.first_cluster < 2 || e.size == 0 { return 1; }

        let pcm = crate::drivers::audio::pcm_buf();
        let cap = pcm.len();
        let size = e.size as usize;
        let bps = fat.bytes_per_sector as usize;
        let spc = fat.sectors_per_cluster as usize;

        let mut sec = vec![0u8; bps];
        let mut c = e.first_cluster;
        let mut off = 0usize;
        let mut guard = 0u32;

        while off < size && off < cap && c >= 2 && c < 0x0FFF_FFF8 {
            let lba = cluster_to_lba(&fat, c);
            for s in 0..spc {
                if off >= size || off >= cap { break; }
                if dev.read_block(lba + s as u64, &mut sec).is_err() { return 1; }
                let n = bps.min(size - off).min(cap - off);
                pcm[off..off + n].copy_from_slice(&sec[..n]);
                off += n;
            }
            c = match next_cluster(&fat, &mut *dev, c) { Ok(v) => v, Err(_) => break };
            guard += 1;
            if guard > 200_000 { break; }
        }
        off
    };

    if written == 0 { return 1; }
    crate::drivers::audio::play_pcm(written);
    0
}

fn take_entry_raw(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, sn: [u8; 11]) -> Option<[u8; 32]> {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent; let mut guard = 0;
    loop {
        let lba = cluster_to_lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { return None; }
            for off in (0..bps).step_by(32) {
                if sec[off] == 0x00 { return None; }
                if sec[off] != 0xE5 && sec[off+11] != 0x0F && sec[off..off+11] == sn {
                    let mut raw = [0u8; 32];
                    raw.copy_from_slice(&sec[off..off+32]);
                    sec[off] = 0xE5;
                    if dev.write_block(lba + s as u64, &sec).is_err() { return None; }
                    return Some(raw);
                }
            }
        }
        c = match next_cluster(fat, dev, c) { Ok(n) => n, Err(_) => return None };
        if c < 2 || c >= 0x0FFF_FFF8 { return None; }
        guard += 1; if guard > 64 { return None; }
    }
}

fn insert_raw(fat: &Fat32, dev: &mut dyn BlockDevice, parent: u32, raw: [u8; 32]) -> u64 {
    let bps = fat.bytes_per_sector as usize;
    let mut c = parent; let mut guard = 0;
    loop {
        let lba = cluster_to_lba(fat, c);
        for s in 0..fat.sectors_per_cluster {
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { return 1; }
            for off in (0..bps).step_by(32) {
                let fb = sec[off];
                if fb == 0x00 || fb == 0xE5 {
                    sec[off..off+32].copy_from_slice(&raw);
                    if dev.write_block(lba + s as u64, &sec).is_err() { return 1; }
                    return 0;
                }
            }
        }
        c = match next_cluster(fat, dev, c) { Ok(n) => n, Err(_) => return 1 };
        if c < 2 || c >= 0x0FFF_FFF8 { return 1; }
        guard += 1; if guard > 64 { return 1; }
    }
}

pub fn move_call(buf: &mut [u8]) -> u64 {
    let Some((src, next)) = read_path(buf, 0) else { return 1 };
    let Some((dstdir, _)) = read_path(buf, next) else { return 1 };

    if letter_of(&src) != letter_of(&dstdir) { return 3; }
    if is_critical(&src) { return 2; }
    if dstdir.eq_ignore_ascii_case(&src) { return 1; }
    let mut pref = src.clone(); pref.push('/');
    if dstdir.len() >= pref.len() && dstdir[..pref.len()].eq_ignore_ascii_case(&pref) { return 1; }

    let (sparent, name) = split_parent(&src);
    if dstdir.eq_ignore_ascii_case(sparent) { return 0; }

    let Some((fs, sprest)) = resolve(sparent) else { return 1 };
    let Some((_, dprest)) = resolve(&dstdir) else { return 1 };
    let Some(sn) = short_name_83(name) else { return 1 };

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let sp = match walk_dir(&fat, &mut *dev, sprest) { Ok(c) => c, Err(_) => return 1 };
    let dp = match walk_dir(&fat, &mut *dev, dprest) { Ok(c) => c, Err(_) => return 1 };
    if let Ok(es) = fat.list_dir(&mut *dev, dp) {
        if es.iter().any(|e| e.name.eq_ignore_ascii_case(name)) { return 1; }
    }
    let Some(raw) = take_entry_raw(&fat, &mut *dev, sp, sn) else { return 1 };
    insert_raw(&fat, &mut *dev, dp, raw)
}

fn is_critical(path: &str) -> bool {
    let up = path.to_ascii_uppercase();
    let p: &str = match split_drive(&up) {
        Some((_, rest)) => rest,
        None => &up,
    };
    if p.is_empty() { return true; }
    p == "RSYS" || p.starts_with("RSYS/") || p == "USERS" || p == "APPS"
        || p.ends_with("CORE.BIN") || p.ends_with("KERNEL.ELF") || p.ends_with("REGISTRY.DAT")
}
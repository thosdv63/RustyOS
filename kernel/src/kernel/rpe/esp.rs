use alloc::vec::Vec;
use crate::fs::BlockDevice;

const ZERO: [u8; 512] = [0u8; 512];
const EOC: u32 = 0x0FFF_FFFF;
const DOT:    [u8; 11] = *b".          ";
const DOTDOT: [u8; 11] = *b"..         ";

fn sn(name: &str) -> [u8; 11] {
    let mut o = [b' '; 11];
    let u = name.as_bytes();
    let n = u.len().min(8);
    for i in 0..n { o[i] = u[i].to_ascii_uppercase(); }
    o
}
fn sn3(base: &str, ext: &str) -> [u8; 11] {
    let mut o = [b' '; 11];
    let b = base.as_bytes(); let n = b.len().min(8);
    for i in 0..n { o[i] = b[i].to_ascii_uppercase(); }
    let e = ext.as_bytes(); let m = e.len().min(3);
    for i in 0..m { o[8 + i] = e[i].to_ascii_uppercase(); }
    o
}

pub struct Esp<'a> {
    dev: &'a mut dyn BlockDevice,
    bps: u32,
    spc: u32,
    num_fats: u32,
    fat_size: u32,
    fsinfo_sec: u64,
    pub root_cluster: u32,
    fat_start: u64,
    data_start: u64,
    max_cluster: u32,
    hint: u32,
}

impl<'a> Esp<'a> {
    pub fn open(dev: &'a mut dyn BlockDevice) -> Result<Self, &'static str> {
        let mut b = [0u8; 512];
        dev.read_block(0, &mut b)?;
        if b[510] != 0x55 || b[511] != 0xAA { return Err("ESP: invalid boot sector"); }
        let bps = u16::from_le_bytes([b[11], b[12]]) as u32;
        if bps != 512 { return Err("ESP: 512B sector isn't supporting"); }
        let spc = b[13] as u32;
        if spc == 0 || (spc & (spc - 1)) != 0 || spc > 128 { return Err("ESP: cluster size is invalid"); }
        let reserved = u16::from_le_bytes([b[14], b[15]]) as u32;
        if reserved == 0 { return Err("ESP: reserved is invalid"); }
        let num_fats = b[16] as u32;
        if num_fats == 0 || num_fats > 2 { return Err("ESP: FAT number is invalid"); }
        if u16::from_le_bytes([b[22], b[23]]) != 0 { return Err("ESP: isnt FAT32 (FAT16?)"); }
        let fat_size = u32::from_le_bytes([b[36], b[37], b[38], b[39]]);
        if fat_size == 0 { return Err("ESP: FAT32 invalid"); }
        let root_cluster = u32::from_le_bytes([b[44], b[45], b[46], b[47]]);
        if root_cluster < 2 { return Err("ESP: root cluster is invalid"); }
        let fsinfo_sec = u16::from_le_bytes([b[48], b[49]]) as u64;
        let t16 = u16::from_le_bytes([b[19], b[20]]) as u32;
        let total = if t16 != 0 { t16 } else { u32::from_le_bytes([b[32], b[33], b[34], b[35]]) };
        let fat_start = reserved as u64;
        let data_start = (reserved + num_fats * fat_size) as u64;
        if (total as u64) <= data_start { return Err("ESP: geometry is invalid"); }
        let clusters = ((total as u64 - data_start) / spc as u64) as u32;
        if clusters < 65525 { return Err("ESP: FAT32 is invalid (low cluster number)"); }
        Ok(Esp {
            dev, bps, spc, num_fats, fat_size, fsinfo_sec, root_cluster,
            fat_start, data_start, max_cluster: clusters + 1, hint: 2,
        })
    }

    fn cluster_lba(&self, c: u32) -> u64 { self.data_start + (c as u64 - 2) * self.spc as u64 }

    fn fat_get(&mut self, c: u32) -> Result<u32, &'static str> {
        if c < 2 || c > self.max_cluster { return Err("ESP: cluster araligi disi"); }
        let off = c as u64 * 4;
        let sec = self.fat_start + off / self.bps as u64;
        let o = (off % self.bps as u64) as usize;
        let mut b = [0u8; 512];
        self.dev.read_block(sec, &mut b)?;
        Ok(u32::from_le_bytes([b[o], b[o+1], b[o+2], b[o+3]]) & 0x0FFF_FFFF)
    }

    fn fat_set(&mut self, c: u32, val: u32) -> Result<(), &'static str> {
        if c < 2 || c > self.max_cluster { return Err("ESP: cluster araligi disi"); }
        let off = c as u64 * 4;
        let o = (off % self.bps as u64) as usize;
        for i in 0..self.num_fats {
            let sec = self.fat_start + (i * self.fat_size) as u64 + off / self.bps as u64;
            let mut b = [0u8; 512];
            self.dev.read_block(sec, &mut b)?;
            let old = u32::from_le_bytes([b[o], b[o+1], b[o+2], b[o+3]]);
            let nv = (old & 0xF000_0000) | (val & 0x0FFF_FFFF);
            b[o..o+4].copy_from_slice(&nv.to_le_bytes());
            self.dev.write_block(sec, &b)?;
        }
        Ok(())
    }

    fn zero_cluster(&mut self, c: u32) -> Result<(), &'static str> {
        let lba = self.cluster_lba(c);
        for s in 0..self.spc { self.dev.write_block(lba + s as u64, &ZERO)?; }
        Ok(())
    }

    fn alloc_free(&mut self) -> Result<u32, &'static str> {
        for pass in 0..2 {
            let mut c = if pass == 0 { self.hint.max(2) } else { 2 };
            let stop = if pass == 0 { self.max_cluster } else { self.hint.max(2) };
            while c <= stop && c <= self.max_cluster {
                let sidx = (c as u64 * 4) / self.bps as u64;
                let sec = self.fat_start + sidx;
                let mut b = [0u8; 512];
                self.dev.read_block(sec, &mut b)?;
                let mut e = c;
                while e <= self.max_cluster {
                    if (e as u64 * 4) / self.bps as u64 != sidx { break; }
                    let o = ((e as u64 * 4) % self.bps as u64) as usize;
                    let v = u32::from_le_bytes([b[o], b[o+1], b[o+2], b[o+3]]) & 0x0FFF_FFFF;
                    if v == 0 {
                        self.fat_set(e, EOC)?;
                        self.zero_cluster(e)?;
                        self.hint = e + 1;
                        return Ok(e);
                    }
                    e += 1;
                }
                c = e;
            }
        }
        Err("ESP: bos yer yok")
    }

    fn find(&mut self, dir: u32, name: &[u8; 11]) -> Result<Option<(u32, u32, bool)>, &'static str> {
        let mut c = dir;
        let mut guard = 0u32;
        loop {
            if c < 2 || c > self.max_cluster { return Ok(None); }
            let lba = self.cluster_lba(c);
            for s in 0..self.spc {
                let mut b = [0u8; 512];
                self.dev.read_block(lba + s as u64, &mut b)?;
                for o in (0..512).step_by(32) {
                    if b[o] == 0x00 { return Ok(None); }
                    if b[o] == 0xE5 || b[o + 11] == 0x0F { continue; }
                    if b[o..o+11] == name[..] {
                        let hi = u16::from_le_bytes([b[o+20], b[o+21]]) as u32;
                        let lo = u16::from_le_bytes([b[o+26], b[o+27]]) as u32;
                        let sz = u32::from_le_bytes([b[o+28], b[o+29], b[o+30], b[o+31]]);
                        return Ok(Some(((hi << 16) | lo, sz, b[o+11] & 0x10 != 0)));
                    }
                }
            }
            c = self.fat_get(c)?;
            if c >= 0x0FFF_FFF8 { return Ok(None); }
            guard += 1; if guard > 4096 { return Err("ESP: dizin dongusu"); }
        }
    }

    fn add_entry(&mut self, dir: u32, name: &[u8; 11], attr: u8, first: u32, size: u32) -> Result<(), &'static str> {
        let mut c = dir;
        let mut guard = 0u32;
        loop {
            if c < 2 || c > self.max_cluster { return Err("ESP: dizin gecersiz"); }
            let lba = self.cluster_lba(c);
            for s in 0..self.spc {
                let mut b = [0u8; 512];
                self.dev.read_block(lba + s as u64, &mut b)?;
                for o in (0..512).step_by(32) {
                    if b[o] == 0x00 || b[o] == 0xE5 {
                        for x in b[o..o+32].iter_mut() { *x = 0; }
                        b[o..o+11].copy_from_slice(&name[..]);
                        b[o+11] = attr;
                        b[o+20..o+22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
                        b[o+26..o+28].copy_from_slice(&(first as u16).to_le_bytes());
                        b[o+28..o+32].copy_from_slice(&size.to_le_bytes());
                        self.dev.write_block(lba + s as u64, &b)?;
                        return Ok(());
                    }
                }
            }
            let n = self.fat_get(c)?;
            if n >= 0x0FFF_FFF8 {
                let nc = self.alloc_free()?;
                self.fat_set(c, nc)?;
                c = nc;
            } else { c = n; }
            guard += 1; if guard > 4096 { return Err("ESP: dizin dongusu"); }
        }
    }

    fn make_dir(&mut self, parent: u32, name: &[u8; 11]) -> Result<u32, &'static str> {
        let c = self.alloc_free()?; // sifirlanmis + EOC
        let lba = self.cluster_lba(c);
        let mut b = [0u8; 512];
        b[0..11].copy_from_slice(&DOT);
        b[11] = 0x10;
        b[20..22].copy_from_slice(&((c >> 16) as u16).to_le_bytes());
        b[26..28].copy_from_slice(&(c as u16).to_le_bytes());
        let pc = if parent == self.root_cluster { 0 } else { parent };
        b[32..43].copy_from_slice(&DOTDOT);
        b[43] = 0x10;
        b[52..54].copy_from_slice(&((pc >> 16) as u16).to_le_bytes());
        b[58..60].copy_from_slice(&(pc as u16).to_le_bytes());
        self.dev.write_block(lba, &b)?;
        self.add_entry(parent, name, 0x10, c, 0)?;
        Ok(c)
    }

    fn ensure_dir(&mut self, parent: u32, name: &[u8; 11]) -> Result<u32, &'static str> {
        if let Some((c, _, is_dir)) = self.find(parent, name)? {
            if !is_dir { return Err("ESP: ayni isimde dosya var"); }
            if c < 2 || c > self.max_cluster { return Err("ESP: klasor cluster gecersiz"); }
            return Ok(c);
        }
        self.make_dir(parent, name)
    }

    fn free_chain(&mut self, start: u32) -> Result<(), &'static str> {
        let mut c = start;
        let mut guard = 0u32;
        while c >= 2 && c <= self.max_cluster {
            let n = self.fat_get(c)?;
            self.fat_set(c, 0)?;
            if n < 2 || n >= 0x0FFF_FFF8 { break; }
            c = n;
            guard += 1; if guard > 100_000 { break; }
        }
        Ok(())
    }

    fn remove_entry(&mut self, dir: u32, name: &[u8; 11]) -> Result<(), &'static str> {
        let mut c = dir;
        let mut guard = 0u32;
        loop {
            if c < 2 || c > self.max_cluster { return Ok(()); }
            let lba = self.cluster_lba(c);
            for s in 0..self.spc {
                let mut b = [0u8; 512];
                self.dev.read_block(lba + s as u64, &mut b)?;
                let mut dirty = false;
                let mut done = false;
                for o in (0..512).step_by(32) {
                    if b[o] == 0x00 { done = true; break; }
                    if b[o] == 0xE5 || b[o + 11] == 0x0F { continue; }
                    if b[o..o+11] == name[..] { b[o] = 0xE5; dirty = true; }
                }
                if dirty { self.dev.write_block(lba + s as u64, &b)?; }
                if done { return Ok(()); }
            }
            c = self.fat_get(c)?;
            if c >= 0x0FFF_FFF8 { return Ok(()); }
            guard += 1; if guard > 4096 { return Ok(()); }
        }
    }

    pub fn write_file(&mut self, dir: u32, name: &[u8; 11], data: &[u8]) -> Result<(), &'static str> {
        if let Some((old, _, is_dir)) = self.find(dir, name)? {
            if is_dir { return Err("ESP: ayni isimde klasor var"); }
            if old >= 2 && old <= self.max_cluster { self.free_chain(old)?; }
            self.remove_entry(dir, name)?;
        }
        let per = self.spc * self.bps;
        let n = if data.is_empty() { 1 } else { ((data.len() as u32) + per - 1) / per };
        let first = self.alloc_free()?;
        let mut chain: Vec<u32> = Vec::new();
        chain.push(first);
        let mut cur = first;
        for _ in 1..n {
            let nx = self.alloc_free()?;
            self.fat_set(cur, nx)?;
            cur = nx;
            chain.push(nx);
        }
        let mut off = 0usize;
        for &c in chain.iter() {
            let lba = self.cluster_lba(c);
            for s in 0..self.spc {
                let mut b = [0u8; 512];
                if off < data.len() {
                    let end = (off + 512).min(data.len());
                    b[..end - off].copy_from_slice(&data[off..end]);
                    off = end;
                }
                self.dev.write_block(lba + s as u64, &b)?;
            }
        }
        self.add_entry(dir, name, 0x20, first, data.len() as u32)?;
        Ok(())
    }

    fn invalidate_fsinfo(&mut self) {
        if self.fsinfo_sec == 0 || self.fsinfo_sec > 8 { return; }
        let mut b = [0u8; 512];
        if self.dev.read_block(self.fsinfo_sec, &mut b).is_ok() && b[0..4] == [0x52, 0x52, 0x61, 0x41] {
            b[488..492].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            b[492..496].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
            let _ = self.dev.write_block(self.fsinfo_sec, &b);
        }
    }
}

pub fn install_bootloader(dev: &mut dyn BlockDevice, boot: &[u8]) -> Result<bool, &'static str> {
    if boot.is_empty() { return Err("ESP: bootloader bos"); }
    let mut e = Esp::open(dev)?;
    let root = e.root_cluster;

    let efi = e.ensure_dir(root, &sn("EFI"))?;
    let rusty = e.ensure_dir(efi, &sn("RUSTY"))?;
    e.write_file(rusty, &sn3("BOOTX64", "EFI"), boot)?;

    let mut fallback = false;
    let boot_dir = e.find(efi, &sn("BOOT"))?;
    match boot_dir {
        Some((c, _, true)) => {
            if e.find(c, &sn3("BOOTX64", "EFI"))?.is_none() {
                e.write_file(c, &sn3("BOOTX64", "EFI"), boot)?;
                fallback = true;
            }
            // if we have bootx64.efi already, dont touch it
        }
        Some((_, _, false)) => {}
        None => {
            let c = e.make_dir(efi, &sn("BOOT"))?;
            e.write_file(c, &sn3("BOOTX64", "EFI"), boot)?;
            fallback = true;
        }
    }

    e.invalidate_fsinfo();
    Ok(fallback)
}

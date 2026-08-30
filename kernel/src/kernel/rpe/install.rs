use crate::fs::BlockDevice;

static PAYLOAD_BOOT:   &[u8] = include_bytes!("payload/BOOTX64.EFI");
static PAYLOAD_KERNEL: &[u8] = include_bytes!("payload/KERNEL.ELF");
static PAYLOAD_CORE:   &[u8] = include_bytes!("payload/CORE.BIN");

const ZERO: [u8; 512] = [0u8; 512];
const EOC: u32 = 0x0FFF_FFFF;
const DOT:    [u8; 11] = *b".          ";
const DOTDOT: [u8; 11] = *b"..         ";

#[derive(Clone, Copy)]
struct Geom {
    bps: u32,
    spc: u32,
    reserved: u32,
    num_fats: u32,
    fat_size: u32,
    total_sectors: u32,
    root_cluster: u32,
    fat_start: u64,
    data_start: u64,
}

fn pick_spc(ds: u32) -> u32 {
    if ds <= 0x0020_0000 { 8 }
    else if ds <= 0x0100_0000 { 16 }
    else if ds <= 0x0400_0000 { 32 }
    else if ds <= 0x1000_0000 { 64 }
    else { 128 }
}

fn plan(disk_sectors: u32) -> Geom {
    let bps = 512u32;
    let reserved = 32u32;
    let num_fats = 2u32;
    const MAXF: u32 = 8192;

    let mut spc = pick_spc(disk_sectors);
    let mut total = if disk_sectors < 8192 { 8192 } else { disk_sectors };
    let mut fat_size;

    loop {
        let tmp1 = total - reserved;
        let tmp2 = (256 * spc + num_fats) / 2;
        fat_size = (tmp1 + (tmp2 - 1)) / tmp2;
        if fat_size <= MAXF { break; }
        if spc < 128 {
            spc *= 2;
        } else {
            let tmp2b = (256 * 128 + num_fats) / 2;
            total = MAXF * tmp2b + reserved;
            spc = 128;
            let t1 = total - reserved;
            fat_size = (t1 + (tmp2b - 1)) / tmp2b;
            break;
        }
    }

    let fat_start = reserved as u64;
    let data_start = (reserved + num_fats * fat_size) as u64;
    Geom { bps, spc, reserved, num_fats, fat_size, total_sectors: total,
           root_cluster: 2, fat_start, data_start }
}

fn build_boot_sector(g: &Geom) -> [u8; 512] {
    let mut b = [0u8; 512];
    b[0] = 0xEB; b[1] = 0x58; b[2] = 0x90;
    b[3..11].copy_from_slice(b"RUSTYOS ");
    b[11..13].copy_from_slice(&(g.bps as u16).to_le_bytes());
    b[13] = g.spc as u8;
    b[14..16].copy_from_slice(&(g.reserved as u16).to_le_bytes());
    b[16] = g.num_fats as u8;
    b[21] = 0xF8;
    b[24..26].copy_from_slice(&63u16.to_le_bytes());
    b[26..28].copy_from_slice(&255u16.to_le_bytes());
    b[32..36].copy_from_slice(&g.total_sectors.to_le_bytes());
    b[36..40].copy_from_slice(&g.fat_size.to_le_bytes());
    b[44..48].copy_from_slice(&g.root_cluster.to_le_bytes());
    b[48..50].copy_from_slice(&1u16.to_le_bytes());
    b[50..52].copy_from_slice(&6u16.to_le_bytes());
    b[64] = 0x80;
    b[66] = 0x29;
    b[67..71].copy_from_slice(&0x52555354u32.to_le_bytes());
    b[71..82].copy_from_slice(b"RUSTY OS   ");   // mount_fat_smart will search this
    b[82..90].copy_from_slice(b"FAT32   ");
    b[510] = 0x55; b[511] = 0xAA;
    b
}

fn build_fsinfo() -> [u8; 512] {
    let mut b = [0u8; 512];
    b[0..4].copy_from_slice(&0x41615252u32.to_le_bytes());
    b[484..488].copy_from_slice(&0x61417272u32.to_le_bytes());
    b[488..492].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    b[492..496].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    b[508..512].copy_from_slice(&0xAA55_0000u32.to_le_bytes());
    b
}

fn sn(name: &str) -> [u8; 11] {
    let mut o = [b' '; 11];
    let u = name.as_bytes();
    let n = u.len().min(8);
    for i in 0..n { o[i] = u[i].to_ascii_uppercase(); }
    o
}
fn sn3(base: &str, ext: &str) -> [u8; 11] {
    let mut o = [b' '; 11];
    let bb = base.as_bytes(); let n = bb.len().min(8);
    for i in 0..n { o[i] = bb[i].to_ascii_uppercase(); }
    let eb = ext.as_bytes(); let m = eb.len().min(3);
    for i in 0..m { o[8 + i] = eb[i].to_ascii_uppercase(); }
    o
}
fn put_cluster(e: &mut [u8; 32], first: u32, size: u32) {
    e[20..22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
    e[26..28].copy_from_slice(&(first as u16).to_le_bytes());
    e[28..32].copy_from_slice(&size.to_le_bytes());
}

// FORMATTER
struct Fmt<'a> {
    dev: &'a mut dyn BlockDevice,
    g: Geom,
    next_free: u32,
    max_cluster: u32,
}

impl<'a> Fmt<'a> {
    fn cluster_lba(&self, c: u32) -> u64 {
        self.g.data_start + ((c as u64 - 2) * self.g.spc as u64)
    }

    fn alloc_run(&mut self, n: u32) -> Result<u32, &'static str> {
        if n == 0 || self.next_free + n > self.max_cluster {
            return Err("There is no space in the target section");
        }
        let s = self.next_free;
        self.next_free += n;
        Ok(s)
    }

    fn fat_next(&mut self, c: u32) -> Result<u32, &'static str> {
        let off = c as u64 * 4;
        let sector = self.g.fat_start + off / self.g.bps as u64;
        let o = (off % self.g.bps as u64) as usize;
        let mut sec = [0u8; 512];
        self.dev.read_block(sector, &mut sec)?;
        Ok(u32::from_le_bytes([sec[o], sec[o+1], sec[o+2], sec[o+3]]) & 0x0FFF_FFFF)
    }

    fn set_fat(&mut self, cluster: u32, val: u32) -> Result<(), &'static str> {
        let off = cluster as u64 * 4;
        let sector = self.g.fat_start + off / self.g.bps as u64;
        let o = (off % self.g.bps as u64) as usize;
        let mut sec = [0u8; 512];
        self.dev.read_block(sector, &mut sec)?;
        sec[o..o+4].copy_from_slice(&val.to_le_bytes());
        self.dev.write_block(sector, &sec)?;
        self.dev.write_block(sector + self.g.fat_size as u64, &sec)?;
        Ok(())
    }

    fn write_fat_chain(&mut self, start: u32, n: u32) -> Result<(), &'static str> {
        let mut done = 0u32;
        while done < n {
            let cluster = start + done;
            let sector = self.g.fat_start + (cluster as u64 * 4 / self.g.bps as u64);
            let mut sec = [0u8; 512];
            self.dev.read_block(sector, &mut sec)?;
            let mut cl = cluster;
            while cl < start + n {
                let s2 = self.g.fat_start + (cl as u64 * 4 / self.g.bps as u64);
                if s2 != sector { break; }
                let o = ((cl as u64 * 4) % self.g.bps as u64) as usize;
                let val = if cl == start + n - 1 { EOC } else { cl + 1 };
                sec[o..o+4].copy_from_slice(&val.to_le_bytes());
                cl += 1;
                done += 1;
            }
            self.dev.write_block(sector, &sec)?;
            self.dev.write_block(sector + self.g.fat_size as u64, &sec)?;
        }
        Ok(())
    }

    fn add_entry(&mut self, dir: u32, name: [u8; 11], attr: u8, first: u32, size: u32)
        -> Result<(), &'static str> {
        let mut c = dir;
        let mut guard = 0u32;
        loop {
            let lba = self.cluster_lba(c);
            for s in 0..self.g.spc {
                let mut sec = [0u8; 512];
                self.dev.read_block(lba + s as u64, &mut sec)?;
                for off in (0..512).step_by(32) {
                    let fb = sec[off];
                    if fb == 0x00 || fb == 0xE5 {
                        for b in sec[off..off+32].iter_mut() { *b = 0; }
                        sec[off..off+11].copy_from_slice(&name);
                        sec[off + 11] = attr;
                        sec[off+20..off+22].copy_from_slice(&((first >> 16) as u16).to_le_bytes());
                        sec[off+26..off+28].copy_from_slice(&(first as u16).to_le_bytes());
                        sec[off+28..off+32].copy_from_slice(&size.to_le_bytes());
                        self.dev.write_block(lba + s as u64, &sec)?;
                        return Ok(());
                    }
                }
            }
            let n = self.fat_next(c)?;
            if n < 2 || n >= 0x0FFF_FFF8 {
                let nc = self.alloc_run(1)?;
                let nlba = self.cluster_lba(nc);
                for s in 0..self.g.spc { self.dev.write_block(nlba + s as u64, &ZERO)?; }
                self.set_fat(c, nc)?;
                self.set_fat(nc, EOC)?;
                c = nc;
            } else { c = n; }
            guard += 1;
            if guard > 4096 { return Err("dizin dongusu"); }
        }
    }

    fn make_dir(&mut self, parent: u32, name: [u8; 11]) -> Result<u32, &'static str> {
        let c = self.alloc_run(1)?;
        let lba = self.cluster_lba(c);
        for s in 0..self.g.spc { self.dev.write_block(lba + s as u64, &ZERO)?; }

        let mut dot = [0u8; 32];
        dot[0..11].copy_from_slice(&DOT);
        dot[11] = 0x10;
        put_cluster(&mut dot, c, 0);

        let mut dd = [0u8; 32];
        dd[0..11].copy_from_slice(&DOTDOT);
        dd[11] = 0x10;
        let pc = if parent == self.g.root_cluster { 0 } else { parent };
        put_cluster(&mut dd, pc, 0);

        let mut sec = [0u8; 512];
        sec[0..32].copy_from_slice(&dot);
        sec[32..64].copy_from_slice(&dd);
        self.dev.write_block(lba, &sec)?;

        self.set_fat(c, EOC)?;
        self.add_entry(parent, name, 0x10, c, 0)?;
        Ok(c)
    }

    fn format(&mut self, cb: &mut dyn FnMut(usize, u32)) -> Result<(), &'static str> {
        let fat_total = self.g.num_fats * self.g.fat_size;
        for i in 0..fat_total {
            self.dev.write_block(self.g.fat_start + i as u64, &ZERO)?;
            if i % 64 == 0 { cb(0, i * 100 / fat_total.max(1)); }
        }
        let rlba = self.cluster_lba(self.g.root_cluster);
        for s in 0..self.g.spc { self.dev.write_block(rlba + s as u64, &ZERO)?; }
        self.set_fat(0, 0x0FFF_FFF8)?;
        self.set_fat(1, EOC)?;
        self.set_fat(2, EOC)?;
        let bs = build_boot_sector(&self.g);
        self.dev.write_block(0, &bs)?;
        self.dev.write_block(6, &bs)?;
        let fsi = build_fsinfo();
        self.dev.write_block(1, &fsi)?;
        self.dev.write_block(7, &fsi)?;
        cb(0, 100);
        Ok(())
    }

    // Bellekteki (gomulu) veriyi dosya olarak yaz - heap kullanmaz
    fn write_data(&mut self, data: &[u8], dir: u32, name: [u8; 11],
                  step: usize, cb: &mut dyn FnMut(usize, u32)) -> Result<(), &'static str> {
        let size = data.len() as u32;
        let per = self.g.spc * 512;
        let tn = if size == 0 { 1 } else { (size + per - 1) / per };
        let tstart = self.alloc_run(tn)?;
        let tbase = self.cluster_lba(tstart);
        let run_sectors = tn * self.g.spc;

        let mut buf = [0u8; 512];
        let mut i = 0u32;
        while i < run_sectors {
            for b in buf.iter_mut() { *b = 0; }
            let off = (i as usize) * 512;
            if off < data.len() {
                let end = (off + 512).min(data.len());
                buf[..end - off].copy_from_slice(&data[off..end]);
            }
            self.dev.write_block(tbase + i as u64, &buf)?;
            i += 1;
            if i % 64 == 0 { cb(step, i * 100 / run_sectors.max(1)); }
        }
        self.write_fat_chain(tstart, tn)?;
        self.add_entry(dir, name, 0x20, tstart, size)?;
        cb(step, 100);
        Ok(())
    }
}

fn build_registry() -> [u8; 4096] {
    let hdr: &[u8] = b"Sistem/Ad=str:Rusty OS\nSistem/Surum=str:0.1\nSistem/Dil=str:tr\nSistem/Masaustu/Renk=u32:5249032\nSistem/Taskbar/Renk=u32:2101256\nSistem/Saat/UTC=u32:3\nSistem/Saat/24Saat=bool:0\nSistem/Ses/Seviye=u32:100\nOturum/IlkKurulumBitti=bool:0\n";
    let mut b = [b'\n'; 4096];
    let n = hdr.len().min(4096);
    b[..n].copy_from_slice(&hdr[..n]);
    b
}

pub fn install(dev: &mut dyn BlockDevice, disk_sectors: u32, disk_kind: u8,
               esp: Option<(u64, u64)>, cb: &mut dyn FnMut(usize, u32))
               -> Result<(), &'static str> {
    if dev.block_size() != 512 { return Err("hedef bolum 512B sektor degil"); }
    if PAYLOAD_KERNEL.len() < 4096 { return Err("payload gomulmemis (make rpe-usb kullan)"); }
    if PAYLOAD_BOOT.len() < 1024 { return Err("payload bootloader gomulmemis"); }
    if PAYLOAD_CORE.len() < 1024 { return Err("payload core.bin gomulmemis"); }

    let g = plan(disk_sectors);
    let max_cluster = 2 + ((g.total_sectors as u64 - g.data_start) / g.spc as u64) as u32;
    let mut f = Fmt { dev, g, next_free: 3, max_cluster };

    cb(0, 0);
    f.format(cb)?;

    let efi  = f.make_dir(g.root_cluster, sn("EFI"))?;
    let boot = f.make_dir(efi, sn("BOOT"))?;
    let rsys = f.make_dir(g.root_cluster, sn("RSYS"))?;
    let _apps = f.make_dir(g.root_cluster, sn("APPS"))?;

    // bootloader
    cb(1, 0);
    f.write_data(PAYLOAD_BOOT, boot, sn3("BOOTX64", "EFI"), 1, cb)?;

    // kernel
    cb(2, 0);
    f.write_data(PAYLOAD_KERNEL, g.root_cluster, sn3("KERNEL", "ELF"), 2, cb)?;
    f.write_data(PAYLOAD_KERNEL, rsys, sn3("KERNEL", "ELF"), 2, cb)?;

    // system files
    cb(3, 0);
    f.write_data(PAYLOAD_CORE, rsys, sn3("CORE", "BIN"), 3, cb)?;
    let reg = build_registry();
    f.write_data(&reg, rsys, sn3("REGISTRY", "DAT"), 3, cb)?;

    // esp bootloader
    if let Some((elba, esec)) = esp {
        cb(3, 60);
        let mut edev = crate::fs::offset::PartitionDevice::new(disk_kind, elba, esec);
        super::esp::install_bootloader(&mut edev, PAYLOAD_BOOT)?;
        cb(3, 100);
    }

    cb(4, 100);
    Ok(())
}

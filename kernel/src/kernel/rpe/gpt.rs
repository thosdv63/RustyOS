use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
use spin::Mutex;
use crate::fs::BlockDevice;
use crate::fs::fat32::{Fat32, Fat32FileSystem};
use crate::fs::offset::PartitionDevice;

pub const FS_UNKNOWN: u8 = 0;
pub const FS_FAT: u8 = 1;
pub const FS_NTFS: u8 = 2;
pub const FS_EMPTY: u8 = 3;

pub struct Partition {
    pub index: u32,
    pub first_lba: u64,
    pub last_lba: u64,    
    pub sectors: u64,      
    pub name: String,      
    pub label: &'static str,
    pub fs_kind: u8,
    pub protected: bool,   
}

pub struct DiskLayout {
    pub total_sectors: u64,
    pub has_gpt: bool,
    pub partitions: Vec<Partition>,
}

const G_EFI:       [u8; 16] = [0x28,0x73,0x2A,0xC1,0x1F,0xF8,0xD2,0x11,0xBA,0x4B,0x00,0xA0,0xC9,0x3E,0xC9,0x3B];
const G_MSR:       [u8; 16] = [0x16,0xE3,0xC9,0xE3,0x5C,0x0B,0xB8,0x4D,0x81,0x7D,0xF9,0x2D,0xF0,0x02,0x15,0xAE];
const G_MSDATA:    [u8; 16] = [0xA2,0xA0,0xD0,0xEB,0xE5,0xB9,0x33,0x44,0x87,0xC0,0x68,0xB6,0xB7,0x26,0x99,0xC7];
const G_WINRE:     [u8; 16] = [0xA4,0xBB,0x94,0xDE,0xD1,0x06,0x40,0x4D,0xA1,0x6A,0xBF,0xD5,0x01,0x79,0xD6,0xAC];
const G_LINUX:     [u8; 16] = [0xAF,0x3D,0xC6,0x0F,0x83,0x84,0x72,0x47,0x8E,0x79,0x3D,0x69,0xD8,0x47,0x7D,0xE4];
const G_LINUXLVM:  [u8; 16] = [0x79,0xD3,0xD6,0xE6,0x07,0xF5,0xC2,0x44,0xA2,0x3C,0x23,0x8F,0x2A,0x3D,0xF9,0x28];
const G_LINUXSWAP: [u8; 16] = [0x6D,0xFD,0x57,0x06,0xAB,0xA4,0xC4,0x43,0x84,0xE5,0x09,0x33,0xC8,0x4B,0x4F,0x4F];
const G_ZERO:      [u8; 16] = [0; 16];

fn le32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }
fn le64(b: &[u8]) -> u64 { u64::from_le_bytes([b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7]]) }

fn decode_name(raw: &[u8]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i + 1 < raw.len() {
        let c = u16::from_le_bytes([raw[i], raw[i + 1]]);
        if c == 0 { break; }
        if (0x20..0x7F).contains(&c) { s.push(c as u8 as char); }
        else { s.push('?'); }
        i += 2;
    }
    s
}

fn sniff_fs(dev: &mut dyn BlockDevice, first_lba: u64) -> u8 {
    let mut b = [0u8; 512];
    if dev.read_block(first_lba, &mut b).is_err() { return FS_UNKNOWN; }
    if &b[3..7] == b"NTFS" { return FS_NTFS; }
    if &b[3..11] == b"EXFAT   " { return FS_NTFS; }
    if &b[82..87] == b"FAT32" { return FS_FAT; }
    if &b[54..57] == b"FAT" { return FS_FAT; }      // FAT12/16
    
    if b.iter().all(|&x| x == 0) || b[510] != 0x55 || b[511] != 0xAA {
        return FS_EMPTY;
    }

    FS_UNKNOWN
}

fn classify(tg: &[u8; 16], fs: u8) -> (&'static str, bool) {
    if tg == &G_EFI       { return ("EFI Sistem",         true); }
    if tg == &G_MSR       { return ("Microsoft Ayrilmis", true); }
    if tg == &G_WINRE     { return ("Windows Kurtarma",   true); }
    if tg == &G_LINUX     { return ("Linux",              true); }
    if tg == &G_LINUXLVM  { return ("Linux LVM",          true); }
    if tg == &G_LINUXSWAP { return ("Linux Swap",         true); }
    if tg == &G_MSDATA {
        return match fs {
            FS_NTFS  => ("Windows (NTFS)", true),  
            FS_FAT   => ("FAT32 (bos)",    false), 
            FS_EMPTY => ("Bos (Basic)",    false), 
            _        => ("Kurulabilir Alan", false),
        };
    }
    ("Bilinmeyen", true)
}

pub fn read_disk(dev: &mut dyn BlockDevice, total_sectors: u64) -> DiskLayout {
    let mut out = DiskLayout { total_sectors, has_gpt: false, partitions: Vec::new() };

    // GPT header @ LBA 1
    let mut hdr = [0u8; 512];
    if dev.read_block(1, &mut hdr).is_err() { return out; }
    if &hdr[0..8] != b"EFI PART" { return out; } // no gpt
    out.has_gpt = true;

    let entry_lba = le64(&hdr[72..80]);
    let num_entries = le32(&hdr[80..84]).min(128);
    let entry_size = le32(&hdr[84..88]);
    if entry_size < 128 || entry_size > 512 { return out; }
    let per_sec = (512 / entry_size).max(1);

    let mut idx = 0u32;
    let mut sector = entry_lba;
    'outer: loop {
        let mut buf = [0u8; 512];
        if dev.read_block(sector, &mut buf).is_err() { break; }
        for e in 0..per_sec {
            if idx >= num_entries { break 'outer; }
            let off = (e * entry_size) as usize;
            let ent = &buf[off..off + entry_size as usize];
            let mut tg = [0u8; 16];
            tg.copy_from_slice(&ent[0..16]);
            idx += 1;
            if tg == G_ZERO { continue; }
            let first = le64(&ent[32..40]);
            let last = le64(&ent[40..48]);
            if first == 0 || last < first { continue; }
            let name = decode_name(&ent[56..128]);
            let fs = sniff_fs(dev, first);
            let (label, protected) = classify(&tg, fs);
            out.partitions.push(Partition {
                index: idx, first_lba: first, last_lba: last,
                sectors: last - first + 1, name, label, fs_kind: fs, protected,
            });
        }
        sector += 1;
    }
    out
}

// KERNEL MOUNT
pub fn mount_fat_smart(kind: u8, block_size: u32, block_count: u64) -> Option<Fat32FileSystem> {
    if block_size != 512 { return None; }

    let has_gpt = {
        let mut d = PartitionDevice::new(kind, 0, block_count);
        let mut hdr = [0u8; 512];
        d.read_block(1, &mut hdr).is_ok() && &hdr[0..8] == b"EFI PART"
    };

    if !has_gpt {
        let mut probe = PartitionDevice::new(kind, 0, block_count);
        if let Ok(fat) = Fat32::new(&mut probe) {
            let dev: Arc<Mutex<dyn BlockDevice>> =
                Arc::new(Mutex::new(PartitionDevice::new(kind, 0, block_count)));
            return Some(Fat32FileSystem { fat: Arc::new(Mutex::new(fat)), dev });
        }
        return None;
    }

    let layout = {
        let mut d = PartitionDevice::new(kind, 0, block_count);
        read_disk(&mut d, block_count)
    };
    for p in &layout.partitions {
        let mut probe = PartitionDevice::new(kind, p.first_lba, p.sectors);
        if let Ok(fat) = Fat32::new(&mut probe) {
            let mut bs = [0u8; 512];
            if probe.read_block(0, &mut bs).is_ok() && &bs[71..82] == b"RUSTY OS   " {
                let dev: Arc<Mutex<dyn BlockDevice>> =
                    Arc::new(Mutex::new(PartitionDevice::new(kind, p.first_lba, p.sectors)));
                return Some(Fat32FileSystem { fat: Arc::new(Mutex::new(fat)), dev });
            }
        }
    }
    None
}

pub fn find_esp(kind: u8, total: u64) -> Option<(u64, u64)> {
    let mut d = PartitionDevice::new(kind, 0, total);
    let layout = read_disk(&mut d, total);
    for p in &layout.partitions {
        if p.label == "EFI Sistem" { return Some((p.first_lba, p.sectors)); }
    }
    None
}

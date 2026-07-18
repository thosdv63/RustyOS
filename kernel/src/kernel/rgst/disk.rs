use alloc::vec;
use crate::fs::BlockDevice;
use crate::fs::fat32::{Fat32, Fat32FileSystem};
use super::REGISTRY;
use core::sync::atomic::{AtomicU32, Ordering};

const REG_DIR: &str = "RSYS";
const REG_FILE: &str = "REGISTRY.DAT";
const REG_CAPACITY: usize = 4096;

// REGISTRY.DAT first cluster
static REG_CLUSTER: AtomicU32 = AtomicU32::new(0);

fn cluster_to_lba(fat: &Fat32, c: u32) -> u64 {
    fat.data_start_lba + ((c as u64 - 2) * fat.sectors_per_cluster as u64)
}

fn next_cluster(fat: &Fat32, dev: &mut dyn BlockDevice, c: u32) -> Result<u32, &'static str> {
    let off = c as u64 * 4;
    let sector = fat.fat_start_lba + off / fat.bytes_per_sector as u64;
    let o = (off % fat.bytes_per_sector as u64) as usize;
    let mut buf = vec![0u8; fat.bytes_per_sector as usize];
    dev.read_block(sector, &mut buf)?;
    Ok(u32::from_le_bytes([buf[o], buf[o+1], buf[o+2], buf[o+3]]) & 0x0FFF_FFFF)
}

fn find_reg_cluster(fat: &Fat32, dev: &mut dyn BlockDevice) -> Result<u32, &'static str> {
    let root = fat.list_root(dev)?;
    let rsys = root.iter()
        .find(|e| e.is_dir && e.name.eq_ignore_ascii_case(REG_DIR))
        .ok_or("RSYS doesnt exist")?;
    let entries = fat.list_dir(dev, rsys.first_cluster)?;
    let reg = entries.iter()
        .find(|e| !e.is_dir && e.name.eq_ignore_ascii_case(REG_FILE))
        .ok_or("RSYS/REGISTRY.DAT doesnt exist")?;
    if reg.first_cluster < 2 { return Err("registry cluster invalid"); }
    Ok(reg.first_cluster)
}

pub fn load(fs: &Fat32FileSystem) -> Result<(), &'static str> {
    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();
    let cluster0 = find_reg_cluster(&fat, &mut *dev)?;
    REG_CLUSTER.store(cluster0, Ordering::Relaxed);

    let bps = fat.bytes_per_sector as usize;
    let mut data = vec![0u8; REG_CAPACITY];
    let mut read = 0usize;
    let mut cluster = cluster0;
    let mut guard = 0u32;
    while read < REG_CAPACITY {
        if cluster < 2 || cluster >= 0x0FFF_FFF8 { break; }
        let lba = cluster_to_lba(&fat, cluster);
        for s in 0..fat.sectors_per_cluster {
            if read >= REG_CAPACITY { break; }
            let mut sec = vec![0u8; bps];
            if dev.read_block(lba + s as u64, &mut sec).is_err() { break; }
            let n = bps.min(REG_CAPACITY - read);
            data[read..read + n].copy_from_slice(&sec[..n]);
            read += n;
        }
        cluster = match next_cluster(&fat, &mut *dev, cluster) { Ok(v) => v, Err(_) => break };
        guard += 1; if guard > 64 { break; }
    }

    let text = core::str::from_utf8(&data).unwrap_or("");
    REGISTRY.lock().deserialize(text);
    Ok(())
}

pub fn save(fs: &Fat32FileSystem) -> Result<(), &'static str> {
    let mut text = REGISTRY.lock().serialize();
    if text.len() > REG_CAPACITY { return Err("Registry capacity exceeded 4KB"); }
    while text.len() < REG_CAPACITY { text.push('\n'); }
    let data = text.into_bytes();

    let fat = fs.fat.lock();
    let mut dev = fs.dev.lock();

    let mut cluster = REG_CLUSTER.load(Ordering::Relaxed);
    if cluster < 2 {
        cluster = find_reg_cluster(&fat, &mut *dev)?;
        REG_CLUSTER.store(cluster, Ordering::Relaxed);
    }

    let bps = fat.bytes_per_sector as usize;
    if bps < 512 || bps > 4096 { return Err("invalid bps"); }

    let mut written = 0usize;
    let mut guard = 0u32;
    while written < data.len() {
        if cluster < 2 || cluster >= 0x0FFF_FFF8 { return Err("registry zinciri kisa"); }
        let lba = cluster_to_lba(&fat, cluster);
        for s in 0..fat.sectors_per_cluster {
            if written >= data.len() { break; }
            let end = (written + bps).min(data.len());
            let mut sec = vec![0u8; bps];
            sec[..end - written].copy_from_slice(&data[written..end]);
            dev.write_block(lba + s as u64, &sec)?;
            written = end;
        }
        cluster = next_cluster(&fat, &mut *dev, cluster)?;
        guard += 1; if guard > 64 { return Err("loop"); }
    }
    Ok(())
}
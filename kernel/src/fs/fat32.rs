use crate::fs::BlockDevice;
use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;
use crate::fs::vfs::{INode, VfsMetadata, NodeType, FileSystem};
use alloc::boxed::Box;
use alloc::sync::Arc;
use spin::Mutex;

#[derive(Clone)]
pub struct Fat32FileSystem {
    pub fat: Arc<Mutex<Fat32>>,
    pub dev: Arc<Mutex<dyn BlockDevice>>,
}

impl FileSystem for Fat32FileSystem {
    fn root_node(&self) -> Result<Box<dyn INode>, &'static str> {
        let fat_lock = self.fat.lock();
        let root_entry = DirEntry {
            name: String::from("/"),
            is_dir: true,
            size: 0,
            first_cluster: fat_lock.root_cluster,
            parent_cluster: 0,
        };
        Ok(Box::new(Fat32Node {
            fat: self.fat.clone(),
            dev: self.dev.clone(),
            entry: root_entry,
        }))
    }
    fn name(&self) -> &'static str { "FAT32" }
}

pub struct Fat32Node {
    pub fat: Arc<Mutex<Fat32>>,
    pub dev: Arc<Mutex<dyn BlockDevice>>,
    pub entry: DirEntry,
}

impl INode for Fat32Node {
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
        if self.entry.is_dir { return Err("Bu bir dizin, dosya olarak okunamadi"); }
        let fat_lock = self.fat.lock();
        let mut dev_lock = self.dev.lock();
        let full_data = fat_lock.read_file(&mut *dev_lock, &self.entry)?;
        if offset >= full_data.len() { return Ok(0); }
        let end = core::cmp::min(offset + buf.len(), full_data.len());
        let len = end - offset;
        buf[..len].copy_from_slice(&full_data[offset..end]);
        Ok(len)
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize, &'static str> {
        if self.entry.is_dir { return Err("Dizine dosya gibi yazilamaz"); }
        let fat_lock = self.fat.lock();
        let mut dev_lock = self.dev.lock();
        let mut entry_clone = self.entry.clone();
        let mut full_data = fat_lock.read_file(&mut *dev_lock, &entry_clone)
            .unwrap_or_else(|_| Vec::new());
        if offset + buf.len() > full_data.len() {
            full_data.resize(offset + buf.len(), 0);
        }
        full_data[offset..offset + buf.len()].copy_from_slice(buf);
        fat_lock.write_file(&mut *dev_lock, &mut entry_clone, &full_data)?;
        self.entry = entry_clone;
        Ok(buf.len())
    }

    fn metadata(&self) -> Result<VfsMetadata, &'static str> {
        Ok(VfsMetadata {
            name: self.entry.name.clone(),
            size: self.entry.size,
            node_type: if self.entry.is_dir { NodeType::Directory } else { NodeType::File },
        })
    }

    fn read_dir(&self) -> Result<Vec<VfsMetadata>, &'static str> {
        if !self.entry.is_dir { return Err("Bu node bir dizin degil"); }
        let fat_lock = self.fat.lock();
        let mut dev_lock = self.dev.lock();
        let entries = fat_lock.list_dir(&mut *dev_lock, self.entry.first_cluster)?;
        let mut meta_list = Vec::new();
        for e in entries {
            meta_list.push(VfsMetadata {
                name: e.name.clone(),
                size: e.size,
                node_type: if e.is_dir { NodeType::Directory } else { NodeType::File },
            });
        }
        Ok(meta_list)
    }

    fn find(&self, name: &str) -> Result<Box<dyn INode>, &'static str> {
        if !self.entry.is_dir { return Err("Bu node bir dizin degil"); }
        let fat_lock = self.fat.lock();
        let mut dev_lock = self.dev.lock();
        let entries = fat_lock.list_dir(&mut *dev_lock, self.entry.first_cluster)?;
        for e in entries {
            if e.name == name {
                return Ok(Box::new(Fat32Node {
                    fat: self.fat.clone(),
                    dev: self.dev.clone(),
                    entry: e,
                }));
            }
        }
        Err("Dosya bulunamadi")
    }
}

// FAT32 Boot Sector parameters
pub struct Fat32 {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub fat_size_sectors: u32,
    pub root_cluster: u32,
    pub fat_start_lba: u64,
    pub data_start_lba: u64,
}

// A directory entry (file/folder)
#[derive(Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u32,
    pub first_cluster: u32,
    pub parent_cluster: u32,
}

impl Fat32 {
    pub fn new(dev: &mut dyn BlockDevice) -> Result<Fat32, &'static str> {
        let mut boot = vec![0u8; 512];
        dev.read_block(0, &mut boot)?;

        let bytes_per_sector = u16::from_le_bytes([boot[11], boot[12]]) as u32;
        let sectors_per_cluster = boot[13] as u32;
        let reserved_sectors = u16::from_le_bytes([boot[14], boot[15]]) as u32;
        let num_fats = boot[16] as u32;
        let fat_size_sectors = u32::from_le_bytes([boot[36], boot[37], boot[38], boot[39]]);
        let root_cluster = u32::from_le_bytes([boot[44], boot[45], boot[46], boot[47]]);

        if boot[510] != 0x55 || boot[511] != 0xAA {
            return Err("no FAT32 signature (boot sector is incorrect)");
        }
        if bytes_per_sector == 0 || sectors_per_cluster == 0 {
            return Err("Invalid FAT32 parameters");
        }

        let fat_start_lba = reserved_sectors as u64;
        let data_start_lba = (reserved_sectors + num_fats * fat_size_sectors) as u64;

        Ok(Fat32 {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            fat_size_sectors,
            root_cluster,
            fat_start_lba,
            data_start_lba,
        })
    }

    fn cluster_to_lba(&self, cluster: u32) -> u64 {
        self.data_start_lba + ((cluster as u64 - 2) * self.sectors_per_cluster as u64)
    }

    fn next_cluster(&self, dev: &mut dyn BlockDevice, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_lba + (fat_offset / self.bytes_per_sector) as u64;
        let offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut buf = vec![0u8; self.bytes_per_sector as usize];
        dev.read_block(sector, &mut buf)?;

        let val = u32::from_le_bytes([
            buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]
        ]) & 0x0FFF_FFFF;
        Ok(val)
    }

    fn cluster_chain(&self, dev: &mut dyn BlockDevice, start: u32) -> Result<Vec<u32>, &'static str> {
        let mut chain = Vec::new();
        let mut current = start;
        let mut guard = 0;
        loop {
            if current < 2 || current >= 0x0FFF_FFF8 { break; }
            chain.push(current);
            current = self.next_cluster(dev, current)?;
            guard += 1;
            if guard > 100_000 { return Err("The cluster chain is very long (loop?)"); }
        }
        Ok(chain)
    }

    fn read_cluster(&self, dev: &mut dyn BlockDevice, cluster: u32) -> Result<Vec<u8>, &'static str> {
        let lba = self.cluster_to_lba(cluster);
        let cluster_bytes = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let mut data = vec![0u8; cluster_bytes];
        for s in 0..self.sectors_per_cluster {
            let mut sector_buf = vec![0u8; self.bytes_per_sector as usize];
            dev.read_block(lba + s as u64, &mut sector_buf)?;
            let start = (s * self.bytes_per_sector) as usize;
            data[start..start + self.bytes_per_sector as usize].copy_from_slice(&sector_buf);
        }
        Ok(data)
    }

    pub fn list_dir(&self, dev: &mut dyn BlockDevice, dir_cluster: u32) -> Result<Vec<DirEntry>, &'static str> {
        let mut entries = Vec::new();
        let chain = self.cluster_chain(dev, dir_cluster)?;
        let mut lfn_name = String::new();

        for cluster in chain {
            let data = self.read_cluster(dev, cluster)?;
            for i in (0..data.len()).step_by(32) {
                if i + 32 > data.len() { break; }
                let entry = &data[i..i+32];

                let first_byte = entry[0];
                if first_byte == 0x00 { return Ok(entries); }
                if first_byte == 0xE5 { lfn_name.clear(); continue; }

                let attr = entry[11];
                if attr == 0x0F {
                    let mut part = String::new();
                    let positions = [1,3,5,7,9, 14,16,18,20,22,24, 28,30];
                    for &p in positions.iter() {
                        let ch = entry[p];
                        if ch == 0 || ch == 0xFF { break; }
                        part.push(ch as char);
                    }
                    lfn_name = part + &lfn_name;
                    continue;
                }
                if attr & 0x08 != 0 { lfn_name.clear(); continue; }

                let name = if !lfn_name.is_empty() {
                    let n = lfn_name.clone();
                    lfn_name.clear();
                    n
                } else {
                    parse_short_name(entry)
                };

                let is_dir = (attr & 0x10) != 0;
                let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]);
                let cluster_hi = u16::from_le_bytes([entry[20], entry[21]]) as u32;
                let cluster_lo = u16::from_le_bytes([entry[26], entry[27]]) as u32;
                let first_cluster = (cluster_hi << 16) | cluster_lo;

                entries.push(DirEntry {
                    name, is_dir, size, first_cluster,
                    parent_cluster: dir_cluster,
                });
            }
        }
        Ok(entries)
    }

    pub fn list_root(&self, dev: &mut dyn BlockDevice) -> Result<Vec<DirEntry>, &'static str> {
        self.list_dir(dev, self.root_cluster)
    }

    pub fn read_file(&self, dev: &mut dyn BlockDevice, entry: &DirEntry) -> Result<Vec<u8>, &'static str> {
        if entry.is_dir { return Err("bu bir dizin, dosya degil"); }
        let chain = self.cluster_chain(dev, entry.first_cluster)?;
        let mut data = Vec::new();
        for cluster in chain {
            let cluster_data = self.read_cluster(dev, cluster)?;
            data.extend_from_slice(&cluster_data);
        }
        data.truncate(entry.size as usize);
        Ok(data)
    }

    // WRITE FUNCTIONS

    fn write_cluster(&self, dev: &mut dyn BlockDevice, cluster: u32, data: &[u8]) -> Result<(), &'static str> {
        let lba = self.cluster_to_lba(cluster);
        let cluster_bytes = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        if data.len() != cluster_bytes {
            return Err("The data to be written is not the full size of a cluster.");
        }
        for s in 0..self.sectors_per_cluster {
            let start = (s * self.bytes_per_sector) as usize;
            let end = start + self.bytes_per_sector as usize;
            dev.write_block(lba + s as u64, &data[start..end])?;
        }
        Ok(())
    }

    fn set_fat_entry(&self, dev: &mut dyn BlockDevice, cluster: u32, next_val: u32) -> Result<(), &'static str> {
        let fat_offset = cluster * 4;
        let sector = self.fat_start_lba + (fat_offset / self.bytes_per_sector) as u64;
        let offset = (fat_offset % self.bytes_per_sector) as usize;

        let mut buf = vec![0u8; self.bytes_per_sector as usize];
        dev.read_block(sector, &mut buf)?;

        // FAT32 entries are 28-bit: top 4 bits reserved, MUST BE PROTECTED
        let old = u32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]]);
        let newv = (old & 0xF000_0000) | (next_val & 0x0FFF_FFFF);
        buf[offset..offset+4].copy_from_slice(&newv.to_le_bytes());

        dev.write_block(sector, &buf)?;
        if self.num_fats > 1 {
            let backup_sector = sector + self.fat_size_sectors as u64;
            dev.write_block(backup_sector, &buf)?;
        }
        Ok(())
    }

    fn alloc_cluster(&self, dev: &mut dyn BlockDevice) -> Result<u32, &'static str> {
        let total_entries = (self.fat_size_sectors * self.bytes_per_sector) / 4;
        // start from 3 (2 is root)
        for cluster in 3..total_entries {
            if self.next_cluster(dev, cluster)? == 0 {
                self.set_fat_entry(dev, cluster, 0x0FFFFFFF)?;
                // Reset new cluster, we dont want trash data
                let zero = vec![0u8; self.bytes_per_sector as usize];
                let lba = self.cluster_to_lba(cluster);
                for s in 0..self.sectors_per_cluster {
                    dev.write_block(lba + s as u64, &zero)?;
                }
                return Ok(cluster);
            }
        }
        Err("There is no free space left on the disk")
    }

    fn update_dir_entry(&self, dev: &mut dyn BlockDevice, dir_cluster: u32, target_name: &str, new_size: u32, new_cluster: u32) -> Result<(), &'static str> {
        let chain = self.cluster_chain(dev, dir_cluster)?;
        let mut lfn_name = String::new();

        for cluster in chain {
            let mut data = self.read_cluster(dev, cluster)?;
            let mut modified = false;

            for i in (0..data.len()).step_by(32) {
                if i + 32 > data.len() { break; }
                let first_byte = data[i];
                if first_byte == 0x00 {
                    return Err("The directory entry could not be updated (file not found)");
                }
                if first_byte == 0xE5 { lfn_name.clear(); continue; }

                let attr = data[i+11];
                if attr == 0x0F {
                    let mut part = String::new();
                    let positions = [1,3,5,7,9, 14,16,18,20,22,24, 28,30];
                    for &p in positions.iter() {
                        let ch = data[i+p];
                        if ch == 0 || ch == 0xFF { break; }
                        part.push(ch as char);
                    }
                    lfn_name = part + &lfn_name;
                    continue;
                }
                if attr & 0x08 != 0 { lfn_name.clear(); continue; }

                let name = if !lfn_name.is_empty() {
                    let n = lfn_name.clone();
                    lfn_name.clear();
                    n
                } else {
                    parse_short_name(&data[i..i+32])
                };

                if name.eq_ignore_ascii_case(target_name) {
                    // Size (28-31)
                    data[i+28..i+32].copy_from_slice(&new_size.to_le_bytes());
                    // First cluster (Hi: 20-21, Lo: 26-27)
                    let cl_hi = ((new_cluster >> 16) & 0xFFFF) as u16;
                    let cl_lo = (new_cluster & 0xFFFF) as u16;
                    data[i+20..i+22].copy_from_slice(&cl_hi.to_le_bytes());
                    data[i+26..i+28].copy_from_slice(&cl_lo.to_le_bytes());
                    modified = true;
                    break;
                }
            }

            if modified {
                self.write_cluster(dev, cluster, &data)?;
                return Ok(());
            }
        }
        Err("The directory entry could not be updated (file not found)")
    }

    // Real writing!
    pub fn write_file(&self, dev: &mut dyn BlockDevice, entry: &mut DirEntry, data: &[u8]) -> Result<(), &'static str> {
        if entry.is_dir { return Err("Bu bir dizin, dosya degil"); }

        let parent = if entry.parent_cluster >= 2 { entry.parent_cluster } else { self.root_cluster };

        // data safety
        if data.is_empty() {
            entry.size = 0;
            return self.update_dir_entry(dev, parent, &entry.name, 0, entry.first_cluster);
        }

        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let mut current_cluster = entry.first_cluster;
        if current_cluster == 0 {
            current_cluster = self.alloc_cluster(dev)?;
            entry.first_cluster = current_cluster;
        }

        let mut offset = 0;
        let mut prev_cluster = 0;

        while offset < data.len() {
            if current_cluster < 2 || current_cluster >= 0x0FFFFFF8 {
                let new_cluster = self.alloc_cluster(dev)?;
                if prev_cluster != 0 {
                    self.set_fat_entry(dev, prev_cluster, new_cluster)?;
                } else {
                    entry.first_cluster = new_cluster;
                }
                current_cluster = new_cluster;
            }

            let end = core::cmp::min(offset + cluster_size, data.len());
            let mut chunk = vec![0u8; cluster_size];
            chunk[..end - offset].copy_from_slice(&data[offset..end]);
            self.write_cluster(dev, current_cluster, &chunk)?;

            prev_cluster = current_cluster;
            current_cluster = self.next_cluster(dev, current_cluster)?;
            offset += cluster_size;
        }

        if prev_cluster != 0 {
            self.set_fat_entry(dev, prev_cluster, 0x0FFFFFFF)?;
        }

        let mut to_free = current_cluster;
        let mut guard = 0;
        while to_free >= 2 && to_free < 0x0FFFFFF8 {
            let next = self.next_cluster(dev, to_free)?;
            self.set_fat_entry(dev, to_free, 0)?;
            to_free = next;
            guard += 1;
            if guard > 100_000 { break; }
        }

        entry.size = data.len() as u32;
        self.update_dir_entry(dev, parent, &entry.name, entry.size, entry.first_cluster)?;
        Ok(())
    }
}

fn parse_short_name(entry: &[u8]) -> String {
    let mut name = String::new();
    for i in 0..8 {
        let c = entry[i];
        if c == b' ' { break; }
        name.push(c as char);
    }
    let mut ext = String::new();
    for i in 8..11 {
        let c = entry[i];
        if c == b' ' { break; }
        ext.push(c as char);
    }
    if !ext.is_empty() {
        name.push('.');
        name.push_str(&ext);
    }
    name
}
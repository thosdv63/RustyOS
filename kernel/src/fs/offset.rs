// ============================================================
// PartitionDevice: Shows a GPT partition as a standalone disk.
// Adds the partition start (start_lba) to each LBA AND
// DENIES any access exceeding the partition boundary.
// CRITICAL SECURITY: Returns Err if lba >= sectors -> not a single byte can be written OUTSIDE (Windows/Linux/EFI) of the selected partition.
// Because it is owned ('static'), it can be placed in Arc<Mutex<>>; both RPE installation
// and kernel mount use the same device. 
// ============================================================
use crate::fs::BlockDevice;
use crate::drivers::storage::nvme::NvmeBlockDevice;
use crate::drivers::storage::ahci::AhciBlockDevice;
use crate::drivers::storage::ide::IdeBlockDevice;

pub struct PartitionDevice {
    pub kind: u8,       // 0 = NVMe, 1 = SATA (AHCI), 2 = IDE (PATA)
    pub start_lba: u64, // first LBA of partition
    pub sectors: u64,   // partition's sector count
}

impl PartitionDevice {
    pub fn new(kind: u8, start_lba: u64, sectors: u64) -> Self {
        Self { kind, start_lba, sectors }
    }
}

impl BlockDevice for PartitionDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        if lba >= self.sectors {
            return Err("Read outside the partition was denied");
        }
        let abs = self.start_lba + lba;
        match self.kind {
            0 => { let mut d = NvmeBlockDevice; d.read_block(abs, buf) }
            1 => { let mut d = AhciBlockDevice; d.read_block(abs, buf) }
            2 => {
                // Bizim yazdığımız IDE sürücüsünü Primary Master olarak çağırıyoruz
                let mut d = IdeBlockDevice {
                    channel: crate::drivers::storage::ide::IdeChannel::Primary,
                    drive_type: crate::drivers::storage::ide::IdeDriveType::Master,
                };
                d.read_block(abs, buf)
            }
            _ => Err("Unknown disk type (Read denied)"),
        }
    }

    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
        if lba >= self.sectors {
            return Err("Write request outside partition denied"); // ABSOLOUTE
        }
        let abs = self.start_lba + lba;
        match self.kind {
            0 => { let mut d = NvmeBlockDevice; d.write_block(abs, buf) }
            1 => { let mut d = AhciBlockDevice; d.write_block(abs, buf) }
            2 => {
                let mut d = IdeBlockDevice {
                    channel: crate::drivers::storage::ide::IdeChannel::Primary,
                    drive_type: crate::drivers::storage::ide::IdeDriveType::Master,
                };
                d.write_block(abs, buf)
            }
            _ => Err("Unknown disk type (Write denied)"),
        }
    }

    fn block_size(&self) -> u32 {
        match self.kind {
            0 => NvmeBlockDevice.block_size(),
            1 => AhciBlockDevice.block_size(),
            2 => {
                let d = IdeBlockDevice {
                    channel: crate::drivers::storage::ide::IdeChannel::Primary,
                    drive_type: crate::drivers::storage::ide::IdeDriveType::Master,
                };
                d.block_size()
            }
            _ => 512,
        }
    }
}

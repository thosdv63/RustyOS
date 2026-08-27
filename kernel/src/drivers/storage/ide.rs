use crate::drivers::pci::PciDevice;
use crate::drivers::io;
use crate::fs::BlockDevice;
use x86_64::instructions::port::Port;

// Compatibility Mode (legacy port addr)
pub const ATA_PRIMARY_IO: u16 = 0x1F0;
pub const ATA_PRIMARY_CTRL: u16 = 0x3F6;
pub const ATA_SECONDARY_IO: u16 = 0x170;
pub const ATA_SECONDARY_CTRL: u16 = 0x376;

// Register Offsets
pub const ATA_REG_DATA: u16 = 0x00;
pub const ATA_REG_ERROR: u16 = 0x01;
pub const ATA_REG_FEATURES: u16 = 0x01;
pub const ATA_REG_SECCOUNT: u16 = 0x02;
pub const ATA_REG_LBA_LOW: u16 = 0x03;
pub const ATA_REG_LBA_MID: u16 = 0x04;
pub const ATA_REG_LBA_HIGH: u16 = 0x05;
pub const ATA_REG_DRIVE_SELECT: u16 = 0x06;
pub const ATA_REG_COMMAND: u16 = 0x07;
pub const ATA_REG_STATUS: u16 = 0x07;

// Status Register Bit Masks
pub const ATA_SR_BSY: u8 = 0x80;  // Busy
pub const ATA_SR_DRDY: u8 = 0x40; // Drive Ready
pub const ATA_SR_DF: u8 = 0x20;   // Drive Write Fault
pub const ATA_SR_DSC: u8 = 0x10;  // Drive Seek Complete
pub const ATA_SR_DRQ: u8 = 0x08;  // Data Request Ready
pub const ATA_SR_CORR: u8 = 0x04; // Corrected Data
pub const ATA_SR_IDX: u8 = 0x02;  // Index
pub const ATA_SR_ERR: u8 = 0x01;  // Error

// Error Register Bit Masks
pub const ATA_ER_BBK: u8 = 0x80;  // Bad Block
pub const ATA_ER_UNC: u8 = 0x40;  // Uncorrectable Data
pub const ATA_ER_MC: u8 = 0x20;   // Media Changed
pub const ATA_ER_IDNF: u8 = 0x10; // ID Not Found
pub const ATA_ER_MCR: u8 = 0x08;  // Media Change Request
pub const ATA_ER_ABRT: u8 = 0x04; // Command Aborted
pub const ATA_ER_TK0NF: u8 = 0x02;// Track 0 Not Found
pub const ATA_ER_AMNF: u8 = 0x01; // Address Mark Not Found

// ENUMS

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeChannel {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeDriveType {
    Master,
    Slave,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeCommand {
    ReadPio = 0x20,
    ReadPioExt = 0x24,
    WritePio = 0x30,
    WritePioExt = 0x34,
    CacheFlush = 0xE7,
    CacheFlushExt = 0xEA,
    Identify = 0xEC,
    IdentifyPacket = 0xA1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeError {
    DeviceNotFound,
    BusyTimeout,
    DataRequestTimeout,
    DriveFault,
    CommandAborted,
    BufferTooSmall,
    InvalidLba,
}

// STRUCTS

#[derive(Debug, Clone, Copy)]
pub struct IdeChannelRegisters {
    pub io_base: u16,
    pub ctrl_base: u16,
    pub bus_master_base: u16,
}

pub struct IdeDrive {
    pub channel: IdeChannel,
    pub drive_type: IdeDriveType,
    pub regs: IdeChannelRegisters,
    pub exists: bool,
    pub is_atapi: bool,
    pub block_size: u32,
    pub block_count: u64,
    pub model_name: [u8; 40],
}

pub struct IdeController {
    pub primary_master: Option<IdeDrive>,
    pub primary_slave: Option<IdeDrive>,
    pub secondary_master: Option<IdeDrive>,
    pub secondary_slave: Option<IdeDrive>,
}

pub struct IdeBlockDevice {
    pub channel: IdeChannel,
    pub drive_type: IdeDriveType,
}

static mut IDE_CONTROLLER: Option<IdeController> = None;

// Functions

impl IdeDrive {
    pub fn new(channel: IdeChannel, drive_type: IdeDriveType, regs: IdeChannelRegisters) -> Self {
        Self {
            channel,
            drive_type,
            regs,
            exists: false,
            is_atapi: false,
            block_size: 512,
            block_count: 0,
            model_name: [0;40],
        }
    }

    pub unsafe fn delay_400ns(&self) {
        let _ = io::inb(self.regs.ctrl_base);
        let _ = io::inb(self.regs.ctrl_base);
        let _ = io::inb(self.regs.ctrl_base);
        let _ = io::inb(self.regs.ctrl_base);
    }

    pub unsafe fn read_status(&self) -> u8 {
        return io::inb(self.regs.io_base + ATA_REG_STATUS);
    }

    pub unsafe fn read_alt_status(&self) -> u8 {
        return io::inb(self.regs.ctrl_base)
    }

    pub unsafe fn wait_busy(&self) -> Result<(), IdeError> {
        for _ in 0..100_000 {
            let status = self.read_alt_status();
            if (status & ATA_SR_BSY) == 0 {
                return Ok(());
            }
            core::hint::spin_loop()
        }
        Err(IdeError::BusyTimeout)
    }

    pub unsafe fn wait_drq(&self) -> Result<(), IdeError> {
        for _ in 0..100_000 {
            let status = self.read_status();
            if (status & (ATA_SR_ERR | ATA_SR_DF)) != 0 {
                return Err(IdeError::DriveFault);
            }
            if (status & (ATA_SR_DRQ)) != 0 {
                return Ok(());
            }
        }
        Err(IdeError::DataRequestTimeout)
    }

    pub unsafe fn select_drive(&self) -> Result<(), IdeError> {
        self.wait_busy()?;
        let command_byte = match self.drive_type {
            IdeDriveType::Master => 0xA0,
            IdeDriveType::Slave => 0xB0,
        };
        io::outb(self.regs.io_base + ATA_REG_DRIVE_SELECT, command_byte);
        self.delay_400ns();
        self.wait_busy()?;
        Ok(())
    }

    pub unsafe fn identify(&mut self) -> Result<(), IdeError> {
        self.select_drive()?;
        io::outb(self.regs.io_base + ATA_REG_SECCOUNT, 0x00);
        io::outb(self.regs.io_base + ATA_REG_LBA_LOW, 0x00);
        io::outb(self.regs.io_base + ATA_REG_LBA_MID, 0x00);
        io::outb(self.regs.io_base + ATA_REG_LBA_HIGH, 0x00);

        io::outb(self.regs.io_base + ATA_REG_COMMAND, IdeCommand::Identify as u8);
        self.delay_400ns();

        if (self.read_status() == 0x00) {
           return Err(IdeError::DeviceNotFound);
        }

        if self.wait_busy().is_err() {
            let mid = io::inb(self.regs.io_base + ATA_REG_LBA_MID);
            let high = io::inb(self.regs.io_base + ATA_REG_LBA_HIGH);

            if mid == 0x14 && high == 0xEB { // ATAPI
                self.is_atapi = true;
                self.exists = true;
                return Ok(());
            } else {
                return Err(IdeError::DeviceNotFound);
            }
        }

        self.wait_drq()?;
        self.exists = true;

        let mut buffer = [0u16; 256];
        for i in 0..256 {
           buffer[i] = io::inw(self.regs.io_base + ATA_REG_DATA);
        }

        self.block_count = (buffer[61] as u64) << 16 | (buffer[60] as u64);
        let mut byte_idx = 0;
        for i in 27..=46 {
            let bytes = buffer[i].to_be_bytes();
            
            self.model_name[byte_idx] = bytes[0];
            self.model_name[byte_idx + 1] = bytes[1];
            
            byte_idx += 2;
        }

        let lba_low = buffer[60] as u64;
        let lba_high = buffer[61] as u64;
        self.block_count = (lba_high << 16) | lba_low;
        self.block_size = 512;

        Ok(())
    }

    pub unsafe fn read_sectors_pio(&mut self, lba: u64, count: u8, buf: &mut [u8]) -> Result<(), IdeError> {
        let expected_size = (count as usize) * 512;
        if buf.len() < expected_size {
            return Err(IdeError::BufferTooSmall);
        }
        self.wait_busy()?;

        let drive_type_bit = match self.drive_type {
            IdeDriveType::Master => 0,
            IdeDriveType::Slave => 1,
        };
        let drive_select_byte = 0xE0 | (drive_type_bit << 4) | ((lba >> 24) & 0x0F) as u8;
        io::outb(self.regs.io_base + ATA_REG_DRIVE_SELECT, drive_select_byte);
        self.delay_400ns();
        io::outb(self.regs.io_base + ATA_REG_SECCOUNT, count);

        io::outb(self.regs.io_base + ATA_REG_LBA_LOW, (lba & 0xFF) as u8);
        io::outb(self.regs.io_base + ATA_REG_LBA_MID, ((lba >> 8) & 0xFF) as u8);
        io::outb(self.regs.io_base + ATA_REG_LBA_HIGH, ((lba >> 16) & 0xFF) as u8);
        io::outb(self.regs.io_base + ATA_REG_COMMAND, IdeCommand::ReadPio as u8);
        self.delay_400ns();

        let mut byte_offset = 0;
        for _ in 0..count {
            self.wait_drq()?;
            for _ in 0..256 {
                let word = io::inw(self.regs.io_base + ATA_REG_DATA);
                let bytes = word.to_le_bytes();

                buf[byte_offset] = bytes[0];
                buf[byte_offset + 1] = bytes[1];

                byte_offset += 2;
            }
        }

        Ok(())
    }

    pub unsafe fn write_sectors_pio(&mut self, lba: u64, count: u8, buf: &[u8]) -> Result<(), IdeError> {
        let expected_size = (count as usize) * 512;
        if buf.len() < expected_size {
            return Err(IdeError::BufferTooSmall);
        }

        let drive_type_bit = match self.drive_type {
            IdeDriveType::Master => 0,
            IdeDriveType::Slave => 1,
        };

        let drive_select_byte = 0xE0 | (drive_type_bit << 4) | ((lba >> 24) & 0x0F) as u8;
        io::outb(self.regs.io_base + ATA_REG_DRIVE_SELECT, drive_select_byte);
        self.delay_400ns();
        io::outb(self.regs.io_base + ATA_REG_SECCOUNT, count);

        io::outb(self.regs.io_base + ATA_REG_LBA_LOW, (lba & 0xFF) as u8);
        io::outb(self.regs.io_base + ATA_REG_LBA_MID, ((lba >> 8) & 0xFF) as u8);
        io::outb(self.regs.io_base + ATA_REG_LBA_HIGH, ((lba >> 16) & 0xFF) as u8);

        io::outb(self.regs.io_base + ATA_REG_COMMAND, IdeCommand::WritePio as u8);
        self.delay_400ns();

        let mut byte_offset = 0;

        for _ in 0..count {
            self.wait_drq()?;
            for _ in 0..256 {
                let low_byte = buf[byte_offset];
                let high_byte = buf[byte_offset + 1];
                let word = u16::from_le_bytes([low_byte, high_byte]);

                io::outw(self.regs.io_base + ATA_REG_DATA, word);
                byte_offset += 2;
            }
        }

        self.flush_cache()?;

        Ok(())
    }


    pub unsafe fn flush_cache(&self) -> Result<(), IdeError> {
        self.wait_busy()?;
        io::outb(self.regs.io_base + ATA_REG_COMMAND, IdeCommand::CacheFlush as u8);
        self.delay_400ns();
        self.wait_busy()?;
        Ok(())
    }
}

impl IdeController {
    pub fn new() -> Self {
        Self {
            primary_master: None,
            primary_slave: None,
            secondary_master: None,
            secondary_slave: None,
        }
    }

    pub unsafe fn init_pci(&mut self, pci_dev: &PciDevice) -> Result<(), IdeError> {
        let prog_if = pci_dev.prog_if;

        let primary_regs = if (prog_if & 0x01) == 0 { // Legacy and Compatitable
            IdeChannelRegisters {
                io_base: ATA_PRIMARY_IO,
                ctrl_base: ATA_PRIMARY_CTRL,
                bus_master_base: 0,
            }
        } else { // Native PCI
            IdeChannelRegisters {
                io_base: (crate::drivers::pci::read_bar(pci_dev.bus, pci_dev.device, pci_dev.function, 0) & !0x03) as u16,
                ctrl_base: (crate::drivers::pci::read_bar(pci_dev.bus, pci_dev.device, pci_dev.function, 1) & !0x03) as u16,
                bus_master_base: 0,
            }
        };

        let secondary_regs = if (prog_if & 0x04) == 0 { // Legacy and Compatitable
            IdeChannelRegisters {
                io_base: ATA_SECONDARY_IO,
                ctrl_base: ATA_SECONDARY_CTRL,
                bus_master_base: 0,
            }
        } else { // Native PCI
            IdeChannelRegisters {
                io_base: (crate::drivers::pci::read_bar(pci_dev.bus, pci_dev.device, pci_dev.function, 2) & !0x03) as u16,
                ctrl_base: (crate::drivers::pci::read_bar(pci_dev.bus, pci_dev.device, pci_dev.function, 3) & !0x03) as u16,
                bus_master_base: 0,
            }
        };

        let mut pm_drive = IdeDrive::new(IdeChannel::Primary, IdeDriveType::Master, primary_regs);
        let _ = pm_drive.identify(); 
        if pm_drive.exists {
            self.primary_master = Some(pm_drive);
        }

        let mut ps_drive = IdeDrive::new(IdeChannel::Primary, IdeDriveType::Slave, primary_regs);
        let _ = ps_drive.identify();
        if ps_drive.exists {
            self.primary_slave = Some(ps_drive);
        }

        let mut sm_drive = IdeDrive::new(IdeChannel::Secondary, IdeDriveType::Master, secondary_regs);
        let _ = sm_drive.identify();
        if sm_drive.exists {
            self.secondary_master = Some(sm_drive);
        }

        let mut ss_drive = IdeDrive::new(IdeChannel::Secondary, IdeDriveType::Slave, secondary_regs);
        let _ = ss_drive.identify();
        if ss_drive.exists {
            self.secondary_slave = Some(ss_drive);
        }

        Ok(())
    }

    pub unsafe fn init_legacy(&mut self) -> Result<(), IdeError> {
        let primary_regs = IdeChannelRegisters {
            io_base: ATA_PRIMARY_IO,       // 0x1F0
            ctrl_base: ATA_PRIMARY_CTRL,   // 0x3F6
            bus_master_base: 0,
        };

        let secondary_regs = IdeChannelRegisters {
            io_base: ATA_SECONDARY_IO,     // 0x170
            ctrl_base: ATA_SECONDARY_CTRL, // 0x376
            bus_master_base: 0,
        };

        let mut pm_drive = IdeDrive::new(IdeChannel::Primary, IdeDriveType::Master, primary_regs);
        let _ = pm_drive.identify();
        if pm_drive.exists {
            self.primary_master = Some(pm_drive);
        }

        let mut ps_drive = IdeDrive::new(IdeChannel::Primary, IdeDriveType::Slave, primary_regs);
        let _ = ps_drive.identify();
        if ps_drive.exists {
            self.primary_slave = Some(ps_drive);
        }

        let mut sm_drive = IdeDrive::new(IdeChannel::Secondary, IdeDriveType::Master, secondary_regs);
        let _ = sm_drive.identify();
        if sm_drive.exists {
            self.secondary_master = Some(sm_drive);
        }

        let mut ss_drive = IdeDrive::new(IdeChannel::Secondary, IdeDriveType::Slave, secondary_regs);
        let _ = ss_drive.identify();
        if ss_drive.exists {
            self.secondary_slave = Some(ss_drive);
        }

        Ok(())
    }

    pub fn get_drive(&mut self, channel: IdeChannel, drive_type: IdeDriveType) -> Option<&mut IdeDrive> {
        match (channel, drive_type) {
            (IdeChannel::Primary, IdeDriveType::Master) => self.primary_master.as_mut(),
            (IdeChannel::Primary, IdeDriveType::Slave) => self.primary_slave.as_mut(),
            (IdeChannel::Secondary, IdeDriveType::Master) => self.secondary_master.as_mut(),
            (IdeChannel::Secondary, IdeDriveType::Slave) => self.secondary_slave.as_mut(),
        }
    }

}

pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    let mut ide_pci_device: Option<&PciDevice> = None;
    for dev in devices {
        if dev.class == 0x01 && dev.subclass == 0x01 {
            ide_pci_device = Some(dev);
            break;
        }
    }
    let mut controller = IdeController::new();

    if let Some(pci_dev) = ide_pci_device {
        unsafe {
            crate::drivers::pci::enable_bus_master(pci_dev.bus, pci_dev.device, pci_dev.function);
            if controller.init_pci(pci_dev).is_err() {
                return Err("PCI IDE controller could not started");
            }
        }
    } else {
        unsafe {
            if controller.init_legacy().is_err() {
                return Err("Legacy IDE controller could not started");
            }
        }
    }

    unsafe {
        IDE_CONTROLLER = Some(controller);
    }

    Ok(())
}

pub fn dbg(s: &str) {
    unsafe {
        let mut p = Port::<u8>::new(0x3F8);
        for b in s.bytes() { p.write(b); }
    }
}

impl BlockDevice for IdeBlockDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        unsafe {
            if let Some(ref mut controller) = IDE_CONTROLLER {
                if let Some(drive) = controller.get_drive(self.channel, self.drive_type) {
                    match drive.read_sectors_pio(lba, 1, buf) {
                        Ok(()) => Ok(()),
                        Err(_) => Err("IDE driver sector read err"),
                    }
                } else {
                    Err("IDE cannot found")
                }
            } else {
                Err("IDE controller not started yet")
            }
        }
    }

    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
        unsafe {
            if let Some(ref mut controller) = IDE_CONTROLLER {
                if let Some(drive) = controller.get_drive(self.channel, self.drive_type) {
                    match drive.write_sectors_pio(lba, 1, buf) {
                        Ok(()) => Ok(()),
                        Err(_) => Err("IDE driver sector write err"),
                    }
                } else {
                    Err("IDE cannot found")
                }
            } else {
                Err("IDE controller not started yet")
            }
        }
    }

    fn block_size(&self) -> u32 {
        512
    }
}

pub fn info() -> Option<(u32, u64)> {
    unsafe {
        if let Some(ref mut controller) = IDE_CONTROLLER {
            if let Some(drive) = controller.get_drive(IdeChannel::Primary, IdeDriveType::Master) {
                if drive.exists {
                    return Some((drive.block_size, drive.block_count));
                }
            }
            if let Some(drive) = controller.get_drive(IdeChannel::Secondary, IdeDriveType::Master) {
                if drive.exists {
                    return Some((drive.block_size, drive.block_count));
                }
            }
        }
        None
    }
}

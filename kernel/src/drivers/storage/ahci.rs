use crate::drivers::io::{mmio_read32, mmio_write32};
use crate::drivers::pci::{self, PciDevice};
use crate::mm::pfa;
use core::sync::atomic::{fence, Ordering};

// AHCI HBA (Host Bus Adapter) Register Offsets ===
const HBA_CAP: u64 = 0x00;     // Host Capabilities
const HBA_GHC: u64 = 0x04;     // Global Host Control
const HBA_IS: u64 = 0x08;      // Interrupt Status
const HBA_PI: u64 = 0x0C;      // Ports Implemented

// Port Register Offsets (for every port)
// Port N registers: ABAR + 0x100 + N*0x80
const PORT_CLB: u64 = 0x00;    // Command List Base (64-bit)
const PORT_FB: u64 = 0x08;     // FIS Base (64-bit)
const PORT_IS: u64 = 0x10;     // Interrupt Status
const PORT_IE: u64 = 0x14;     // Interrupt Enable
const PORT_CMD: u64 = 0x18;    // Command and Status
const PORT_TFD: u64 = 0x20;    // Task File Data
const PORT_SIG: u64 = 0x24;    // Signature
const PORT_SSTS: u64 = 0x28;   // SATA Status
const PORT_SCTL: u64 = 0x2C;   // SATA Control
const PORT_SERR: u64 = 0x30;   // SATA Error
const PORT_CI: u64 = 0x38;     // Command Issue

// Port CMD bits
const CMD_ST: u32 = 0x0001;    // Start
const CMD_FRE: u32 = 0x0010;   // FIS Receive Enable
const CMD_FR: u32 = 0x4000;    // FIS Receive Running
const CMD_CR: u32 = 0x8000;    // Command List Running

// SATA signatures
const SIG_ATA: u32 = 0x00000101; // SATA disk

// Command Header (32 byte)
#[repr(C)]
#[derive(Clone, Copy)]
struct CmdHeader {
    flags: u16,        // CFL (command FIS length) + flags
    prdtl: u16,        // PRDT number of entries
    prdbc: u32,        // number of bytes transferred
    ctba: u32,         // Command Table base (low)
    ctba_upper: u32,   // Command Table base (hihh)
    _reserved: [u32; 4],
}

// FIS - Register Host to Device (20 byte)
#[repr(C)]
#[derive(Clone, Copy)]
struct FisRegH2D {
    fis_type: u8,      // 0x27
    pmport_c: u8,      // bit 7 = command/control
    command: u8,       // ATA command
    featurel: u8,
    lba0: u8, lba1: u8, lba2: u8,
    device: u8,
    lba3: u8, lba4: u8, lba5: u8,
    featureh: u8,
    countl: u8, counth: u8,
    icc: u8,
    control: u8,
    _reserved: [u8; 4],
}

// PRDT Entry (Physical Region Descriptor)
#[repr(C)]
#[derive(Clone, Copy)]
struct PrdtEntry {
    dba: u32,          // data base (low)
    dba_upper: u32,    // data base (high)
    _reserved: u32,
    dbc: u32,          // byte count (bit 0-21) + interrupt bit (bit 31)
}

/* Command Table 
    0x00: Command FIS (64 byte alan)
    0x40: ATAPI command (16 byte)
    0x80: reserved (48 byte)
    0x80+: PRDT entries */
const CT_FIS_OFFSET: usize = 0x00;
const CT_PRDT_OFFSET: usize = 0x80;

// AHCI Driver
pub struct AhciDevice {
    abar: u64,             // HBA register address (BAR5)
    port: u32,             // port number used
    clb: u64,              // command list base (physical)
    fb: u64,               // FIS base (physical)
    ctba: u64,             // command table base (physical)
    pub block_size: u32,   // blok size (512)
    pub block_count: u64,  // total blocks
}

static mut AHCI: Option<AhciDevice> = None;

impl AhciDevice {
    // Port register address
    unsafe fn port_reg(&self, offset: u64) -> u64 {
        self.abar + 0x100 + (self.port as u64) * 0x80 + offset
    }

    // Stop port
    unsafe fn stop_port(&self) {
        let mut cmd = mmio_read32(self.port_reg(PORT_CMD));
        cmd &= !CMD_ST;
        cmd &= !CMD_FRE;
        mmio_write32(self.port_reg(PORT_CMD), cmd);
        // Wait until CR and FR are cleaned
        let mut spin = 0;
        loop {
            let c = mmio_read32(self.port_reg(PORT_CMD));
            if (c & CMD_CR) == 0 && (c & CMD_FR) == 0 { break; }
            spin += 1;
            if spin > 1_000_000 { break; }
            core::hint::spin_loop();
        }
    }

    // Start port
    unsafe fn start_port(&self) {
        // Make sure CR is clean
        let mut spin = 0;
        while (mmio_read32(self.port_reg(PORT_CMD)) & CMD_CR) != 0 {
            spin += 1;
            if spin > 1_000_000 { break; }
            core::hint::spin_loop();
        }
        let mut cmd = mmio_read32(self.port_reg(PORT_CMD));
        cmd |= CMD_FRE;
        cmd |= CMD_ST;
        mmio_write32(self.port_reg(PORT_CMD), cmd);
    }

    // Send a command (read=true read, false write), waith with polling
    unsafe fn run_command(&mut self, lba: u64, buf_phys: u64, count: u16, write: bool) -> Result<(), &'static str> {
        // Clean interrupt status
        mmio_write32(self.port_reg(PORT_IS), 0xFFFFFFFF);

        // Set Command Header (slot 0)
        let cmd_header = self.clb as *mut CmdHeader;
        let header = CmdHeader {
            // CFL = 5 dword (20 byte / 4), Write bit
            flags: (5 & 0x1F) | if write { 1 << 6 } else { 0 },
            prdtl: 1, // 1 PRDT entry
            prdbc: 0,
            ctba: (self.ctba & 0xFFFF_FFFF) as u32,
            ctba_upper: (self.ctba >> 32) as u32,
            _reserved: [0; 4],
        };
        core::ptr::write_volatile(cmd_header, header);

        // Reset command table
        core::ptr::write_bytes(self.ctba as *mut u8, 0, 256);

        // Fill command FIS (Register H2D)
        let fis = (self.ctba + CT_FIS_OFFSET as u64) as *mut FisRegH2D;
        let cmd_fis = FisRegH2D {
            fis_type: 0x27,           // Register H2D
            pmport_c: 1 << 7,         // command finished
            command: if write { 0x35 } else { 0x25 }, // WRITE DMA EXT / READ DMA EXT
            featurel: 0,
            lba0: (lba & 0xFF) as u8,
            lba1: ((lba >> 8) & 0xFF) as u8,
            lba2: ((lba >> 16) & 0xFF) as u8,
            device: 1 << 6,           // LBA mode
            lba3: ((lba >> 24) & 0xFF) as u8,
            lba4: ((lba >> 32) & 0xFF) as u8,
            lba5: ((lba >> 40) & 0xFF) as u8,
            featureh: 0,
            countl: (count & 0xFF) as u8,
            counth: ((count >> 8) & 0xFF) as u8,
            icc: 0,
            control: 0,
            _reserved: [0; 4],
        };
        core::ptr::write_volatile(fis, cmd_fis);

        // Fill PRDT entry (data addr + size)
        let prdt = (self.ctba + CT_PRDT_OFFSET as u64) as *mut PrdtEntry;
        let byte_count = (count as u32) * self.block_size - 1; // 0-based
        let prdt_entry = PrdtEntry {
            dba: (buf_phys & 0xFFFF_FFFF) as u32,
            dba_upper: (buf_phys >> 32) as u32,
            _reserved: 0,
            dbc: byte_count & 0x3FFFFF, // bit 0-21
        };
        core::ptr::write_volatile(prdt, prdt_entry);

        fence(Ordering::SeqCst);

        // Wait for TFD to be ready (BSY and DRQ need to be cleared)
        let mut spin = 0;
        while (mmio_read32(self.port_reg(PORT_TFD)) & 0x88) != 0 {
            spin += 1;
            if spin > 1_000_000 { return Err("port busy"); }
            core::hint::spin_loop();
        }

        // Publish command (slot 0)
        mmio_write32(self.port_reg(PORT_CI), 1);

        // Wait for completion (until the CI bit is cleared)
        spin = 0;
        loop {
            if (mmio_read32(self.port_reg(PORT_CI)) & 1) == 0 { break; }
            // error checking (ERR bit in TFD)
            if (mmio_read32(self.port_reg(PORT_TFD)) & 0x01) != 0 {
                return Err("AHCI command error (TFD ERR)");
            }
            spin += 1;
            if spin > 100_000_000 { return Err("AHCI timeout"); }
            core::hint::spin_loop();
        }

        Ok(())
    }
}

// Setup
pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    // Find AHCI controller in PCI (class 0x01, subclass 0x06)
    let ahci_dev = devices.iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x06)
        .ok_or("AHCI controller not found")?;

    // Enable bus mastering
    pci::enable_bus_master(ahci_dev.bus, ahci_dev.device, ahci_dev.function);

    // ABAR = BAR5 (AHCI register address)
    let abar = pci::read_bar(ahci_dev.bus, ahci_dev.device, ahci_dev.function, 5);
    if abar == 0 {
        return Err("ABAR (BAR5) is zero");
    }

    // Map ABAR
    for i in 0..2u64 {
        let addr = abar + i * 0x1000;
        if crate::mm::ptm::translate(addr).is_none() {
            crate::mm::ptm::map_page(addr, addr, true)?;
        }
    }

    unsafe {
        // Enable AHCI mode (GHC.AE = bit 31)
        let ghc = mmio_read32(abar + HBA_GHC);
        mmio_write32(abar + HBA_GHC, ghc | (1 << 31));

        // Get ports (PI register)
        let pi = mmio_read32(abar + HBA_PI);

        // Find the first port that has a disk
        let mut found_port: i32 = -1;
        for port in 0..32u32 {
            if (pi & (1 << port)) == 0 { continue; }

            let port_base = abar + 0x100 + (port as u64) * 0x80;
            let ssts = mmio_read32(port_base + PORT_SSTS);
            let det = ssts & 0x0F;        // device detection
            let ipm = (ssts >> 8) & 0x0F; // interface power management

            // det=3 (device available + communication), ipm=1 (active)
            if det == 3 && ipm == 1 {
                let sig = mmio_read32(port_base + PORT_SIG);
                if sig == SIG_ATA {
                    found_port = port as i32;
                    break;
                }
            }
        }

        if found_port < 0 {
            return Err("SATA disk couldn't found");
        }
        let port = found_port as u32;

        // Get frames for command list, FIS, command table
        let clb = pfa::alloc_frame().ok_or("no CLB frame")?;
        let fb = pfa::alloc_frame().ok_or("no FB frame")?;
        let ctba = pfa::alloc_frame().ok_or("no CT frame")?;
        core::ptr::write_bytes(clb as *mut u8, 0, 4096);
        core::ptr::write_bytes(fb as *mut u8, 0, 4096);
        core::ptr::write_bytes(ctba as *mut u8, 0, 4096);

        let mut dev = AhciDevice {
            abar, port, clb, fb, ctba,
            block_size: 512,
            block_count: 0,
        };

        // Stop port, set addresses, start
        dev.stop_port();
        mmio_write32(dev.port_reg(PORT_CLB), (clb & 0xFFFF_FFFF) as u32);
        mmio_write32(dev.port_reg(PORT_CLB) + 4, (clb >> 32) as u32);
        mmio_write32(dev.port_reg(PORT_FB), (fb & 0xFFFF_FFFF) as u32);
        mmio_write32(dev.port_reg(PORT_FB) + 4, (fb >> 32) as u32);
        // Clear SATA errors
        mmio_write32(dev.port_reg(PORT_SERR), 0xFFFFFFFF);
        dev.start_port();

        // Find out the number of blocks with the IDENTIFY DEVICE
        let identify_buf = pfa::alloc_frame().ok_or("no identify frame")?;
        core::ptr::write_bytes(identify_buf as *mut u8, 0, 4096);

        // Special command for IDENTIFY (0xEC), data reading
        mmio_write32(dev.port_reg(PORT_IS), 0xFFFFFFFF);
        let cmd_header = dev.clb as *mut CmdHeader;
        core::ptr::write_volatile(cmd_header, CmdHeader {
            flags: 5 & 0x1F, // read
            prdtl: 1,
            prdbc: 0,
            ctba: (dev.ctba & 0xFFFF_FFFF) as u32,
            ctba_upper: (dev.ctba >> 32) as u32,
            _reserved: [0; 4],
        });
        core::ptr::write_bytes(dev.ctba as *mut u8, 0, 256);
        let fis = (dev.ctba + CT_FIS_OFFSET as u64) as *mut FisRegH2D;
        core::ptr::write_volatile(fis, FisRegH2D {
            fis_type: 0x27, pmport_c: 1 << 7,
            command: 0xEC, // IDENTIFY DEVICE
            featurel: 0, lba0: 0, lba1: 0, lba2: 0, device: 0,
            lba3: 0, lba4: 0, lba5: 0, featureh: 0,
            countl: 0, counth: 0, icc: 0, control: 0, _reserved: [0; 4],
        });
        let prdt = (dev.ctba + CT_PRDT_OFFSET as u64) as *mut PrdtEntry;
        core::ptr::write_volatile(prdt, PrdtEntry {
            dba: (identify_buf & 0xFFFF_FFFF) as u32,
            dba_upper: (identify_buf >> 32) as u32,
            _reserved: 0,
            dbc: (512 - 1) & 0x3FFFFF,
        });
        fence(Ordering::SeqCst);
        // wait + publish
        let mut spin = 0;
        while (mmio_read32(dev.port_reg(PORT_TFD)) & 0x88) != 0 {
            spin += 1; if spin > 1_000_000 { break; } core::hint::spin_loop();
        }
        mmio_write32(dev.port_reg(PORT_CI), 1);
        spin = 0;
        while (mmio_read32(dev.port_reg(PORT_CI)) & 1) != 0 {
            spin += 1; if spin > 100_000_000 { return Err("identify timeout"); }
            core::hint::spin_loop();
        }

        // IDENTIFY result: number of blocks (LBA48) in word 100-103 (offset 200 bytes)
        let lba48 = core::ptr::read_volatile((identify_buf + 200) as *const u64);
        dev.block_count = lba48;
        pfa::free_frame(identify_buf);

        AHCI = Some(dev);
    }

    Ok(())
}

// Get disk info
pub fn info() -> Option<(u32, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        AHCI.as_ref().map(|d| (d.block_size, d.block_count))
    }
}

// BlockDevice interface (same with NVMe) ===
pub fn read_block(lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = AHCI.as_mut().ok_or("AHCI could not be started")?;
        let bs = dev.block_size as usize;
        if buf.len() < bs { return Err("buffer is tiny"); }

        let dma = pfa::alloc_frame().ok_or("no dma frame")?;
        dev.run_command(lba, dma, 1, false)?;
        core::ptr::copy(dma as *const u8, buf.as_mut_ptr(), bs);
        pfa::free_frame(dma);
        Ok(())
    }
}

pub fn write_block(lba: u64, buf: &[u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = AHCI.as_mut().ok_or("AHCI could not be started")?;
        let bs = dev.block_size as usize;
        if buf.len() < bs { return Err("buffer is tiny"); }

        let dma = pfa::alloc_frame().ok_or("no dma frame")?;
        core::ptr::copy(buf.as_ptr(), dma as *mut u8, bs);
        dev.run_command(lba, dma, 1, true)?;
        pfa::free_frame(dma);
        Ok(())
    }
}

pub struct AhciBlockDevice;

impl crate::fs::BlockDevice for AhciBlockDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        read_block(lba, buf)
    }
    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
        write_block(lba, buf)
    }
    fn block_size(&self) -> u32 {
        info().map(|(bs, _)| bs).unwrap_or(512)
    }
}
use crate::drivers::io::{mmio_read32, mmio_write32};
use crate::drivers::pci::{self, PciDevice};
use crate::mm::pfa;
use crate::mm::pfa::HHDM_OFFSET;
use core::sync::atomic::{fence, Ordering};

const HBA_CAP: u64 = 0x00;     
const HBA_GHC: u64 = 0x04;     
const HBA_IS: u64 = 0x08;      
const HBA_PI: u64 = 0x0C;    

const PORT_CLB: u64 = 0x00;   
const PORT_FB: u64 = 0x08;     
const PORT_IS: u64 = 0x10;    
const PORT_IE: u64 = 0x14;     
const PORT_CMD: u64 = 0x18;    
const PORT_TFD: u64 = 0x20;  
const PORT_SIG: u64 = 0x24;  
const PORT_SSTS: u64 = 0x28;   
const PORT_SCTL: u64 = 0x2C;   
const PORT_SERR: u64 = 0x30;  
const PORT_CI: u64 = 0x38;    

const CMD_ST: u32 = 0x0001;   
const CMD_FRE: u32 = 0x0010;  
const CMD_FR: u32 = 0x4000;   
const CMD_CR: u32 = 0x8000;    

const SIG_ATA: u32 = 0x00000101; 

#[repr(C)]
#[derive(Clone, Copy)]
struct CmdHeader {
    flags: u16,       
    prdtl: u16,       
    prdbc: u32,        
    ctba: u32,         
    ctba_upper: u32, 
    _reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FisRegH2D {
    fis_type: u8,
    pmport_c: u8,
    command: u8,
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

#[repr(C)]
#[derive(Clone, Copy)]
struct PrdtEntry {
    dba: u32,       
    dba_upper: u32,    
    _reserved: u32,
    dbc: u32,          
}

const CT_FIS_OFFSET: usize = 0x00;
const CT_PRDT_OFFSET: usize = 0x80;

pub struct AhciDevice {
    abar: u64,             
    port: u32,           
    clb: u64,            
    fb: u64,              
    ctba: u64,             
    pub block_size: u32,  
    pub block_count: u64, 
}

static mut AHCI: Option<AhciDevice> = None;

impl AhciDevice {
    unsafe fn port_reg(&self, offset: u64) -> u64 {
        self.abar + 0x100 + (self.port as u64) * 0x80 + offset
    }

    unsafe fn stop_port(&self) {
        let mut cmd = mmio_read32(self.port_reg(PORT_CMD));
        cmd &= !CMD_ST;
        cmd &= !CMD_FRE;
        mmio_write32(self.port_reg(PORT_CMD), cmd);
        let mut spin = 0;
        loop {
            let c = mmio_read32(self.port_reg(PORT_CMD));
            if (c & CMD_CR) == 0 && (c & CMD_FR) == 0 { break; }
            spin += 1;
            if spin > 1_000_000 { break; }
            core::hint::spin_loop();
        }
    }

    unsafe fn start_port(&self) {
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

    unsafe fn run_command(&mut self, lba: u64, pages: &[u64], count: u16, write: bool) -> Result<(), &'static str> {
        if count == 0 {
            return Ok(());
        }
        mmio_write32(self.port_reg(PORT_IS), 0xFFFFFFFF);

        let cmd_header = (self.clb + HHDM_OFFSET) as *mut CmdHeader;
        let header = CmdHeader {
            flags: (5 & 0x1F) | if write { 1 << 6 } else { 0 },
            prdtl: pages.len() as u16,
            prdbc: 0,
            ctba: (self.ctba & 0xFFFF_FFFF) as u32,
            ctba_upper: (self.ctba >> 32) as u32,
            _reserved: [0; 4],
        };
        core::ptr::write_volatile(cmd_header, header);

        core::ptr::write_bytes((self.ctba + HHDM_OFFSET) as *mut u8, 0, 256);

        let fis = (self.ctba + HHDM_OFFSET + CT_FIS_OFFSET as u64) as *mut FisRegH2D;
        let cmd_fis = FisRegH2D {
            fis_type: 0x27,          
            pmport_c: 1 << 7,        
            command: if write { 0x35 } else { 0x25 }, 
            featurel: 0,
            lba0: (lba & 0xFF) as u8,
            lba1: ((lba >> 8) & 0xFF) as u8,
            lba2: ((lba >> 16) & 0xFF) as u8,
            device: 1 << 6,
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

        let prdt_ptr = (self.ctba + HHDM_OFFSET + CT_PRDT_OFFSET as u64) as *mut PrdtEntry;
        let mut remaining_bytes = (count as u32) * self.block_size;
        
        for (i, &page_phys) in pages.iter().enumerate() {
            let bytes_for_this_page = core::cmp::min(remaining_bytes, 4096);
            let prdt_entry = PrdtEntry {
                dba: (page_phys & 0xFFFF_FFFF) as u32,
                dba_upper: (page_phys >> 32) as u32,
                _reserved: 0,
                dbc: (bytes_for_this_page - 1) & 0x3FFFFF,
            };
            core::ptr::write_volatile(prdt_ptr.add(i), prdt_entry);
            remaining_bytes -= bytes_for_this_page;
        }

        fence(Ordering::SeqCst);

        let mut spin = 0;
        while (mmio_read32(self.port_reg(PORT_TFD)) & 0x88) != 0 {
            spin += 1;
            if spin > 1_000_000 { return Err("port busy"); }
            core::hint::spin_loop();
        }

        mmio_write32(self.port_reg(PORT_CI), 1);

        spin = 0;
        loop {
            if (mmio_read32(self.port_reg(PORT_CI)) & 1) == 0 { break; }
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

pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    let ahci_dev = devices.iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x06)
        .ok_or("AHCI controller not found")?;

    pci::enable_bus_master(ahci_dev.bus, ahci_dev.device, ahci_dev.function);

    let abar = pci::read_bar(ahci_dev.bus, ahci_dev.device, ahci_dev.function, 5);
    if abar == 0 {
        return Err("ABAR (BAR5) is zero");
    }

    let mut ptm = crate::mm::vmm::PageTableManager::active();
    for i in 0..2u64 {
        let addr = abar + i * 0x1000;
        if ptm.translate(addr).is_none() {
            ptm.map(addr, addr, 4096, true, false, true, false);
        }
    }

    unsafe {
        let ghc = mmio_read32(abar + HBA_GHC);
        mmio_write32(abar + HBA_GHC, ghc | (1 << 31));

        let pi = mmio_read32(abar + HBA_PI);

        let mut found_port: i32 = -1;
        for port in 0..32u32 {
            if (pi & (1 << port)) == 0 { continue; }

            let port_base = abar + 0x100 + (port as u64) * 0x80;
            let ssts = mmio_read32(port_base + PORT_SSTS);
            let det = ssts & 0x0F;
            let ipm = (ssts >> 8) & 0x0F;

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

        let clb = pfa::alloc_page().ok_or("no CLB page")?;
        let fb = pfa::alloc_page().ok_or("no FB page")?;
        let ctba = pfa::alloc_page().ok_or("no CT page")?;
        core::ptr::write_bytes((clb + HHDM_OFFSET) as *mut u8, 0, 4096);
        core::ptr::write_bytes((fb + HHDM_OFFSET) as *mut u8, 0, 4096);
        core::ptr::write_bytes((ctba + HHDM_OFFSET) as *mut u8, 0, 4096);

        let mut dev = AhciDevice {
            abar, port, clb, fb, ctba,
            block_size: 512,
            block_count: 0,
        };

        dev.stop_port();
        mmio_write32(dev.port_reg(PORT_CLB), (clb & 0xFFFF_FFFF) as u32);
        mmio_write32(dev.port_reg(PORT_CLB) + 4, (clb >> 32) as u32);
        mmio_write32(dev.port_reg(PORT_FB), (fb & 0xFFFF_FFFF) as u32);
        mmio_write32(dev.port_reg(PORT_FB) + 4, (fb >> 32) as u32);
        mmio_write32(dev.port_reg(PORT_SERR), 0xFFFFFFFF);
        dev.start_port();

        let identify_buf = pfa::alloc_page().ok_or("no identify page")?;
        core::ptr::write_bytes((identify_buf + HHDM_OFFSET) as *mut u8, 0, 4096);

        mmio_write32(dev.port_reg(PORT_IS), 0xFFFFFFFF);
        let cmd_header = (dev.clb + HHDM_OFFSET) as *mut CmdHeader;
        core::ptr::write_volatile(cmd_header, CmdHeader {
            flags: 5 & 0x1F,
            prdtl: 1,
            prdbc: 0,
            ctba: (dev.ctba & 0xFFFF_FFFF) as u32,
            ctba_upper: (dev.ctba >> 32) as u32,
            _reserved: [0; 4],
        });
        core::ptr::write_bytes((dev.ctba + HHDM_OFFSET) as *mut u8, 0, 256);
        let fis = (dev.ctba + HHDM_OFFSET + CT_FIS_OFFSET as u64) as *mut FisRegH2D;
        core::ptr::write_volatile(fis, FisRegH2D {
            fis_type: 0x27, pmport_c: 1 << 7,
            command: 0xEC,
            featurel: 0, lba0: 0, lba1: 0, lba2: 0, device: 0,
            lba3: 0, lba4: 0, lba5: 0, featureh: 0,
            countl: 0, counth: 0, icc: 0, control: 0, _reserved: [0; 4],
        });
        let prdt = (dev.ctba + HHDM_OFFSET + CT_PRDT_OFFSET as u64) as *mut PrdtEntry;
        core::ptr::write_volatile(prdt, PrdtEntry {
            dba: (identify_buf & 0xFFFF_FFFF) as u32,
            dba_upper: (identify_buf >> 32) as u32,
            _reserved: 0,
            dbc: (512 - 1) & 0x3FFFFF,
        });
        fence(Ordering::SeqCst);
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

        let lba48 = core::ptr::read_volatile((identify_buf + HHDM_OFFSET + 200) as *const u64);
        dev.block_count = lba48;
        pfa::free_page(identify_buf);

        AHCI = Some(dev);
    }

    Ok(())
}

pub fn info() -> Option<(u32, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        AHCI.as_ref().map(|d| (d.block_size, d.block_count))
    }
}

pub fn read_block(lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = AHCI.as_mut().ok_or("AHCI could not be started")?;
        let bs = dev.block_size as usize;

        if buf.len() % bs != 0 { return Err("buffer not aligned to block size"); }
        if buf.len() == 0 { return Ok(()); }

        let count = (buf.len() / bs) as u16;              
        let page_count = (buf.len() + 4095) / 4096;       
        if page_count > 512 { return Err("transfer too large"); }

        let mut pages = [0u64; 512];
        for i in 0..page_count {
            if let Some(frame) = pfa::alloc_page() {
                pages[i] = frame;
            } else {
                for j in 0..i {
                    pfa::free_page(pages[j]);
                }
                return Err("no dma frame");
            }
        }

        let result = dev.run_command(lba, &pages[0..page_count], count, false);
        if result.is_ok() {
            let mut remaining = buf.len();
            let mut offset = 0;
            for i in 0..page_count {
                let copy_len = core::cmp::min(remaining, 4096);
                core::ptr::copy(
                    (pages[i] + HHDM_OFFSET) as *const u8,
                    buf.as_mut_ptr().add(offset),
                    copy_len,
                );
                offset += copy_len;
                remaining -= copy_len;
            }
        }

        for i in 0..page_count {
            pfa::free_page(pages[i]);
        }

        result
    }
}

pub fn write_block(lba: u64, buf: &[u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = AHCI.as_mut().ok_or("AHCI could not be started")?;
        let bs = dev.block_size as usize;

        if buf.len() % bs != 0 { return Err("buffer not aligned to block size"); }
        if buf.len() == 0 { return Ok(()); }

        let count = (buf.len() / bs) as u16;
        let page_count = (buf.len() + 4095) / 4096;
        if page_count > 512 { return Err("transfer too large"); }

        let mut pages = [0u64; 512];
        for i in 0..page_count {
            if let Some(frame) = pfa::alloc_page() {
                pages[i] = frame;
            } else {
                for j in 0..i {
                    pfa::free_page(pages[j]);
                }
                return Err("no dma frame");
            }
        }

        let mut remaining = buf.len();
        let mut offset = 0;
        for i in 0..page_count {
            let copy_len = core::cmp::min(remaining, 4096);
            core::ptr::copy(
                buf.as_ptr().add(offset),
                (pages[i] + HHDM_OFFSET) as *mut u8,
                copy_len,
            );
            offset += copy_len;
            remaining -= copy_len;
        }

        let result = dev.run_command(lba, &pages[0..page_count], count, true);
        for i in 0..page_count {
            pfa::free_page(pages[i]);
        }

        result
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

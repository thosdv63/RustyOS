use crate::drivers::io::{mmio_read32, mmio_write32, mmio_read64, mmio_write64};
use crate::drivers::pci::{self, PciDevice};
use crate::mm::pfa;
use crate::mm::pfa::HHDM_OFFSET;
use core::sync::atomic::{fence, Ordering};

const REG_CAP: u64 = 0x00;
const REG_CC: u64 = 0x14;
const REG_CSTS: u64 = 0x1C;
const REG_AQA: u64 = 0x24;
const REG_ASQ: u64 = 0x28;
const REG_ACQ: u64 = 0x30;

const QUEUE_SIZE: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
struct SqEntry {
    opcode: u8,        
    flags: u8,
    command_id: u16,  
    nsid: u32,         
    _reserved: u64,
    metadata: u64,
    prp1: u64,       
    prp2: u64,       
    cdw10: u32,       
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
}

impl SqEntry {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CqEntry {
    result: u32,       
    _reserved: u32,
    sq_head: u16,     
    sq_id: u16,        
    command_id: u16,  
    status: u16,       
}

pub struct NvmeDevice {
    bar0: u64,              
    doorbell_stride: u64,

    admin_sq: u64,          
    admin_cq: u64,          
    admin_sq_tail: usize,
    admin_cq_head: usize,
    admin_phase: bool,      

    io_sq: u64,
    io_cq: u64,
    io_sq_tail: usize,
    io_cq_head: usize,
    io_phase: bool,

    command_id: u16,        

    pub block_size: u32,    
    pub block_count: u64,   
}

static mut NVME: Option<NvmeDevice> = None;

impl NvmeDevice {
    unsafe fn sq_doorbell(&self, qid: u64) -> u64 {
        self.bar0 + 0x1000 + (2 * qid) * self.doorbell_stride
    }
    unsafe fn cq_doorbell(&self, qid: u64) -> u64 {
        self.bar0 + 0x1000 + (2 * qid + 1) * self.doorbell_stride
    }

    unsafe fn admin_command(&mut self, mut cmd: SqEntry) -> Result<u32, &'static str> {
        self.command_id = self.command_id.wrapping_add(1);
        cmd.command_id = self.command_id;

        let sq = (self.admin_sq + HHDM_OFFSET) as *mut SqEntry;
        core::ptr::write_volatile(sq.add(self.admin_sq_tail), cmd);

        self.admin_sq_tail = (self.admin_sq_tail + 1) % QUEUE_SIZE;
        fence(Ordering::SeqCst);
        mmio_write32(self.sq_doorbell(0), self.admin_sq_tail as u32);

        let cq_db_addr = self.cq_doorbell(0);
        
        Self::wait_completion(
            self.admin_cq, 
            cq_db_addr, 
            &mut self.admin_cq_head, 
            &mut self.admin_phase
        )
    }

    unsafe fn setup_prp(&mut self, pages: &[u64]) -> Result<(u64, u64, Option<u64>), &'static str> {
        if pages.len() > 513 {
            return Err("transfer too large");
        }

        match pages.len() {
            0 => Err("empty page list"),

            1 => { 
                let prp1 = pages[0];
                let prp2 = 0;
                Ok((prp1, prp2, None))
            }

            2 => { 
                let prp1 = pages[0];
                let prp2 = pages[1];
                Ok((prp1, prp2, None))
            }

            _ => { 
                let prp_list_frame = pfa::alloc_page().ok_or("no frame for prp list")?;
                core::ptr::write_bytes((prp_list_frame + HHDM_OFFSET) as *mut u8, 0, 4096); 
                let prp_list_ptr = (prp_list_frame + HHDM_OFFSET) as *mut u64;

                for i in 1..pages.len() {
                    core::ptr::write_volatile(prp_list_ptr.add(i - 1), pages[i]);
                }

                let prp1 = pages[0];
                let prp2 = prp_list_frame;

                Ok((prp1, prp2, Some(prp_list_frame)))
            }
        }
    }

    unsafe fn io_command(&mut self, mut cmd: SqEntry) -> Result<u32, &'static str> {
        self.command_id = self.command_id.wrapping_add(1);
        cmd.command_id = self.command_id;

        let sq = (self.io_sq + HHDM_OFFSET) as *mut SqEntry;
        core::ptr::write_volatile(sq.add(self.io_sq_tail), cmd);
        self.io_sq_tail = (self.io_sq_tail + 1) % QUEUE_SIZE;

        fence(Ordering::SeqCst);
        mmio_write32(self.sq_doorbell(1), self.io_sq_tail as u32);

        let cq_db_addr = self.cq_doorbell(1);
        
        Self::wait_completion(
            self.io_cq, 
            cq_db_addr, 
            &mut self.io_cq_head, 
            &mut self.io_phase
        )
    }

    unsafe fn wait_completion(
        cq_addr: u64,
        cq_db_addr: u64, 
        cq_head: &mut usize,
        phase: &mut bool,
    ) -> Result<u32, &'static str> {
        let cq = (cq_addr + HHDM_OFFSET) as *const CqEntry;
        let mut spin = 0u64;
        loop {
            let entry = core::ptr::read_volatile(cq.add(*cq_head));
            let entry_phase = (entry.status & 1) == 1;

            if entry_phase == *phase {
                let status_code = (entry.status >> 1) & 0x7FF;

                *cq_head = (*cq_head + 1) % QUEUE_SIZE;
                if *cq_head == 0 {
                    *phase = !*phase; 
                }

                mmio_write32(cq_db_addr, *cq_head as u32);

                if status_code != 0 {
                    return Err("NVMe command error");
                }
                return Ok(entry.result);
            }

            spin += 1;
            if spin > 100_000_000 {
                return Err("NVMe timeout");
            }
            core::hint::spin_loop();
        }
    }

    pub unsafe fn read_block(&mut self, lba: u64, pages: &[u64], block_count: u16) -> Result<(), &'static str> {
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x02; 
        cmd.nsid = 1;

        let (prp1, prp2, prp_list_frame) = self.setup_prp(pages)?;
        cmd.prp1 = prp1;
        cmd.prp2 = prp2;

        cmd.cdw10 = (lba & 0xFFFF_FFFF) as u32;       
        cmd.cdw11 = (lba >> 32) as u32;               
        cmd.cdw12 = (block_count - 1) as u32;          

        let result = self.io_command(cmd);

        if let Some(frame) = prp_list_frame {
            pfa::free_page(frame);
        }

        result.map(|_| ())
    }

    pub unsafe fn write_block(&mut self, lba: u64, pages: &[u64], block_count: u16) -> Result<(), &'static str> {
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x01; 
        cmd.nsid = 1;
        
        let (prp1, prp2, prp_list_frame) = self.setup_prp(pages)?;
        cmd.prp1 = prp1;
        cmd.prp2 = prp2;
        
        cmd.cdw10 = (lba & 0xFFFF_FFFF) as u32;
        cmd.cdw11 = (lba >> 32) as u32;
        cmd.cdw12 = (block_count - 1) as u32;
        
        let result = self.io_command(cmd);

        if let Some(frame) = prp_list_frame {
            pfa::free_page(frame);
        }

        result.map(|_| ())
    }
}

pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    let nvme_dev = devices.iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x08)
        .ok_or("NVMe controller could not found")?;

    pci::enable_bus_master(nvme_dev.bus, nvme_dev.device, nvme_dev.function);

    let bar0 = pci::read_bar(nvme_dev.bus, nvme_dev.device, nvme_dev.function, 0);
    if bar0 == 0 {
        return Err("BAR0 is zero");
    }

    let mut ptm = crate::mm::vmm::PageTableManager::active();
    for i in 0..4u64 {
        let addr = bar0 + i * 0x1000;
        if ptm.translate(addr).is_none() {
            ptm.map(addr, addr, 4096, true, false, true, false);
        }
    }

    unsafe {
        let cap = mmio_read64(bar0 + REG_CAP);
        let doorbell_stride = 1u64 << (2 + ((cap >> 32) & 0xF)); 

        let cc = mmio_read32(bar0 + REG_CC);
        mmio_write32(bar0 + REG_CC, cc & !1);
        let mut spin = 0;
        while (mmio_read32(bar0 + REG_CSTS) & 1) != 0 {
            spin += 1;
            if spin > 100_000_000 { return Err("reset timeout"); }
            core::hint::spin_loop();
        }

        let admin_sq = pfa::alloc_page().ok_or("no admin SQ frame")?;
        let admin_cq = pfa::alloc_page().ok_or("no admin CQ frame")?;

        core::ptr::write_bytes((admin_sq + HHDM_OFFSET) as *mut u8, 0, 4096);
        core::ptr::write_bytes((admin_cq + HHDM_OFFSET) as *mut u8, 0, 4096);

        let aqa = ((QUEUE_SIZE as u32 - 1) << 16) | (QUEUE_SIZE as u32 - 1);
        mmio_write32(bar0 + REG_AQA, aqa);
        mmio_write64(bar0 + REG_ASQ, admin_sq);
        mmio_write64(bar0 + REG_ACQ, admin_cq);

        let cc_new = (6 << 16) | (4 << 20) | 1; 
        mmio_write32(bar0 + REG_CC, cc_new);
        spin = 0;
        while (mmio_read32(bar0 + REG_CSTS) & 1) == 0 {
            spin += 1;
            if spin > 100_000_000 { return Err("enable timeout"); }
            core::hint::spin_loop();
        }

        let mut dev = NvmeDevice {
            bar0,
            doorbell_stride,
            admin_sq, admin_cq,
            admin_sq_tail: 0, admin_cq_head: 0, admin_phase: true,
            io_sq: 0, io_cq: 0,
            io_sq_tail: 0, io_cq_head: 0, io_phase: true,
            command_id: 0,
            block_size: 512,
            block_count: 0,
        };

        let identify_buf = pfa::alloc_page().ok_or("no identify frame")?;
        core::ptr::write_bytes((identify_buf + HHDM_OFFSET) as *mut u8, 0, 4096);

        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x06; 
        cmd.nsid = 1;       
        cmd.prp1 = identify_buf;
        cmd.cdw10 = 0x00;   
        dev.admin_command(cmd)?;

        let nsze = core::ptr::read_volatile((identify_buf + HHDM_OFFSET) as *const u64);
        let flbas = core::ptr::read_volatile((identify_buf + HHDM_OFFSET + 26) as *const u8);
        let lba_format_index = (flbas & 0xF) as usize;
        let lbaf: u32 = core::ptr::read_volatile((identify_buf + HHDM_OFFSET + 128 + (lba_format_index as u64) * 4) as *const u32);
        let lbads = (lbaf >> 16) & 0xFF;
        dev.block_size = 1u32 << lbads;
        dev.block_count = nsze;

        pfa::free_page(identify_buf);

        let io_cq = pfa::alloc_page().ok_or("no io CQ frame")?;
        core::ptr::write_bytes((io_cq + HHDM_OFFSET) as *mut u8, 0, 4096);
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x05;
        cmd.prp1 = io_cq;
        cmd.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; 
        cmd.cdw11 = 1;
        dev.admin_command(cmd)?;
        dev.io_cq = io_cq;

        let io_sq = pfa::alloc_page().ok_or("no io SQ frame")?;
        core::ptr::write_bytes((io_sq + HHDM_OFFSET) as *mut u8, 0, 4096);
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x01;
        cmd.prp1 = io_sq;
        cmd.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; 
        cmd.cdw11 = (1 << 16) | 1; 
        dev.admin_command(cmd)?;
        dev.io_sq = io_sq;

        NVME = Some(dev);
    }

    Ok(())
}

pub fn info() -> Option<(u32, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        NVME.as_ref().map(|d| (d.block_size, d.block_count))
    }
}

pub fn read_block(lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = NVME.as_mut().ok_or("NVMe didn't started")?;
        let bs = dev.block_size as usize;
        
        if buf.len() % bs != 0 { return Err("buffer not aligned to block size"); }
        if buf.len() == 0 { return Ok(()); }

        let block_count = (buf.len() / bs) as u16;
        let page_count = (buf.len() + 4095) / 4096;

        if page_count > 513 { return Err("transfer too large for PRP list"); }

        let mut pages = [0u64; 513];
        
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

        let result = dev.read_block(lba, &pages[0..page_count], block_count);

        if result.is_ok() {
            let mut remaining = buf.len();
            let mut offset = 0;
            for i in 0..page_count {
                let copy_len = core::cmp::min(remaining, 4096);
                core::ptr::copy(
                    (pages[i] + HHDM_OFFSET) as *const u8,
                    buf.as_mut_ptr().add(offset),
                    copy_len
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
        let dev = NVME.as_mut().ok_or("NVMe didn't started")?;
        let bs = dev.block_size as usize;
        
        if buf.len() % bs != 0 { return Err("buffer not aligned to block size"); }
        if buf.len() == 0 { return Ok(()); }

        let block_count = (buf.len() / bs) as u16;
        let page_count = (buf.len() + 4095) / 4096;

        if page_count > 513 { return Err("transfer too large for PRP list"); }

        let mut pages = [0u64; 513];
        
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
                copy_len
            );
            offset += copy_len;
            remaining -= copy_len;
        }

        let result = dev.write_block(lba, &pages[0..page_count], block_count);

        for i in 0..page_count {
            pfa::free_page(pages[i]);
        }

        result
    }
}

pub struct NvmeBlockDevice;

impl crate::fs::BlockDevice for NvmeBlockDevice {
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

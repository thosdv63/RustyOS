use crate::drivers::io::{mmio_read32, mmio_write32, mmio_read64, mmio_write64};
use crate::drivers::pci::{self, PciDevice};
use crate::mm::pfa;
use core::sync::atomic::{fence, Ordering};

// NVMe Controller Register Offsets
const REG_CAP: u64 = 0x00;    // Capabilities (64-bit)
const REG_CC: u64 = 0x14;     // Controller Configuration
const REG_CSTS: u64 = 0x1C;   // Controller Status
const REG_AQA: u64 = 0x24;    // Admin Queue Attributes
const REG_ASQ: u64 = 0x28;    // Admin SQ base (64-bit)
const REG_ACQ: u64 = 0x30;    // Admin CQ base (64-bit)

// Queue size (entry number)
const QUEUE_SIZE: usize = 64;

// Submission Queue Entry (64 byte)
#[repr(C)]
#[derive(Clone, Copy)]
struct SqEntry {
    opcode: u8,        // command type
    flags: u8,
    command_id: u16,   // To match in CQ
    nsid: u32,         // namespace id
    _reserved: u64,
    metadata: u64,
    prp1: u64,         // data adress 1
    prp2: u64,         // data adress 2
    cdw10: u32,        // special to command
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

// Completion Queue Entry (16 byte)
#[repr(C)]
#[derive(Clone, Copy)]
struct CqEntry {
    result: u32,       // result of command
    _reserved: u32,
    sq_head: u16,      // SQ head position
    sq_id: u16,        // which SQ
    command_id: u16,   // which command
    status: u16,       // status (bit 0 = phase, high bits = error code)
}

// NVMe Driver
pub struct NvmeDevice {
    bar0: u64,              // controller register addr
    doorbell_stride: u64,

    // Admin queue
    admin_sq: u64,          // admin SQ physical addr
    admin_cq: u64,          // admin CQ virtual addr
    admin_sq_tail: usize,
    admin_cq_head: usize,
    admin_phase: bool,      // CQ phase end

    // I/O queue
    io_sq: u64,
    io_cq: u64,
    io_sq_tail: usize,
    io_cq_head: usize,
    io_phase: bool,

    command_id: u16,        // increasing command id

    // Namespace bilgisi
    pub block_size: u32,    // block size (usually 512)
    pub block_count: u64,   // total block number
}

static mut NVME: Option<NvmeDevice> = None;

// SQ tail doorbell: 0x1000 + (2*qid) * stride
// CQ head doorbell:  0x1000 + (2*qid + 1) * stride
impl NvmeDevice {
    unsafe fn sq_doorbell(&self, qid: u64) -> u64 {
        self.bar0 + 0x1000 + (2 * qid) * self.doorbell_stride
    }
    unsafe fn cq_doorbell(&self, qid: u64) -> u64 {
        self.bar0 + 0x1000 + (2 * qid + 1) * self.doorbell_stride
    }

    // Send the admin command, wait for it to complete (polling)
    unsafe fn admin_command(&mut self, mut cmd: SqEntry) -> Result<u32, &'static str> {
        self.command_id = self.command_id.wrapping_add(1);
        cmd.command_id = self.command_id;

        // Write command to SQ
        let sq = self.admin_sq as *mut SqEntry;
        core::ptr::write_volatile(sq.add(self.admin_sq_tail), cmd);

        // Advance the tail (circularly)
        self.admin_sq_tail = (self.admin_sq_tail + 1) % QUEUE_SIZE;

        // Ensure (barrier) that the text is imprinted on memory
        fence(Ordering::SeqCst);

        // Doorbell call (notify disk)
        mmio_write32(self.sq_doorbell(0), self.admin_sq_tail as u32);

        let cq_db_addr = self.cq_doorbell(0);
        
        // critical area
        Self::wait_completion(
            self.admin_cq, 
            cq_db_addr, 
            &mut self.admin_cq_head, 
            &mut self.admin_phase
        )
    }

    // Sending io command
    unsafe fn io_command(&mut self, mut cmd: SqEntry) -> Result<u32, &'static str> {
        self.command_id = self.command_id.wrapping_add(1);
        cmd.command_id = self.command_id;

        let sq = self.io_sq as *mut SqEntry;
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

   // Pooling
    unsafe fn wait_completion(
        cq_addr: u64,
        cq_db_addr: u64, 
        cq_head: &mut usize,
        phase: &mut bool,
    ) -> Result<u32, &'static str> {
        let cq = cq_addr as *const CqEntry;
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
    // Read block (From LBA, buf to physical addr)
    pub unsafe fn read_block(&mut self, lba: u64, buf_phys: u64, block_count: u16) -> Result<(), &'static str> {
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x02; // NVM Read
        cmd.nsid = 1;
        cmd.prp1 = buf_phys;
        cmd.cdw10 = (lba & 0xFFFF_FFFF) as u32;       // LBA low
        cmd.cdw11 = (lba >> 32) as u32;               // LBA high
        cmd.cdw12 = (block_count - 1) as u32;          // block count (0-based)
        self.io_command(cmd)?;
        Ok(())
    }

    pub unsafe fn write_block(&mut self, lba: u64, buf_phys: u64, block_count: u16) -> Result<(), &'static str> {
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x01; // NVM Write
        cmd.nsid = 1;
        cmd.prp1 = buf_phys;
        cmd.cdw10 = (lba & 0xFFFF_FFFF) as u32;
        cmd.cdw11 = (lba >> 32) as u32;
        cmd.cdw12 = (block_count - 1) as u32;
        self.io_command(cmd)?;
        Ok(())
    }
}

pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    // Find NVMe controller in PCI (class 0x01, subclass 0x08)
    let nvme_dev = devices.iter()
        .find(|d| d.class == 0x01 && d.subclass == 0x08)
        .ok_or("NVMe controller could not found")?;

    // Open bus mastering (for dma)
    pci::enable_bus_master(nvme_dev.bus, nvme_dev.device, nvme_dev.function);

    // Get BAR0 (controller register address)
    let bar0 = pci::read_bar(nvme_dev.bus, nvme_dev.device, nvme_dev.function, 0);
    if bar0 == 0 {
        return Err("BAR0 is zero");
    }

    // Map BAR0 (register area, ~8KB is enough)
    for i in 0..4u64 {
        let addr = bar0 + i * 0x1000;
        if crate::mm::ptm::translate(addr).is_none() {
            crate::mm::ptm::map_page(addr, addr, true)?;
        }
    }

    unsafe {
        // read CAP Register (we need doorbell stride)
        let cap = mmio_read64(bar0 + REG_CAP);
        let doorbell_stride = 1u64 << (2 + ((cap >> 32) & 0xF)); // CAP.DSTRD

        // Reset controller (CC.EN = 0)
        let cc = mmio_read32(bar0 + REG_CC);
        mmio_write32(bar0 + REG_CC, cc & !1);
        // wait until CSTS.RDY = 0
        let mut spin = 0;
        while (mmio_read32(bar0 + REG_CSTS) & 1) != 0 {
            spin += 1;
            if spin > 100_000_000 { return Err("reset timeout"); }
            core::hint::spin_loop();
        }

        // Get frames for admin queues (1 page each = 4KB)
        let admin_sq = pfa::alloc_frame().ok_or("no admin SQ frame")?;
        let admin_cq = pfa::alloc_frame().ok_or("no admin CQ frame")?;
        // reset
        core::ptr::write_bytes(admin_sq as *mut u8, 0, 4096);
        core::ptr::write_bytes(admin_cq as *mut u8, 0, 4096);

        // AQA: admin queue sizes (0-based)
        let aqa = ((QUEUE_SIZE as u32 - 1) << 16) | (QUEUE_SIZE as u32 - 1);
        mmio_write32(bar0 + REG_AQA, aqa);
        // ASQ/ACQ: admin queue addresses
        mmio_write64(bar0 + REG_ASQ, admin_sq);
        mmio_write64(bar0 + REG_ACQ, admin_cq);

        // Open controller (CC.EN = 1, IOSQES=6 (64B), IOCQES=4 (16B))
        let cc_new = (6 << 16) | (4 << 20) | 1; // IOSQES=6, IOCQES=4, EN=1
        mmio_write32(bar0 + REG_CC, cc_new);
        // wait until CSTS.RDY = 1
        spin = 0;
        while (mmio_read32(bar0 + REG_CSTS) & 1) == 0 {
            spin += 1;
            if spin > 100_000_000 { return Err("enable timeout"); }
            core::hint::spin_loop();
        }

        // Create NvmeDevice struct
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

        // Identify Namespace (block size, number)
        let identify_buf = pfa::alloc_frame().ok_or("no identify frame")?;
        core::ptr::write_bytes(identify_buf as *mut u8, 0, 4096);

        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x06; // Identify
        cmd.nsid = 1;       // namespace 1
        cmd.prp1 = identify_buf;
        cmd.cdw10 = 0x00;   // CNS=0: namespace identify
        dev.admin_command(cmd)?;

        // get block info from Identify Namespace
        // NSZE (namespace size, blok cinsinden) offset 0
        let nsze = core::ptr::read_volatile(identify_buf as *const u64);
        // FLBAS (formatted LBA size) offset 26 -> which LBA Format
        let flbas = core::ptr::read_volatile((identify_buf + 26) as *const u8);
        let lba_format_index = (flbas & 0xF) as usize;
        // The LBAF sequence starts at offset 128, each string being 4 bytes
        let lbaf = core::ptr::read_volatile((identify_buf + 128 + (lba_format_index as u64) * 4) as *const u32);
        // LBADS (LBA data size) bit 16-23 -> 2^LBADS = block size
        let lbads = (lbaf >> 16) & 0xFF;
        dev.block_size = 1u32 << lbads;
        dev.block_count = nsze;

        pfa::free_frame(identify_buf);

        // create I/O Completion queue (with admin command)
        let io_cq = pfa::alloc_frame().ok_or("no io CQ frame")?;
        core::ptr::write_bytes(io_cq as *mut u8, 0, 4096);
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x05; // Create I/O Completion Queue
        cmd.prp1 = io_cq;
        cmd.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; // size | qid=1
        cmd.cdw11 = 1; // PC=1 (physically contiguous)
        dev.admin_command(cmd)?;
        dev.io_cq = io_cq;

        // create I/O Submission Queue
        let io_sq = pfa::alloc_frame().ok_or("no io SQ frame")?;
        core::ptr::write_bytes(io_sq as *mut u8, 0, 4096);
        let mut cmd = SqEntry::zeroed();
        cmd.opcode = 0x01; // Create I/O Submission Queue
        cmd.prp1 = io_sq;
        cmd.cdw10 = ((QUEUE_SIZE as u32 - 1) << 16) | 1; // size | qid=1
        cmd.cdw11 = (1 << 16) | 1; // CQID=1 | PC=1
        dev.admin_command(cmd)?;
        dev.io_sq = io_sq;

        NVME = Some(dev);
    }

    Ok(())
}

// Get info from disk
pub fn info() -> Option<(u32, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        NVME.as_ref().map(|d| (d.block_size, d.block_count))
    }
}

// BlockDevice interface (filesystem connected there)
// Read one block from LBA (It takes a buffer from the heap and converts it to physical form)
pub fn read_block(lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = NVME.as_mut().ok_or("NVMe didn't started")?;
        let bs = dev.block_size as usize;
        if buf.len() < bs { return Err("buffer is tiny"); }

        // Get a temporary frame for DMA (identity mapping: physical=virtual)
        let dma = pfa::alloc_frame().ok_or("no dma frame")?;
        dev.read_block(lba, dma, 1)?;
        // copy data to buf
        core::ptr::copy(dma as *const u8, buf.as_mut_ptr(), bs);
        pfa::free_frame(dma);
        Ok(())
    }
}

// write a block to LBA
pub fn write_block(lba: u64, buf: &[u8]) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let dev = NVME.as_mut().ok_or("NVMe didn't started")?;
        let bs = dev.block_size as usize;
        if buf.len() < bs { return Err("buffer is tiny"); }

        let dma = pfa::alloc_frame().ok_or("no dma frame")?;
        core::ptr::copy(buf.as_ptr(), dma as *mut u8, bs);
        dev.write_block(lba, dma, 1)?;
        pfa::free_frame(dma);
        Ok(())
    }
}

// Wrapper for BlockDevice trait
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
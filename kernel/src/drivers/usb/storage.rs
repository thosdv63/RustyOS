use crate::drivers::usb::xhci::{self, rd_buf, wr_buf};
use x86_64::instructions::interrupts::without_interrupts;

const CBW_SIG: u32 = 0x4342_5355; // "USBC"
const CSW_SIG: u32 = 0x5342_5355; // "USBS"
const CSW_OFF: u64 = 64;          // Location of CSW within cmd_buf
const MAX_DEVS: usize = 4;

pub struct StorageDev {
    slot: u8,
    lun: u8,
    pub block_size: u32,
    pub block_count: u64,
    tag: u32,
}

static mut STORAGE: [Option<StorageDev>; MAX_DEVS] = [None, None, None, None];

// small MMIO/memory helpers
unsafe fn wr8(a: u64, v: u8) { core::ptr::write_volatile(a as *mut u8, v); }
unsafe fn rd8(a: u64) -> u8 { core::ptr::read_volatile(a as *const u8) }
unsafe fn wr32le(a: u64, v: u32) { wr_buf(a, &v.to_le_bytes()); }
unsafe fn rd32le(a: u64) -> u32 {
    let mut b = [0u8; 4]; rd_buf(a, &mut b); u32::from_le_bytes(b)
}

// BOT: CBW -> [veri] -> CSW
fn bot(idx: usize, cb: &[u8], dir_in: bool, data_len: u32) -> Result<(), &'static str> {
    if cb.is_empty() || cb.len() > 16 { return Err("SCSI cmd size is invalid"); }

    let (slot, lun, tag) = unsafe {
        #[allow(static_mut_refs)]
        let d = STORAGE[idx].as_mut().ok_or("no usb storage")?;
        d.tag = d.tag.wrapping_add(1);
        (d.slot, d.lun, d.tag)
    };
    let (cbuf, dbuf) = xhci::msc_bufs(slot).ok_or("no msc buffer")?;
    let csw = cbuf + CSW_OFF;

    without_interrupts(|| {
        // prepare CBW (31 byte)
        unsafe {
            core::ptr::write_bytes(cbuf as *mut u8, 0, 96);
            wr32le(cbuf, CBW_SIG);
            wr32le(cbuf + 4, tag);
            wr32le(cbuf + 8, data_len);
            wr8(cbuf + 12, if dir_in { 0x80 } else { 0x00 });
            wr8(cbuf + 13, lun);
            wr8(cbuf + 14, cb.len() as u8);
            wr_buf(cbuf + 15, cb);
        }

        // command physe
        if xhci::bulk(slot, false, cbuf, 31).is_err() {
            let _ = xhci::recover(slot, false);
            xhci::bulk(slot, false, cbuf, 31)?;
        }

        // Data physe
        if data_len > 0 {
            if xhci::bulk(slot, dir_in, dbuf, data_len).is_err() {
                // stall: get the endpoint sorted, but read the CSW anyway
                let _ = xhci::recover(slot, dir_in);
            }
        }

        // Status physe
        if xhci::bulk(slot, true, csw, 13).is_err() {
            let _ = xhci::recover(slot, true);
            xhci::bulk(slot, true, csw, 13)?;
        }

        unsafe {
            if rd32le(csw) != CSW_SIG { return Err("CSW signature is invalid"); }
            if rd32le(csw + 4) != tag { return Err("CSW tag is invalid"); }
            match rd8(csw + 12) {
                0 => Ok(()),
                1 => Err("SCSI command failed"),
                _ => Err("SCSI physe error"),
            }
        }
    })
}

// SCSI Commands
fn test_unit_ready(idx: usize) -> Result<(), &'static str> {
    bot(idx, &[0x00, 0, 0, 0, 0, 0], false, 0)
}

fn request_sense(idx: usize) -> Result<(), &'static str> {
    bot(idx, &[0x03, 0, 0, 0, 18, 0], true, 18)
}

fn inquiry(idx: usize) -> Result<(), &'static str> {
    bot(idx, &[0x12, 0, 0, 0, 36, 0], true, 36)
}

fn read_capacity(idx: usize) -> Result<(u32, u32), &'static str> {
    bot(idx, &[0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0], true, 8)?;
    let slot = unsafe {
        #[allow(static_mut_refs)]
        STORAGE[idx].as_ref().ok_or("dev yok")?.slot
    };
    let (_, dbuf) = xhci::msc_bufs(slot).ok_or("buffer yok")?;
    let mut b = [0u8; 8];
    unsafe { rd_buf(dbuf, &mut b); }
    // Big-endian!
    let last_lba = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let bsize = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
    Ok((last_lba, bsize))
}

fn rw10(idx: usize, write: bool, lba: u32, blocks: u16) -> Result<(), &'static str> {
    let bs = unsafe {
        #[allow(static_mut_refs)]
        STORAGE[idx].as_ref().ok_or("dev yok")?.block_size
    };
    let l = lba.to_be_bytes();
    let n = blocks.to_be_bytes();
    let cb = [
        if write { 0x2A } else { 0x28 }, 0,
        l[0], l[1], l[2], l[3],
        0, n[0], n[1], 0,
    ];
    bot(idx, &cb, !write, bs * blocks as u32)
}


pub fn init_all() {
    let mut slots = [0u8; 4];
    let n = xhci::storage_slots(&mut slots);

    for i in 0..n {
        let slot = slots[i];

        // GET MAX LUN (For supported devices; if STALL occurs, it returns 0)
        let lun = {
            let lun_buf = match xhci::msc_bufs(slot) { Some((c, _)) => c + 96, None => continue };
            match xhci::ctrl(slot, 0xA1, 0xFE, 0, xhci::msc_iface(slot) as u16, lun_buf, 1) {
                Ok(_) => unsafe { rd8(lun_buf) & 0x0F },
                Err(_) => { let _ = xhci::recover(slot, true); 0 }
            }
        };

        unsafe {
            #[allow(static_mut_refs)]
            { STORAGE[i] = Some(StorageDev { slot, lun, block_size: 512, block_count: 0, tag: 0 }); }
        }

        let _ = inquiry(i);

        // Try a few times until the device is ready
        let mut ready = false;
        for _ in 0..50 {
            if test_unit_ready(i).is_ok() { ready = true; break; }
            let _ = request_sense(i);
            crate::drivers::usb::mdelay(50);
        }
        if !ready {
            unsafe {
                #[allow(static_mut_refs)]
                { STORAGE[i] = None; }
            }
            continue;
        }

        match read_capacity(i) {
            Ok((last, bs)) if bs >= 512 && bs <= 4096 => unsafe {
                #[allow(static_mut_refs)]
                if let Some(d) = STORAGE[i].as_mut() {
                    d.block_size = bs;
                    d.block_count = last as u64 + 1;
                }
            },
            _ => unsafe {
                #[allow(static_mut_refs)]
                { STORAGE[i] = None; }
            },
        }
    }
}

pub fn count() -> usize {
    unsafe {
        #[allow(static_mut_refs)]
        STORAGE.iter().filter(|d| d.is_some()).count()
    }
}

pub fn info(idx: usize) -> Option<(u32, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        STORAGE.get(idx)?.as_ref().map(|d| (d.block_size, d.block_count))
    }
}

// block device interface
pub fn read_block(idx: usize, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    let (slot, bs) = unsafe {
        #[allow(static_mut_refs)]
        let d = STORAGE.get(idx).and_then(|x| x.as_ref()).ok_or("usb depolama yok")?;
        (d.slot, d.block_size as usize)
    };
    if buf.len() < bs { return Err("buffer is small"); }
    if lba > u32::MAX as u64 { return Err("LBA is very big"); }

    rw10(idx, false, lba as u32, 1)?;

    let (_, dbuf) = xhci::msc_bufs(slot).ok_or("no buffer")?;
    unsafe { rd_buf(dbuf, &mut buf[..bs]); }
    Ok(())
}

pub fn write_block(idx: usize, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
    let (slot, bs) = unsafe {
        #[allow(static_mut_refs)]
        let d = STORAGE.get(idx).and_then(|x| x.as_ref()).ok_or("usb depolama yok")?;
        (d.slot, d.block_size as usize)
    };
    if buf.len() < bs { return Err("buffer is small"); }
    if lba > u32::MAX as u64 { return Err("LBA is very big"); }

    let (_, dbuf) = xhci::msc_bufs(slot).ok_or("no buffer")?;
    unsafe { wr_buf(dbuf, &buf[..bs]); }

    rw10(idx, true, lba as u32, 1)
}

pub struct UsbBlockDevice { pub idx: usize }

impl crate::fs::BlockDevice for UsbBlockDevice {
    fn read_block(&mut self, lba: u64, buf: &mut [u8]) -> Result<(), &'static str> {
        read_block(self.idx, lba, buf)
    }
    fn write_block(&mut self, lba: u64, buf: &[u8]) -> Result<(), &'static str> {
        write_block(self.idx, lba, buf)
    }
    fn block_size(&self) -> u32 {
        info(self.idx).map(|(bs, _)| bs).unwrap_or(512)
    }
}
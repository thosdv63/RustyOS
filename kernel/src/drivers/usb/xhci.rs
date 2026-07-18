use crate::drivers::io::*;
use crate::drivers::pci::{self, PciDevice};
use crate::drivers::usb::{mdelay, CLS_HID, CLS_MSC};
use crate::mm::pfa;
use core::sync::atomic::{fence, AtomicBool, Ordering};

// Register offsets
const CAP_CAPLENGTH: u64 = 0x00;
const CAP_HCIVERSION: u64 = 0x02;
const CAP_HCSPARAMS1: u64 = 0x04;
const CAP_HCSPARAMS2: u64 = 0x08;
const CAP_HCCPARAMS1: u64 = 0x10;
const CAP_DBOFF: u64 = 0x14;
const CAP_RTSOFF: u64 = 0x18;

pub static INIT_RESULT:  core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0); // 0=hic,1=OK,2=HATA
pub static INIT_ERR_PTR: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static INIT_ERR_LEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub static ENUM_OK:      core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub static ENUM_ERR_PTR: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static ENUM_ERR_LEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub static LAST_CC: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

const OP_USBCMD: u64 = 0x00;
const OP_USBSTS: u64 = 0x04;
const OP_CRCR: u64 = 0x18;
const OP_DCBAAP: u64 = 0x30;
const OP_CONFIG: u64 = 0x38;
const OP_PORTSC: u64 = 0x400;

const RT_IMAN: u64 = 0x20;
const RT_IMOD: u64 = 0x24;
const RT_ERSTSZ: u64 = 0x28;
const RT_ERSTBA: u64 = 0x30;
const RT_ERDP: u64 = 0x38;

const CMD_RS: u32 = 1 << 0;
const CMD_HCRST: u32 = 1 << 1;

const STS_HCH: u32 = 1 << 0;
const STS_CNR: u32 = 1 << 11;

const PSC_CCS: u32 = 1 << 0;
const PSC_PED: u32 = 1 << 1;
const PSC_PR: u32 = 1 << 4;
const PSC_PP: u32 = 1 << 9;
const PSC_CSC: u32 = 1 << 17;
const PSC_PEC: u32 = 1 << 18;
const PSC_PRC: u32 = 1 << 21;
const PSC_KEEP: u32 = 0x0E00_C3E0;

const TRB_NORMAL: u32 = 1;
const TRB_SETUP: u32 = 2;
const TRB_DATA: u32 = 3;
const TRB_STATUS: u32 = 4;
const TRB_LINK: u32 = 6;
const TRB_ENABLE_SLOT: u32 = 9;
const TRB_ADDRESS_DEVICE: u32 = 11;
const TRB_CONFIG_EP: u32 = 12;
const TRB_EVAL_CTX: u32 = 13;
const TRB_RESET_EP: u32 = 14;
const TRB_SET_TR_DEQ: u32 = 16;
const TRB_NOOP_CMD: u32 = 23;
const TRB_TRANSFER_EVENT: u32 = 32;
const TRB_CMD_COMPLETE: u32 = 33;
const TRB_PORT_STATUS: u32 = 34;

const TRB_C: u32 = 1 << 0;
const TRB_TC: u32 = 1 << 1;
const TRB_ISP: u32 = 1 << 2;
const TRB_CH: u32 = 1 << 4;
const TRB_IOC: u32 = 1 << 5;
const TRB_IDT: u32 = 1 << 6;
const TRB_DIR_IN: u32 = 1 << 16;

const EPT_BULK_OUT: u32 = 2;
const EPT_CONTROL: u32 = 4;
const EPT_BULK_IN: u32 = 6;
const EPT_INTR_IN: u32 = 7;

const RING_TRBS: usize = 256;
const MAX_SLOTS_USED: usize = 256;
const SPIN_MAX: u64 = 50_000_000;

const QUEUE_DEPTH: usize = 8;
const SLOT_SIZE: usize = 64;

#[derive(Clone, Copy)]
pub struct Trb {
    pub param: u64,
    pub status: u32,
    pub control: u32,
}

impl Trb {
    fn ttype(&self) -> u32 { (self.control >> 10) & 0x3F }
    fn cc(&self) -> u32 { (self.status >> 24) & 0xFF }
    fn slot_id(&self) -> u32 { (self.control >> 24) & 0xFF }
    fn ep_id(&self) -> u32 { (self.control >> 16) & 0x1F }
    fn resid(&self) -> u32 { self.status & 0x00FF_FFFF }
}

unsafe fn zero_frame(addr: u64) {
    core::ptr::write_bytes(addr as *mut u8, 0, 4096);
}

pub unsafe fn rd_buf(addr: u64, out: &mut [u8]) {
    for i in 0..out.len() {
        out[i] = core::ptr::read_volatile((addr + i as u64) as *const u8);
    }
}

pub unsafe fn wr_buf(addr: u64, src: &[u8]) {
    for i in 0..src.len() {
        core::ptr::write_volatile((addr + i as u64) as *mut u8, src[i]);
    }
}

pub struct Ring {
    base: u64,
    enq: usize,
    cycle: bool,
}

impl Ring {
    fn new() -> Option<Ring> {
        let base = pfa::alloc_frame()?;
        let mut r = Ring { base, enq: 0, cycle: true };
        r.reset();
        Some(r)
    }

    fn reset(&mut self) {
        unsafe {
            zero_frame(self.base);
            let l = self.base + ((RING_TRBS - 1) * 16) as u64;
            core::ptr::write_volatile(l as *mut u64, self.base);
            core::ptr::write_volatile((l + 8) as *mut u32, 0);
            core::ptr::write_volatile((l + 12) as *mut u32, (TRB_LINK << 10) | TRB_TC | TRB_C);
        }
        self.enq = 0;
        self.cycle = true;
    }

    fn push(&mut self, param: u64, status: u32, control: u32) -> u64 {
        let addr = self.base + (self.enq * 16) as u64;
        unsafe {
            core::ptr::write_volatile(addr as *mut u64, param);
            core::ptr::write_volatile((addr + 8) as *mut u32, status);
            fence(Ordering::SeqCst);
            let c = if self.cycle { TRB_C } else { 0 };
            core::ptr::write_volatile((addr + 12) as *mut u32, control | c);
        }
        self.enq += 1;
        if self.enq == RING_TRBS - 1 {
            unsafe {
                let l = self.base + ((RING_TRBS - 1) * 16) as u64;
                let c = if self.cycle { TRB_C } else { 0 };
                core::ptr::write_volatile((l + 12) as *mut u32, (TRB_LINK << 10) | TRB_TC | c);
            }
            self.cycle = !self.cycle;
            self.enq = 0;
        }
        fence(Ordering::SeqCst);
        addr
    }
}
struct EventRing {
    base: u64,
    deq: usize,
    cycle: bool,
    erdp: u64,
}

impl EventRing {
    fn poll(&mut self) -> Option<Trb> {
        let addr = self.base + (self.deq * 16) as u64;
        let control = unsafe { core::ptr::read_volatile((addr + 12) as *const u32) };
        if ((control & 1) == 1) != self.cycle { return None; }
        fence(Ordering::SeqCst);
        let trb = unsafe {
            Trb {
                param: core::ptr::read_volatile(addr as *const u64),
                status: core::ptr::read_volatile((addr + 8) as *const u32),
                control,
            }
        };
        self.deq += 1;
        if self.deq == RING_TRBS {
            self.deq = 0;
            self.cycle = !self.cycle;
        }
        let next = self.base + (self.deq * 16) as u64;
        unsafe { mmio_write64(self.erdp, (next & !0xF) | (1 << 3)); }
        Some(trb)
    }
}

pub struct UsbDevice {
    slot: u8,
    speed: u8,
    port: u8,
    class: u8,
    iface: u8,
    dev_ctx: u64,
    input_ctx: u64,
    ep0: Ring,
    ep0_max: u16,

    // HID
    intr: Option<Ring>,
    dci: u8,
    buf: u64,
    buf_len: u16,
    proto: u8,

    // Mass storage
    bulk_in: Option<Ring>,
    bulk_out: Option<Ring>,
    dci_in: u8,
    dci_out: u8,
    ep_in_addr: u8,
    ep_out_addr: u8,
    msc_cmd: u64,
    msc_data: u64,
}

// ============================================================
// === Xhci ===================================================
// ============================================================
pub struct Xhci {
    bar: u64,
    op: u64,
    rt: u64,
    db: u64,
    ctx_size: usize,
    max_slots: u8,
    max_ports: u8,
    dcbaa: u64,
    cmd: Ring,
    evt: EventRing,
    devices: [Option<UsbDevice>; MAX_SLOTS_USED],
}

static mut XHCI: Option<Xhci> = None;
static POLL_BUSY: AtomicBool = AtomicBool::new(false);

impl Xhci {
    unsafe fn portsc(&self, i: u8) -> u64 { self.op + OP_PORTSC + (i as u64) * 0x10 }

    unsafe fn portsc_set(&self, i: u8, bits: u32) {
        let a = self.portsc(i);
        let cur = mmio_read32(a);
        mmio_write32(a, (cur & PSC_KEEP) | bits);
    }

    unsafe fn ring_db(&self, slot: u8, target: u32) {
        fence(Ordering::SeqCst);
        mmio_write32(self.db + (slot as u64) * 4, target);
    }

    fn ctx(&self, base: u64, idx: usize) -> u64 { base + (idx * self.ctx_size) as u64 }

    unsafe fn write_ep(&self, ictx: u64, dci: u8, ept: u32, mps: u16, ival: u8, tr: u64, avg: u32) {
        let ep = self.ctx(ictx, 1 + dci as usize);
        core::ptr::write_volatile(ep as *mut u32, (ival as u32) << 16);
        let d1 = (3u32 << 1) | (ept << 3) | ((mps as u32) << 16);
        core::ptr::write_volatile((ep + 4) as *mut u32, d1);
        core::ptr::write_volatile((ep + 8) as *mut u64, tr | 1);
        core::ptr::write_volatile((ep + 16) as *mut u32, avg);
    }

    // --- hid event use ---
    fn handle_hid(&mut self, trb: Trb) {
        if trb.ttype() != TRB_TRANSFER_EVENT { return; }
        let cc = trb.cc();
        if cc != 1 && cc != 13 { return; }
        let slot = trb.slot_id() as usize;
        let dci = trb.ep_id() as u8;
        if slot == 0 || slot >= MAX_SLOTS_USED { return; }

        let db = self.db;
        let trb_ptr = trb.param & !0xF;

        let Some(d) = self.devices[slot].as_mut() else { return };
        if d.class != CLS_HID || d.dci != dci { return; }
        let rbase = match d.intr.as_ref() { Some(r) => r.base, None => return };
        if trb_ptr < rbase || trb_ptr >= rbase + 4096 { return; }

        let bufp = unsafe { core::ptr::read_volatile(trb_ptr as *const u64) };
        if bufp == 0 { return; }

        let want = d.buf_len as u32;
        let got = want.saturating_sub(trb.resid()) as usize;
        if got > 0 {
            let mut tmp = [0u8; 8];
            let n = got.min(8);
            unsafe { rd_buf(bufp, &mut tmp[..n]); }
            crate::drivers::usb::hid::on_report(d.proto, &tmp[..n]);
        }

        if let Some(r) = d.intr.as_mut() {
            r.push(bufp, want, (TRB_NORMAL << 10) | TRB_ISP | TRB_IOC);
        }
        unsafe {
            fence(Ordering::SeqCst);
            mmio_write32(db + (slot as u64) * 4, dci as u32);
        }
    }

    fn wait_cmd(&mut self) -> Result<Trb, &'static str> {
        let mut spin = 0u64;
        loop {
            if let Some(t) = self.evt.poll() {
                if t.ttype() == TRB_CMD_COMPLETE { return Ok(t); }
                self.handle_hid(t);
                continue;
            }
            spin += 1;
            if spin > SPIN_MAX { return Err("xhci: command timeout"); }
            core::hint::spin_loop();
        }
    }

    fn wait_transfer(&mut self, slot: u8, dci: u8) -> Result<Trb, &'static str> {
        let mut spin = 0u64;
        loop {
            if let Some(t) = self.evt.poll() {
                if t.ttype() == TRB_TRANSFER_EVENT
                    && t.slot_id() as u8 == slot
                    && t.ep_id() as u8 == dci
                {
                    return Ok(t);
                }
                self.handle_hid(t);
                continue;
            }
            spin += 1;
            if spin > SPIN_MAX { return Err("xhci: transfer timeout"); }
            core::hint::spin_loop();
        }
    }

    fn command(&mut self, param: u64, control: u32) -> Result<Trb, &'static str> {
        self.cmd.push(param, 0, control);
        unsafe { self.ring_db(0, 0); }
        let ev = self.wait_cmd()?;
        if ev.cc() != 1 {
    LAST_CC.store(ev.cc(), Ordering::Relaxed);
    return Err("xhci: command error");
}
        Ok(ev)
    }

    fn control(
        &mut self,
        dev: &mut UsbDevice,
        req_type: u8, req: u8, value: u16, index: u16,
        buf: u64, len: u16,
    ) -> Result<u32, &'static str> {
        let in_dir = (req_type & 0x80) != 0;

        let setup: u64 = (req_type as u64)
            | ((req as u64) << 8)
            | ((value as u64) << 16)
            | ((index as u64) << 32)
            | ((len as u64) << 48);

        let trt: u32 = if len == 0 { 0 } else if in_dir { 3 } else { 2 };
        let ctl = (TRB_SETUP << 10) | TRB_IDT | (trt << 16);
        dev.ep0.push(setup, 8, ctl);

        if len != 0 {
    let mut dctl = TRB_DATA << 10;   // no TRB_CH
    if in_dir { dctl |= TRB_DIR_IN; }
    dev.ep0.push(buf, len as u32, dctl);
}

        let mut sctl = (TRB_STATUS << 10) | TRB_IOC;
        if len == 0 || !in_dir { sctl |= TRB_DIR_IN; }
        dev.ep0.push(0, 0, sctl);

        let slot = dev.slot;
        unsafe { self.ring_db(slot, 1); }
        let ev = self.wait_transfer(slot, 1)?;
        let cc = ev.cc();
        if cc != 1 && cc != 13 { return Err("xhci: control transfer error"); }
        Ok(ev.resid())
    }

    fn control_slot(
        &mut self, slot: u8,
        req_type: u8, req: u8, value: u16, index: u16,
        buf: u64, len: u16,
    ) -> Result<u32, &'static str> {
        let mut dev = self.devices[slot as usize].take().ok_or("slot bos")?;
        let r = self.control(&mut dev, req_type, req, value, index, buf, len);
        self.devices[slot as usize] = Some(dev);
        r
    }
}

unsafe fn bios_handoff(bar: u64, hccparams1: u32) {
    let xecp = ((hccparams1 >> 16) & 0xFFFF) as u64;
    if xecp == 0 { return; }
    let mut off = xecp * 4;
    let mut guard = 0;
    loop {
        let cap = mmio_read32(bar + off);
        if (cap & 0xFF) == 1 {
            mmio_write32(bar + off, cap | (1 << 24));
            let mut spin = 0u64;
            while (mmio_read32(bar + off) & (1 << 16)) != 0 {
                spin += 1;
                if spin > 5_000_000 { break; }
                core::hint::spin_loop();
            }
            mmio_write32(bar + off + 4, 0);
            return;
        }
        let next = (cap >> 8) & 0xFF;
        if next == 0 { return; }
        off += (next as u64) * 4;
        guard += 1;
        if guard > 64 { return; }
    }
}

fn ep0_max_for(speed: u8) -> u16 {
    match speed { 3 => 64, 4 => 512, _ => 8 }
}

fn interval_for(speed: u8, b_interval: u8) -> u8 {
    let bi = if b_interval == 0 { 1u32 } else { b_interval as u32 };
    match speed {
        1 | 2 => {
            let mut log = 0u32;
            while (1u32 << (log + 1)) <= bi { log += 1; }
            (log + 3) as u8
        }
        _ => (bi - 1) as u8,
    }
}

pub fn init(devices: &[PciDevice]) -> Result<(), &'static str> {
    let r = init_inner(devices);
    match &r {
        Ok(()) => INIT_RESULT.store(1, Ordering::Relaxed),
        Err(e) => {
            INIT_RESULT.store(2, Ordering::Relaxed);
            INIT_ERR_PTR.store(e.as_ptr() as u64, Ordering::Relaxed);
            INIT_ERR_LEN.store(e.len() as u32, Ordering::Relaxed);
        }
    }
    r
}

fn init_inner(devices: &[PciDevice]) -> Result<(), &'static str> {
    let pd = devices.iter()
        .find(|d| d.class == 0x0C && d.subclass == 0x03 && d.prog_if == 0x30)
        .ok_or("xHCI controller couldn't found")?;

    pci::enable_bus_master(pd.bus, pd.device, pd.function);

    let bar = pci::read_bar(pd.bus, pd.device, pd.function, 0);
    if bar == 0 { return Err("xHCI BAR0 is zero"); }

    for i in 0..16u64 {
        let a = bar + i * 0x1000;
        if crate::mm::ptm::translate(a).is_none() {
            crate::mm::ptm::map_page(a, a, true)?;
        }
    }

    unsafe {
        let caplen = mmio_read8(bar + CAP_CAPLENGTH) as u64;
        let hcs1 = mmio_read32(bar + CAP_HCSPARAMS1);
        let hcs2 = mmio_read32(bar + CAP_HCSPARAMS2);
        let hcc1 = mmio_read32(bar + CAP_HCCPARAMS1);
        let dboff = (mmio_read32(bar + CAP_DBOFF) & !0x3) as u64;
        let rtsoff = (mmio_read32(bar + CAP_RTSOFF) & !0x1F) as u64;

        let max_slots = (hcs1 & 0xFF) as u8;
        let max_ports = ((hcs1 >> 24) & 0xFF) as u8;
        let ctx_size: usize = if (hcc1 & (1 << 2)) != 0 { 64 } else { 32 };

        let sp_hi = (hcs2 >> 21) & 0x1F;
        let sp_lo = (hcs2 >> 27) & 0x1F;
        let scratchpads = ((sp_hi << 5) | sp_lo) as usize;

        if max_slots == 0 || max_ports == 0 { return Err("xHCI parametreleri gecersiz"); }

        bios_handoff(bar, hcc1);

        let op = bar + caplen;
        let rt = bar + rtsoff;
        let db = bar + dboff;

        let cmd = mmio_read32(op + OP_USBCMD);
        mmio_write32(op + OP_USBCMD, cmd & !CMD_RS);
        let mut spin = 0u64;
        while (mmio_read32(op + OP_USBSTS) & STS_HCH) == 0 {
            spin += 1;
            if spin > SPIN_MAX { return Err("xHCI halt timeout"); }
            core::hint::spin_loop();
        }
        mmio_write32(op + OP_USBCMD, CMD_HCRST);
        spin = 0;
        while (mmio_read32(op + OP_USBCMD) & CMD_HCRST) != 0 {
            spin += 1;
            if spin > SPIN_MAX { return Err("xHCI reset timeout"); }
            core::hint::spin_loop();
        }
        spin = 0;
        while (mmio_read32(op + OP_USBSTS) & STS_CNR) != 0 {
            spin += 1;
            if spin > SPIN_MAX { return Err("xHCI CNR timeout"); }
            core::hint::spin_loop();
        }

        let dcbaa = pfa::alloc_frame().ok_or("dcbaa frame yok")?;
        zero_frame(dcbaa);

        if scratchpads > 0 {
            let arr = pfa::alloc_frame().ok_or("scratchpad array doesn't exist")?;
            zero_frame(arr);
            for i in 0..scratchpads.min(512) {
                let sp = pfa::alloc_frame().ok_or("scratchpad frame doesn't exist")?;
                zero_frame(sp);
                core::ptr::write_volatile((arr + (i * 8) as u64) as *mut u64, sp);
            }
            core::ptr::write_volatile(dcbaa as *mut u64, arr);
        }

        let cmd_ring = Ring::new().ok_or("cmd ring frame doesn't exist")?;

        let evt_seg = pfa::alloc_frame().ok_or("event seg frame doesn't exist")?;
        zero_frame(evt_seg);
        let erst = pfa::alloc_frame().ok_or("erst frame doesn't exist")?;
        zero_frame(erst);
        core::ptr::write_volatile(erst as *mut u64, evt_seg);
        core::ptr::write_volatile((erst + 8) as *mut u32, RING_TRBS as u32);
        core::ptr::write_volatile((erst + 12) as *mut u32, 0);

        let slots_en = max_slots as u32;
        mmio_write32(op + OP_CONFIG, slots_en);
        mmio_write64(op + OP_DCBAAP, dcbaa);
        mmio_write64(op + OP_CRCR, cmd_ring.base | 1);

        mmio_write32(rt + RT_ERSTSZ, 1);
        mmio_write64(rt + RT_ERDP, evt_seg | (1 << 3));
        mmio_write64(rt + RT_ERSTBA, erst);
        mmio_write32(rt + RT_IMOD, 0);
        mmio_write32(rt + RT_IMAN, 0);

        mmio_write32(op + OP_USBCMD, CMD_RS);
        spin = 0;
        while (mmio_read32(op + OP_USBSTS) & STS_HCH) != 0 {
            spin += 1;
            if spin > SPIN_MAX { return Err("xHCI run timeout"); }
            core::hint::spin_loop();
        }

        const NONE: Option<UsbDevice> = None;
        let mut x = Xhci {
            bar, op, rt, db, ctx_size, max_slots, max_ports, dcbaa,
            cmd: cmd_ring,
            evt: EventRing { base: evt_seg, deq: 0, cycle: true, erdp: rt + RT_ERDP },
            devices: [NONE; MAX_SLOTS_USED],
        };

        x.command(0, TRB_NOOP_CMD << 10)?;

        for p in 0..x.max_ports {
            let a = x.portsc(p);
            let sc = mmio_read32(a);
            if sc & PSC_CCS == 0 || sc & PSC_PED != 0 { continue; }
            if sc & PSC_PP == 0 { mmio_write32(a, (sc & PSC_KEEP) | PSC_PP); mdelay(20); }
            let sc = mmio_read32(a);
            mmio_write32(a, (sc & PSC_KEEP) | PSC_PR);
        }
        let mut spin = 0u64;
        loop {
            let mut done = true;
            for p in 0..x.max_ports {
                let v = mmio_read32(x.portsc(p));
                if v & PSC_CCS != 0 && v & PSC_PR != 0 { done = false; }
            }
        if done { break; }
        spin += 1; if spin > SPIN_MAX { break; }
        core::hint::spin_loop();
    }
    mdelay(15); // reset recovery
    for p in 0..x.max_ports {
        let v = mmio_read32(x.portsc(p));
        if v & PSC_CCS != 0 {
            mmio_write32(x.portsc(p), (v & PSC_KEEP) | PSC_PRC | PSC_CSC | PSC_PEC);
        }
    }

        let mut nok = 0u32;
        for p in 0..x.max_ports {
            match x.enumerate_port(p) {
                Ok(()) => nok += 1,
                Err(e) => {
                    if e != "port bos" {
                        ENUM_ERR_PTR.store(e.as_ptr() as u64, Ordering::Relaxed);
                        ENUM_ERR_LEN.store(e.len() as u32, Ordering::Relaxed);
                    }
                }
            }
        }
        ENUM_OK.store(nok, Ordering::Relaxed);
        XHCI = Some(x);
    }

    Ok(())
}

impl Xhci {
    fn enumerate_port(&mut self, port: u8) -> Result<(), &'static str> {
        unsafe {
            let a = self.portsc(port);
            let mut sc = mmio_read32(a);

            if (sc & PSC_PP) == 0 {
                self.portsc_set(port, PSC_PP);
                mdelay(50);
                sc = mmio_read32(a);
            }
            
            if (sc & PSC_CCS) == 0 { return Err("port is empty"); }

            self.portsc_set(port, PSC_CSC | PSC_PEC | PSC_PRC);
            mdelay(20);

            sc = mmio_read32(a);
            
            // USB 2.0 / Low Speed reset
            if (sc & PSC_PED) == 0 {
                self.portsc_set(port, PSC_PR);
                
                let mut spin = 0u64;
                loop {
                    let v = mmio_read32(a);
                    if (v & PSC_PRC) != 0 { break; }
                    if (v & PSC_PED) != 0 && (v & PSC_PR) == 0 { break; }
                    
                    spin += 1;
                    if spin > SPIN_MAX { return Err("port reset timeout"); }
                    core::hint::spin_loop();
                }
                
                // give delay
                mdelay(50);
                
                // critical!
                self.portsc_set(port, PSC_PRC | PSC_CSC | PSC_PEC);
                mdelay(50);
            }

            sc = mmio_read32(a);
            if (sc & PSC_PED) == 0 { return Err("port etkinlesmedi"); }

            crate::drivers::usb::mdelay(150);
            
            let speed = ((sc >> 10) & 0xF) as u8;
            self.attach(port, speed)
        }
    }

    fn attach(&mut self, port: u8, speed: u8) -> Result<(), &'static str> {
        let ev = self.command(0, TRB_ENABLE_SLOT << 10)?;
        let slot = ev.slot_id() as u8;
        if slot == 0 || (slot as usize) >= MAX_SLOTS_USED { return Err("gecersiz slot id"); }

        let dev_ctx = pfa::alloc_frame().ok_or("dev ctx frame yok")?;
        let input_ctx = pfa::alloc_frame().ok_or("input ctx frame yok")?;
        let buf = pfa::alloc_frame().ok_or("dma buf frame yok")?;
        unsafe {
            zero_frame(dev_ctx);
            zero_frame(input_ctx);
            zero_frame(buf);
            core::ptr::write_volatile((self.dcbaa + (slot as u64) * 8) as *mut u64, dev_ctx);
        }

        let ep0 = Ring::new().ok_or("ep0 ring frame yok")?;
        let ep0_max = ep0_max_for(speed);

        let mut dev = UsbDevice {
            slot, speed, port, class: 0, iface: 0,
            dev_ctx, input_ctx, ep0, ep0_max,
            intr: None, dci: 0, buf, buf_len: 8, proto: 0,
            bulk_in: None, bulk_out: None, dci_in: 0, dci_out: 0,
            ep_in_addr: 0, ep_out_addr: 0, msc_cmd: 0, msc_data: 0,
        };

        unsafe {
            let icc = self.ctx(input_ctx, 0);
            core::ptr::write_volatile(icc as *mut u32, 0);
            core::ptr::write_volatile((icc + 4) as *mut u32, 0b11);

            let sc = self.ctx(input_ctx, 1);
            core::ptr::write_volatile(sc as *mut u32, ((speed as u32) << 20) | (1u32 << 27));
            core::ptr::write_volatile((sc + 4) as *mut u32, ((port as u32) + 1) << 16);
            core::ptr::write_volatile((sc + 8) as *mut u32, 0);
            core::ptr::write_volatile((sc + 12) as *mut u32, 0);

            self.write_ep(input_ctx, 1, EPT_CONTROL, ep0_max, 0, dev.ep0.base, 8);
        }

        self.command(input_ctx, (TRB_ADDRESS_DEVICE << 10) | ((slot as u32) << 24))?;
        mdelay(10);

        self.control(&mut dev, 0x80, 6, 0x0100, 0, buf, 8)?;
        let mut d8 = [0u8; 8];
        unsafe { rd_buf(buf, &mut d8); }
        let real_mps = if speed == 4 { 1u16 << d8[7] } else { d8[7] as u16 };

        if real_mps != 0 && real_mps != ep0_max {
            unsafe {
                let icc = self.ctx(input_ctx, 0);
                core::ptr::write_volatile(icc as *mut u32, 0);
                core::ptr::write_volatile((icc + 4) as *mut u32, 0b10);
                self.write_ep(input_ctx, 1, EPT_CONTROL, real_mps, 0, dev.ep0.base, 8);
            }
            self.command(input_ctx, (TRB_EVAL_CTX << 10) | ((slot as u32) << 24))?;
            dev.ep0_max = real_mps;
        }

        self.control(&mut dev, 0x80, 6, 0x0200, 0, buf, 9)?;
        let mut c9 = [0u8; 9];
        unsafe { rd_buf(buf, &mut c9); }
        let total = u16::from_le_bytes([c9[2], c9[3]]).min(1024); // 1024'e çıkarıldı
        if total < 9 { return Err("config desc bozuk"); }
        self.control(&mut dev, 0x80, 6, 0x0200, 0, buf, total)?;

        let mut cfg = [0u8; 1024]; // Dizi boyutu artırıldı
        unsafe { rd_buf(buf, &mut cfg[..total as usize]); }
        let cfg_value = cfg[5];

        let end = total as usize;
        let mut i = 0usize;
        let mut mode: u8 = 0;          // CLS_HID | CLS_MSC
        let mut iface: i32 = -1;
        let mut proto: u8 = 0;
        let mut ep_int_addr: u8 = 0;
        let mut ep_int_mps: u16 = 0;
        let mut ep_int_iv: u8 = 10;
        let mut ep_in: u8 = 0; let mut ep_in_mps: u16 = 0;
        let mut ep_out: u8 = 0; let mut ep_out_mps: u16 = 0;
        let mut cur: u8 = 0;

        while i + 1 < end {
            let dlen = cfg[i] as usize;
            let dtype = cfg[i + 1];
            if dlen == 0 { break; }

           if dtype == 0x04 && i + 9 <= end {
                let icls = cfg[i + 5];
                let isub = cfg[i + 6];
                let iprot = cfg[i + 7];
                cur = 0;
                
                if icls == CLS_HID && isub == 0x01 && (iprot == 1 || iprot == 2) {

                    if mode == 0 || iprot == 2 {
                        cur = CLS_HID; 
                        mode = CLS_HID;
                        iface = cfg[i + 2] as i32; 
                        proto = iprot;
                        
                        if iprot == 2 {
                            ep_int_addr = 0;
                            ep_int_mps = 0;
                        }
                    }
                } else if icls == CLS_MSC && isub == 0x06 && iprot == 0x50 {
                    if mode == 0 {
                        cur = CLS_MSC; mode = CLS_MSC;
                        iface = cfg[i + 2] as i32;
                    }
                }
            } else if dtype == 0x05 && i + 7 <= end && cur != 0 {
                let addr = cfg[i + 2];
                let attr = cfg[i + 3] & 0x03;
                let mps = u16::from_le_bytes([cfg[i + 4], cfg[i + 5]]) & 0x07FF;
                if cur == CLS_HID && attr == 3 && (addr & 0x80) != 0 && ep_int_addr == 0 {
                    ep_int_addr = addr; ep_int_mps = mps; ep_int_iv = cfg[i + 6];
                } else if cur == CLS_MSC && attr == 2 {
                    if (addr & 0x80) != 0 { if ep_in == 0 { ep_in = addr; ep_in_mps = mps; } }
                    else { if ep_out == 0 { ep_out = addr; ep_out_mps = mps; } }
                }
            }
            i += dlen;
        }

        if iface < 0 || mode == 0 { return Err("desteklenmeyen USB cihaz"); }

        self.control(&mut dev, 0x00, 9, cfg_value as u16, 0, 0, 0)?;
        mdelay(5);

        dev.class = mode;
        dev.iface = iface as u8;

        if mode == CLS_HID {
            if ep_int_addr == 0 { return Err("HID endpoint yok"); }
            let _ = self.control(&mut dev, 0x21, 0x0B, 0, iface as u16, 0, 0); // SET_PROTOCOL(boot)
            let _ = self.control(&mut dev, 0x21, 0x0A, 0, iface as u16, 0, 0); // SET_IDLE

            let ep_num = (ep_int_addr & 0x0F) as u32;
            let dci = (ep_num * 2 + 1) as u8;
            let iring = Ring::new().ok_or("intr ring frame yok")?;
            let ival = interval_for(speed, ep_int_iv);
            let mps = if ep_int_mps == 0 { 8 } else { ep_int_mps };

            unsafe {
                zero_frame(input_ctx);
                let icc = self.ctx(input_ctx, 0);
                core::ptr::write_volatile(icc as *mut u32, 0);
                core::ptr::write_volatile((icc + 4) as *mut u32, 1u32 | (1u32 << dci));
                self.copy_slot_ctx(dev_ctx, input_ctx, dci);
                self.write_ep(input_ctx, dci, EPT_INTR_IN, mps, ival, iring.base,
                              (mps as u32) | ((mps as u32) << 16));
            }
            self.command(input_ctx, (TRB_CONFIG_EP << 10) | ((slot as u32) << 24))?;

            dev.intr = Some(iring);
            dev.dci = dci;
            dev.proto = proto;
            dev.buf_len = mps.min(SLOT_SIZE as u16);

            let blen = dev.buf_len as u32;
            if let Some(r) = dev.intr.as_mut() {
                for k in 0..QUEUE_DEPTH {
                    r.push(buf + (k * SLOT_SIZE) as u64, blen, (TRB_NORMAL << 10) | TRB_ISP | TRB_IOC);
                }
            }
            unsafe { self.ring_db(slot, dci as u32); }

        } else {
            // MASS STORAGE
            if ep_in == 0 || ep_out == 0 { return Err("no MSC bulk endpoint"); }

            let rin = Ring::new().ok_or("no bulk in ring")?;
            let rout = Ring::new().ok_or("no bulk out ring")?;
            let msc_cmd = pfa::alloc_frame().ok_or("no msc cmd frame")?;
            let msc_data = pfa::alloc_frame().ok_or("no msc data frame")?;
            unsafe { zero_frame(msc_cmd); zero_frame(msc_data); }

            let dci_in = ((ep_in & 0x0F) as u8) * 2 + 1;
            let dci_out = ((ep_out & 0x0F) as u8) * 2;
            let top = if dci_in > dci_out { dci_in } else { dci_out };

            unsafe {
                zero_frame(input_ctx);
                let icc = self.ctx(input_ctx, 0);
                core::ptr::write_volatile(icc as *mut u32, 0);
                core::ptr::write_volatile((icc + 4) as *mut u32,
                    1u32 | (1u32 << dci_in) | (1u32 << dci_out));
                self.copy_slot_ctx(dev_ctx, input_ctx, top);
                self.write_ep(input_ctx, dci_out, EPT_BULK_OUT, ep_out_mps, 0, rout.base, 512);
                self.write_ep(input_ctx, dci_in, EPT_BULK_IN, ep_in_mps, 0, rin.base, 512);
            }
            self.command(input_ctx, (TRB_CONFIG_EP << 10) | ((slot as u32) << 24))?;

            dev.bulk_in = Some(rin);
            dev.bulk_out = Some(rout);
            dev.dci_in = dci_in;
            dev.dci_out = dci_out;
            dev.ep_in_addr = ep_in;
            dev.ep_out_addr = ep_out;
            dev.msc_cmd = msc_cmd;
            dev.msc_data = msc_data;
        }

        self.devices[slot as usize] = Some(dev);
        Ok(())
    }

    unsafe fn copy_slot_ctx(&self, dev_ctx: u64, input_ctx: u64, entries: u8) {
        let src = self.ctx(dev_ctx, 0);
        let dst = self.ctx(input_ctx, 1);
        for k in 0..(self.ctx_size / 4) {
            let v = core::ptr::read_volatile((src + (k * 4) as u64) as *const u32);
            core::ptr::write_volatile((dst + (k * 4) as u64) as *mut u32, v);
        }
        let d0 = core::ptr::read_volatile(dst as *const u32);
        core::ptr::write_volatile(dst as *mut u32, (d0 & 0x07FF_FFFF) | ((entries as u32) << 27));
    }
}

// bulk transfer
impl Xhci {
    fn bulk_slot(&mut self, slot: u8, dir_in: bool, phys: u64, len: u32)
        -> Result<u32, &'static str>
    {
        let mut dev = self.devices[slot as usize].take().ok_or("slot is empty")?;
        let dci = if dir_in { dev.dci_in } else { dev.dci_out };
        if dci == 0 { self.devices[slot as usize] = Some(dev); return Err("bulk ep is empty"); }

        {
            let r = if dir_in { dev.bulk_in.as_mut() } else { dev.bulk_out.as_mut() };
            let Some(r) = r else {
                self.devices[slot as usize] = Some(dev);
                return Err("bulk ring yok");
            };
            r.push(phys, len, (TRB_NORMAL << 10) | TRB_ISP | TRB_IOC);
        }

        unsafe { self.ring_db(slot, dci as u32); }
        let res = self.wait_transfer(slot, dci);
        self.devices[slot as usize] = Some(dev);

        let ev = res?;
        let cc = ev.cc();
        if cc == 6 { return Err("STALL"); }
        if cc != 1 && cc != 13 { return Err("bulk transfer error"); }
        Ok(ev.resid())
    }

    fn recover_ep(&mut self, slot: u8, dir_in: bool) -> Result<(), &'static str> {
        let (dci, ep_addr, rbase) = {
            let d = self.devices[slot as usize].as_ref().ok_or("slot bos")?;
            if dir_in {
                (d.dci_in, d.ep_in_addr, d.bulk_in.as_ref().map(|r| r.base).unwrap_or(0))
            } else {
                (d.dci_out, d.ep_out_addr, d.bulk_out.as_ref().map(|r| r.base).unwrap_or(0))
            }
        };
        if dci == 0 || rbase == 0 { return Err("no ep"); }

        let _ = self.command(0, (TRB_RESET_EP << 10) | ((dci as u32) << 16) | ((slot as u32) << 24));
        let _ = self.control_slot(slot, 0x02, 0x01, 0, ep_addr as u16, 0, 0);

        if let Some(d) = self.devices[slot as usize].as_mut() {
            let r = if dir_in { d.bulk_in.as_mut() } else { d.bulk_out.as_mut() };
            if let Some(r) = r { r.reset(); }
        }

        self.command(rbase | 1,
            (TRB_SET_TR_DEQ << 10) | ((dci as u32) << 16) | ((slot as u32) << 24))?;
        Ok(())
    }
}

impl Xhci {
    fn service(&mut self) {
        let mut guard = 0;
        while let Some(trb) = self.evt.poll() {
            guard += 1;
            if guard > 128 { break; }
            self.handle_hid(trb);
        }
    }
}

pub fn poll() {
    if POLL_BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return;
    }
    unsafe {
        #[allow(static_mut_refs)]
        if let Some(x) = XHCI.as_mut() { x.service(); }
    }
    POLL_BUSY.store(false, Ordering::Release);
}

// api for storage.rs
pub fn bulk(slot: u8, dir_in: bool, phys: u64, len: u32) -> Result<u32, &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let x = XHCI.as_mut().ok_or("xhci yok")?;
        x.bulk_slot(slot, dir_in, phys, len)
    }
}

pub fn recover(slot: u8, dir_in: bool) -> Result<(), &'static str> {
    unsafe {
        #[allow(static_mut_refs)]
        let x = XHCI.as_mut().ok_or("xhci yok")?;
        x.recover_ep(slot, dir_in)
    }
}

pub fn ctrl(slot: u8, rt: u8, req: u8, val: u16, idx: u16, buf: u64, len: u16)
    -> Result<u32, &'static str>
{
    unsafe {
        #[allow(static_mut_refs)]
        let x = XHCI.as_mut().ok_or("xhci yok")?;
        x.control_slot(slot, rt, req, val, idx, buf, len)
    }
}

// (cmd_buf, data_buf)
pub fn msc_bufs(slot: u8) -> Option<(u64, u64)> {
    unsafe {
        #[allow(static_mut_refs)]
        let x = XHCI.as_ref()?;
        let d = x.devices[slot as usize].as_ref()?;
        if d.class != CLS_MSC { return None; }
        Some((d.msc_cmd, d.msc_data))
    }
}

pub fn msc_iface(slot: u8) -> u8 {
    unsafe {
        #[allow(static_mut_refs)]
        match XHCI.as_ref() {
            Some(x) => x.devices[slot as usize].as_ref().map(|d| d.iface).unwrap_or(0),
            None => 0,
        }
    }
}

// Freeze mass storage slots
pub fn storage_slots(out: &mut [u8; 4]) -> usize {
    unsafe {
        #[allow(static_mut_refs)]
        let Some(x) = XHCI.as_ref() else { return 0 };
        let mut n = 0usize;
        for s in 1..MAX_SLOTS_USED {
            if n >= 4 { break; }
            if let Some(d) = x.devices[s].as_ref() {
                if d.class == CLS_MSC { out[n] = d.slot; n += 1; }
            }
        }
        n
    }
}

pub fn device_count() -> usize {
    unsafe {
        #[allow(static_mut_refs)]
        match XHCI.as_ref() {
            Some(x) => x.devices.iter().filter(|d| d.is_some()).count(),
            None => 0,
        }
    }
}

pub fn info() -> Option<(u8, u8, usize)> {
    unsafe {
        #[allow(static_mut_refs)]
        XHCI.as_ref().map(|x| (x.max_slots, x.max_ports, x.ctx_size))
    }
}
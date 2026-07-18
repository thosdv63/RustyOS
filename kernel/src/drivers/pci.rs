use alloc::vec::Vec;

// PCI config base
static mut PCI_BASE: u64 = 0;

#[derive(Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,      // sinif kodu (0x01 = storage, 0x02 = network...)
    pub subclass: u8,   // alt sinif (0x06 = SATA/AHCI, 0x08 = NVMe...)
    pub prog_if: u8,    // programlama arayuzu
}

// PCIe ECAM
unsafe fn config_addr(bus: u8, dev: u8, func: u8, offset: u16) -> *mut u8 {
    let addr = PCI_BASE
        + ((bus as u64) << 20)
        + ((dev as u64) << 15)
        + ((func as u64) << 12)
        + offset as u64;
    addr as *mut u8
}

unsafe fn read_u16(bus: u8, dev: u8, func: u8, offset: u16) -> u16 {
    core::ptr::read_volatile(config_addr(bus, dev, func, offset) as *const u16)
}

unsafe fn read_u8(bus: u8, dev: u8, func: u8, offset: u16) -> u8 {
    core::ptr::read_volatile(config_addr(bus, dev, func, offset))
}

// Scan all pci bus, find devices
pub fn scan(pci_base: u64) -> Vec<PciDevice> {
    unsafe { PCI_BASE = pci_base; }
    let mut devices = Vec::new();

    unsafe {
        for bus in 0..=255u16 {
            for dev in 0..32u8 {
                for func in 0..8u8 {
                    let vendor = read_u16(bus as u8, dev, func, 0x00);
                    // 0xFFFF = no device
                    if vendor == 0xFFFF { continue; }

                    let device_id = read_u16(bus as u8, dev, func, 0x02);
                    let class = read_u8(bus as u8, dev, func, 0x0B);
                    let subclass = read_u8(bus as u8, dev, func, 0x0A);
                    let prog_if = read_u8(bus as u8, dev, func, 0x09);

                    devices.push(PciDevice {
                        bus: bus as u8, device: dev, function: func,
                        vendor_id: vendor, device_id, class, subclass, prog_if,
                    });

                    if func == 0 {
                        let header_type = read_u8(bus as u8, dev, func, 0x0E);
                        if header_type & 0x80 == 0 { break; }
                    }
                }
            }
        }
    }
    devices
}

// Translate class name
pub fn class_name(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x01, 0x06) => "SATA Controller (AHCI)",
        (0x01, 0x08) => "NVMe Controller",
        (0x01, 0x01) => "IDE Controller",
        (0x01, _)    => "Storage Controller",
        (0x02, _)    => "Network Controller",
        (0x03, _)    => "Display Controller",
        (0x0C, 0x03) => "USB Controller",
        (0x06, _)    => "Bridge",
        _            => "Diger",
    }
}

// Read a device's bar address (offset: 0x10=BAR0, 0x14=BAR1...)
pub fn read_bar(bus: u8, dev: u8, func: u8, bar_index: u8) -> u64 {
    unsafe {
        let offset = 0x10 + (bar_index as u16) * 4;
        let low = core::ptr::read_volatile(config_addr(bus, dev, func, offset) as *const u32);
        // is it 64-bit BAR? (bit 2-1 == 10)
        if (low & 0b110) == 0b100 {
            let high = core::ptr::read_volatile(config_addr(bus, dev, func, offset + 4) as *const u32);
            (((high as u64) << 32) | (low as u64)) & !0xF
        } else {
            (low as u64) & !0xF
        }
    }
}

// Enable bus mastering (for dma)
pub fn enable_bus_master(bus: u8, dev: u8, func: u8) {
    unsafe {
        let cmd = read_u16(bus, dev, func, 0x04);
        let new_cmd = cmd | 0x4 | 0x2; // bit2=bus master, bit1=memory space
        core::ptr::write_volatile(config_addr(bus, dev, func, 0x04) as *mut u16, new_cmd);
    }
}
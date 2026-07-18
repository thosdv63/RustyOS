use x86_64::instructions::port::Port;
use core::ptr::{read_volatile, write_volatile};

// I/O APIC Register Offsets (MMIO)
const IOREGSEL: usize = 0x00; // Register Selector
const IOWIN:    usize = 0x10; // Data Window

// I/O APIC Internal Register Addresses
const IOAPICID:  u32 = 0x00;
const IOAPICVER: u32 = 0x01;
const IOAPICARB: u32 = 0x02;
const IOREDTBL:  u32 = 0x10; // Redirection Table startup (2 registers for each IRQ)

pub struct IoApic {
    base_addr: usize,
}

impl IoApic {
    pub fn new(base_addr: usize) -> Self {
        Self { base_addr }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        let io_reg_sel = (self.base_addr + IOREGSEL) as *mut u32;
        let io_win = (self.base_addr + IOWIN) as *const u32;
        
        write_volatile(io_reg_sel, reg);
        read_volatile(io_win)
    }

    unsafe fn write(&self, reg: u32, value: u32) {
        let io_reg_sel = (self.base_addr + IOREGSEL) as *mut u32;
        let io_win = (self.base_addr + IOWIN) as *mut u32;
        
        write_volatile(io_reg_sel, reg);
        write_volatile(io_win, value);
    }

    // It connects a specific IRQ line (e.g., Keyboard = IRQ 1) to a vector in the IDT
    pub unsafe fn route(&self, irq: u32, vector: u8) {
        // Each IRQ has two consecutive 32-bit registers in the Redirection Table
        let low_reg = IOREDTBL + (irq * 2);
        let high_reg = low_reg + 1;

        self.write(high_reg, 0x00000000);

        // Bottom 32-bit: Interrupt settings
        // Bits 0-7: IDT Vector Number (e.g., 0x21)
        // Bit 16: Mask bit (0 = Interrupt enabled, active)
        let low_val = vector as u32;
        self.write(low_reg, low_val);
    }
}

pub unsafe fn disable_pic() {
    let mut cmd_master = Port::new(0x20);
    let mut data_master = Port::new(0x21);
    let mut cmd_slave = Port::new(0xA0);
    let mut data_slave = Port::new(0xA1);

    // ICW1: Put the PICs into boot mode
    cmd_master.write(0x11_u8);
    cmd_slave.write(0x11_u8);

    // ICW2: Change offsets (Remap)
    // We move the Master PIC to 0x20 and the Slave to 0x28 so that they don't clash with the first 32 error vectors
    data_master.write(0x20_u8);
    data_slave.write(0x28_u8);

    // ICW3: Cascade settings
    data_master.write(0x04_u8);
    data_slave.write(0x02_u8);

    // ICW4: 8086/88 mode
    data_master.write(0x01_u8);
    data_slave.write(0x01_u8);

    // NOW completely mask them all (Put them to sleep)
    data_master.write(0xFF_u8);
    data_slave.write(0xFF_u8);
}
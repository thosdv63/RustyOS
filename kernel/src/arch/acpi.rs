use acpi::{AcpiTables, Handler, PhysicalMapping};
use acpi::platform::{AcpiPlatform, InterruptModel, PciConfigRegions};
use core::ptr::NonNull;

#[derive(Clone)]
pub struct RustyAcpiHandler;

impl Handler for RustyAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        phys_addr: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let virt = NonNull::new(phys_addr as *mut T).unwrap();
        
        // updated due to the compiler
        PhysicalMapping {
            physical_start: phys_addr,
            virtual_start: virt,
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        // identity mapping, we dont need to do anything
    }

    // The AML functions that come with the new ACPI version and are mandatory to implement
    fn read_u8(&self, _: usize) -> u8 { todo!() }
    fn read_u16(&self, _: usize) -> u16 { todo!() }
    fn read_u32(&self, _: usize) -> u32 { todo!() }
    fn read_u64(&self, _: usize) -> u64 { todo!() }
    fn write_u8(&self, _: usize, _: u8) { todo!() }
    fn write_u16(&self, _: usize, _: u16) { todo!() }
    fn write_u32(&self, _: usize, _: u32) { todo!() }
    fn write_u64(&self, _: usize, _: u64) { todo!() }
    fn read_io_u8(&self, _: u16) -> u8 { todo!() }
    fn read_io_u16(&self, _: u16) -> u16 { todo!() }
    fn read_io_u32(&self, _: u16) -> u32 { todo!() }
    fn write_io_u8(&self, _: u16, _: u8) { todo!() }
    fn write_io_u16(&self, _: u16, _: u16) { todo!() }
    fn write_io_u32(&self, _: u16, _: u32) { todo!() }
    fn read_pci_u8(&self, _: acpi::PciAddress, _: u16) -> u8 { todo!() }
    fn read_pci_u16(&self, _: acpi::PciAddress, _: u16) -> u16 { todo!() }
    fn read_pci_u32(&self, _: acpi::PciAddress, _: u16) -> u32 { todo!() }
    fn write_pci_u8(&self, _: acpi::PciAddress, _: u16, _: u8) { todo!() }
    fn write_pci_u16(&self, _: acpi::PciAddress, _: u16, _: u16) { todo!() }
    fn write_pci_u32(&self, _: acpi::PciAddress, _: u16, _: u32) { todo!() }
    fn nanos_since_boot(&self) -> u64 { todo!() }
    fn stall(&self, _: u64) { todo!() }
    fn sleep(&self, _: u64) { todo!() }
    fn create_mutex(&self) -> acpi::Handle { todo!() }
    
    // AmlError path updated
    fn acquire(&self, _: acpi::Handle, _: u16) -> Result<(), acpi::aml::AmlError> { todo!() }
    fn release(&self, _: acpi::Handle) { todo!() }
}

pub struct AcpiInfo {
    pub local_apic_addr: u64, 
    pub io_apic_addr: u64,
    pub cpu_count: usize,
    pub pci_config_addr: u64,
    pub pm1a_cnt_blk: u16,
    pub pm1b_cnt_blk: u16,
    pub dsdt: &'static [u8],
}

static mut ACPI_INFO: Option<AcpiInfo> = None;

pub fn s5_info() -> Option<(u16, u16, &'static [u8])> {
    unsafe {
        #[allow(static_mut_refs)]
        ACPI_INFO.as_ref().and_then(|info| {
            // For the S5 package, both the pm1a port (different from 0) and the DSDT must be ready
            if info.pm1a_cnt_blk != 0 && !info.dsdt.is_empty() {
                Some((info.pm1a_cnt_blk, info.pm1b_cnt_blk, info.dsdt))
            } else {
                None
            }
        })
    }
}

pub fn init(rsdp_addr: u64) -> Result<(), &'static str> {
    if rsdp_addr == 0 {
        return Err("No RSDP address");
    }

    let tables = unsafe {
        AcpiTables::from_rsdp(RustyAcpiHandler, rsdp_addr as usize)
            .map_err(|_| "ACPI tables could not be parsed")?
    };

    // AcpiPlatform takes ownership (moves) of the tables and also requires a handler
    // Since RustyAcpiHandler is a zero-sized clone, giving away a new one is free
    let platform = AcpiPlatform::new(tables, RustyAcpiHandler)
        .map_err(|_| "platform info could not be obtained")?;

    let mut local_apic_addr = 0u64;
    let mut io_apic_addr = 0u64;
    let mut cpu_count = 0usize;

    // Referencing match: it reads the platform without breaking it down
    if let InterruptModel::Apic(apic) = &platform.interrupt_model {
        local_apic_addr = apic.local_apic_address;
        if let Some(ioapic) = apic.io_apics.first() {
            io_apic_addr = ioapic.address as u64;
        }
    }
    if let Some(info) = &platform.processor_info {
        cpu_count = 1 + info.application_processors.len();
    }

    // MCFG: Tables now reside within platform.tables; we get references from there.
    let mut pci_config_addr = 0u64;
    if let Ok(pci_regions) = PciConfigRegions::new(&platform.tables) {
        if let Some(region) = pci_regions.physical_address(0, 0, 0, 0) {
            pci_config_addr = region;
        }
    }

    let mut pm1a_cnt_blk = 0;
    let mut pm1b_cnt_blk = 0;
    let mut dsdt_slice: &'static [u8] = &[];

    // Find FADT and get pm1 control blocks
    if let Some(fadt) = platform.tables.find_table::<acpi::sdt::fadt::Fadt>() {
        
        // pm1a definitely exists, we're just taking the GenericAddress directly from within it
        if let Ok(pm1a) = fadt.pm1a_control_block() {
            pm1a_cnt_blk = pm1a.address as u16;
        }
        
        // pm1b is optional in the hardware, so it has to be matched with Ok(Some(...))
        if let Ok(Some(pm1b)) = fadt.pm1b_control_block() {
            pm1b_cnt_blk = pm1b.address as u16;
        }
    }

    // Pull the DSDT and translate the physical address in memory into a slice (Identity mapped)
    // dsdt() is a method that returns a result, so we use it with `()` and `ok()`.
    if let Ok(dsdt) = platform.tables.dsdt() {
        dsdt_slice = unsafe { 
            core::slice::from_raw_parts(dsdt.phys_address as *const u8, dsdt.length as usize) 
        };
    }

    unsafe {
        ACPI_INFO = Some(AcpiInfo {
            local_apic_addr,
            io_apic_addr,
            cpu_count,
            pci_config_addr,
            pm1a_cnt_blk,
            pm1b_cnt_blk,
            dsdt: dsdt_slice,
        });
    }

    Ok(())
}

pub fn info() -> Option<&'static AcpiInfo> {
    unsafe {
        #[allow(static_mut_refs)]
        ACPI_INFO.as_ref()
    }
}
use x86_64::structures::paging::{
    OffsetPageTable, PageTable, Page, PhysFrame, Mapper, Size4KiB,
    PageTableFlags as Flags,
};
use x86_64::{VirtAddr, PhysAddr};
use x86_64::registers::control::Cr3;
use crate::mm::pfa::PfaWrapper;

// Physical memory access offset. 
// If UEFI identity mapping is used, offset = 0 (virtual = physical).
const PHYS_OFFSET: u64 = 0;

// Read active PML4 and create OffsetPageTable
unsafe fn active_page_table() -> OffsetPageTable<'static> {
    let (pml4_frame, _) = Cr3::read();
    let phys = pml4_frame.start_address().as_u64();
    let virt = VirtAddr::new(phys + PHYS_OFFSET);
    let pml4: *mut PageTable = virt.as_mut_ptr();
    OffsetPageTable::new(&mut *pml4, VirtAddr::new(PHYS_OFFSET))
}

// Map a virtual page to a physical frame
pub fn map_page(virt_addr: u64, phys_addr: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        // Read the CR0 register and clear the Write Protect (WP) bit
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut flags = old_cr0;
        flags.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(flags);

        // Make normal mappings
        let mut mapper = active_page_table();
        let mut pfa = PfaWrapper;

        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(phys_addr));

        let mut page_flags = Flags::PRESENT;
        if writable { page_flags |= Flags::WRITABLE; }

        let result = match mapper.map_to(page, frame, page_flags, &mut pfa) {
            Ok(tlb) => { tlb.flush(); Ok(()) }
            Err(_) => Err("map_to basarisiz"),
        };

        // Restore CR0 WP bit (Re-enable blinding)
        x86_64::registers::control::Cr0::write(old_cr0);

        result
    }
}

// Remove mapping (disconnect the virtual page)
pub fn unmap_page(virt_addr: u64) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));

        let result = match mapper.unmap(page) {
            Ok((_frame, tlb)) => { tlb.flush(); Ok(()) }
            Err(_) => Err("unmap basarisiz"),
        };

        x86_64::registers::control::Cr0::write(old_cr0);
        result
    }
}

// Map consecutive pages to a virtual range (for heap)
// Page_count pages starting from virt_start, a new frame from PFA to each
pub fn map_range(virt_start: u64, page_count: u64, writable: bool) -> Result<(), &'static str> {
    for i in 0..page_count {
        let virt = virt_start + i * 0x1000;
        // Get a new physical frame from PFA for each page
        let frame = crate::mm::pfa::alloc_frame().ok_or("frame yok (map_range)")?;
        map_page(virt, frame, writable)?;
    }
    Ok(())
}

// Which physical address does the virtual address point to? (debug/verify)
pub fn translate(virt_addr: u64) -> Option<u64> {
    use x86_64::structures::paging::Translate;
    unsafe {
        let mapper = active_page_table();
        mapper.translate_addr(VirtAddr::new(virt_addr)).map(|p| p.as_u64())
    }
}

pub fn init() {
    // We are currently only using the existing table, no extra setup is needed. 
    // We will come back later when we set up our own address space.
}

// Map pages accessible from user mode (USER_ACCESSIBLE flag)
pub fn map_range_user(virt_start: u64, page_count: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let mut pfa = PfaWrapper;

        for i in 0..page_count {
            let virt = virt_start + i * 0x1000;
            let frame_addr = crate::mm::pfa::alloc_frame().ok_or("frame yok")?;
            let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
            let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(frame_addr));
            let mut flags = Flags::PRESENT | Flags::USER_ACCESSIBLE;
            if writable { flags |= Flags::WRITABLE; }
            let _ = mapper.map_to(page, frame, flags, &mut pfa).map(|t| t.flush());
        }
        x86_64::registers::control::Cr0::write(old_cr0);
        Ok(())
    }
}

// Add USER_ACCESSIBLE flag to an existing page (for already mapped page)
pub fn remap_user(virt_addr: u64) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));

        // USER_ACCESSIBLE is being added to the existing flags
        use x86_64::structures::paging::mapper::Mapper;
        let result = mapper.update_flags(
            page,
            Flags::PRESENT | Flags::WRITABLE | Flags::USER_ACCESSIBLE,
        );
        let r = match result {
            Ok(tlb) => { tlb.flush(); Ok(()) }
            Err(_) => Err("update_flags basarisiz"),
        };

        x86_64::registers::control::Cr0::write(old_cr0);
        r
    }
}

pub fn map_page_user(virt: u64, phys: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let mut pfa = PfaWrapper;
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(phys));
        let mut flags = Flags::PRESENT | Flags::USER_ACCESSIBLE;
        if writable { flags |= Flags::WRITABLE; }
        let _ = mapper.unmap(page).map(|(_, t)| t.flush());
        let _ = mapper.map_to(page, frame, flags, &mut pfa).map(|t| t.flush());

        x86_64::registers::control::Cr0::write(old_cr0);
        Ok(())
    }
}
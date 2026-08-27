use x86_64::structures::paging::{
    OffsetPageTable, PageTable, Page, PhysFrame, Mapper, Size4KiB,
    PageTableFlags as Flags,
};
use x86_64::{VirtAddr, PhysAddr};
use x86_64::registers::control::Cr3; 
use crate::mm::pfa::PfaWrapper;

const PHYS_OFFSET: u64 = 0;

unsafe fn active_page_table() -> OffsetPageTable<'static> {
    let (pml4_frame, _) = Cr3::read();
    let phys = pml4_frame.start_address().as_u64();
    let virt = VirtAddr::new(phys + PHYS_OFFSET);
    let pml4: *mut PageTable = virt.as_mut_ptr();
    OffsetPageTable::new(&mut *pml4, VirtAddr::new(PHYS_OFFSET))
}

pub fn map_page(virt_addr: u64, phys_addr: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut flags = old_cr0;
        flags.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(flags);

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

        x86_64::registers::control::Cr0::write(old_cr0);

        result
    }
}

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

pub fn map_range(virt_start: u64, page_count: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let mut pfa = PfaWrapper;
        let mut page_flags = Flags::PRESENT;
        if writable { page_flags |= Flags::WRITABLE; }

        let mut result = Ok(());

        for i in 0..page_count {
            let virt = virt_start + i * 0x1000;
            if let Some(frame_addr) = crate::mm::pfa::alloc_frame() {
                let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
                let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(frame_addr));
                let _ = mapper.map_to(page, frame, page_flags, &mut pfa).map(|t| t.flush());
            } else {
                result = Err("frame yok (map_range)");
                break;
            }
        }

        x86_64::registers::control::Cr0::write(old_cr0);
        result
    }
}

pub fn translate(virt_addr: u64) -> Option<u64> {
    use x86_64::structures::paging::Translate;
    unsafe {
        let mapper = active_page_table();
        mapper.translate_addr(VirtAddr::new(virt_addr)).map(|p| p.as_u64())
    }
}

pub fn init() {}

pub fn map_range_user(virt_start: u64, page_count: u64, writable: bool) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let mut pfa = PfaWrapper;
        let mut flags = Flags::PRESENT | Flags::USER_ACCESSIBLE;
        if writable { flags |= Flags::WRITABLE; }

        let mut result = Ok(());

        for i in 0..page_count {
            let virt = virt_start + i * 0x1000;
            if let Some(frame_addr) = crate::mm::pfa::alloc_frame() {
                let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
                let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(frame_addr));
                let _ = mapper.map_to(page, frame, flags, &mut pfa).map(|t| t.flush());
            } else {
                result = Err("frame yok");
                break;
            }
        }
        
        x86_64::registers::control::Cr0::write(old_cr0);
        result
    }
}

pub fn remap_user(virt_addr: u64) -> Result<(), &'static str> {
    unsafe {
        let old_cr0 = x86_64::registers::control::Cr0::read();
        let mut tmp = old_cr0;
        tmp.remove(x86_64::registers::control::Cr0Flags::WRITE_PROTECT);
        x86_64::registers::control::Cr0::write(tmp);

        let mut mapper = active_page_table();
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt_addr));

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

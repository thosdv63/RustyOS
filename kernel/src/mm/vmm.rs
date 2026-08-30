use crate::mm::pfa::{PFA, PAGE_SIZE, HHDM_OFFSET};
use x86_64::registers::control::Cr3;
use x86_64::registers::model_specific::{Efer, EferFlags};

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_WRITE_THROUGH: u64 = 1 << 3;
pub const PTE_NO_CACHE: u64 = 1 << 4; 
pub const PTE_HUGE_PAGE: u64 = 1 << 7; 
pub const PTE_NO_EXECUTE: u64 = 1 << 63; 

pub const PTE_ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn is_present(&self) -> bool { (self.0 & PTE_PRESENT) != 0 }
    pub fn is_huge(&self) -> bool { (self.0 & PTE_HUGE_PAGE) != 0 }
    
    pub fn set_entry(&mut self, phys_addr: u64, flags: u64) {
        self.0 = (phys_addr & PTE_ADDR_MASK) | flags;
    }

    pub fn phys_addr(&self) -> u64 { self.0 & PTE_ADDR_MASK }
    pub fn clear(&mut self) { self.0 = 0; }
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

pub struct PageTableManager {
    pml4_virt: *mut PageTable,
}

impl PageTableManager {
    pub fn active() -> Self {
        let (pml4_frame, _) = Cr3::read();
        let phys = pml4_frame.start_address().as_u64();
        let virt = (phys + HHDM_OFFSET) as *mut PageTable;
        Self { pml4_virt: virt }
    }

    fn next_table_alloc(&self, entry: &mut PageTableEntry, user: bool) -> *mut PageTable {
        if entry.is_present() {
            if user && (entry.0 & PTE_USER) == 0 {
                let phys = entry.phys_addr();
                let flags = entry.0 & 0xFFF;
                entry.set_entry(phys, flags | PTE_USER);
            }
            return (entry.phys_addr() + HHDM_OFFSET) as *mut PageTable;
        }

        if let Some(phys_addr) = PFA.lock().alloc_pages(0) {
            let virt_addr = (phys_addr + HHDM_OFFSET) as *mut PageTable;
            unsafe { core::ptr::write_bytes(virt_addr, 0, 1); }
            let flags = PTE_PRESENT | PTE_WRITABLE | PTE_USER;
            entry.set_entry(phys_addr, flags);
            virt_addr
        } else {
            panic!("Page Table Manager: Alt tablo icin fiziksel bellek kalmadi!");
        }
    }

    pub fn map(&mut self, virt: u64, phys: u64, size: u64, writable: bool, user: bool, disable_cache: bool, execute: bool) {
        let mut flags = PTE_PRESENT;
        if writable { flags |= PTE_WRITABLE; }
        if user { flags |= PTE_USER; }
        if !execute { flags |= PTE_NO_EXECUTE; }
        
        if disable_cache { 
            flags |= PTE_NO_CACHE | PTE_WRITE_THROUGH; 
        }

        let pml4_idx= ((virt >> 39) & 0x1FF) as usize;
        let pdpt_idx= ((virt >> 30) & 0x1FF) as usize;
        let pd_idx= ((virt >> 21) & 0x1FF) as usize;
        let pt_idx= ((virt >> 12) & 0x1FF) as usize;

        unsafe {
            let pml4 = &mut *self.pml4_virt;
            let pdpt_virt = self.next_table_alloc(&mut pml4.entries[pml4_idx], user);
            let pdpt = &mut *pdpt_virt;

            if size == 1024 * 1024 * 1024 {
                assert!(virt % size == 0 && phys % size == 0, "1GB Sayfa hizalamasi hatali!");
                pdpt.entries[pdpt_idx].set_entry(phys, flags | PTE_HUGE_PAGE);
                self.flush_tlb(virt);
                return;
            }

            let pd_virt = self.next_table_alloc(&mut pdpt.entries[pdpt_idx], user);
            let pd = &mut *pd_virt;

            if size == 2 * 1024 * 1024 {
                assert!(virt % size == 0 && phys % size == 0, "2MB Sayfa hizalamasi hatali!");
                pd.entries[pd_idx].set_entry(phys, flags | PTE_HUGE_PAGE);
                self.flush_tlb(virt);
                return;
            }

            let pt_virt = self.next_table_alloc(&mut pd.entries[pd_idx], user);
            let pt = &mut *pt_virt;

            pt.entries[pt_idx].set_entry(phys, flags);
            self.flush_tlb(virt);
        }
    }

    pub fn unmap(&mut self, virt: u64, size: u64) {
        let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
        let pd_idx   = ((virt >> 21) & 0x1FF) as usize;
        let pt_idx   = ((virt >> 12) & 0x1FF) as usize;

        unsafe {
            let pml4 = &mut *self.pml4_virt;
            if !pml4.entries[pml4_idx].is_present() { return; }
            
            let pdpt = &mut *((pml4.entries[pml4_idx].phys_addr() + HHDM_OFFSET) as *mut PageTable);
            if !pdpt.entries[pdpt_idx].is_present() { return; }

            if size == 1024 * 1024 * 1024 { 
                pdpt.entries[pdpt_idx].clear();
                self.flush_tlb(virt);
                return;
            }

            let pd = &mut *((pdpt.entries[pdpt_idx].phys_addr() + HHDM_OFFSET) as *mut PageTable);
            if !pd.entries[pd_idx].is_present() { return; }

            if size == 2 * 1024 * 1024 { 
                pd.entries[pd_idx].clear();
                self.flush_tlb(virt);
                return;
            }

            let pt = &mut *((pd.entries[pd_idx].phys_addr() + HHDM_OFFSET) as *mut PageTable);
            pt.entries[pt_idx].clear();
            self.flush_tlb(virt);
        }
    }

    pub fn translate(&self, virt: u64) -> Option<u64> {
        let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
        let pdpt_idx = ((virt >> 30) & 0x1FF) as usize;
        let pd_idx   = ((virt >> 21) & 0x1FF) as usize;
        let pt_idx   = ((virt >> 12) & 0x1FF) as usize;

        unsafe {
            let pml4 = &*self.pml4_virt;
            if !pml4.entries[pml4_idx].is_present() { return None; }

            let pdpt = &*((pml4.entries[pml4_idx].phys_addr() + HHDM_OFFSET) as *const PageTable);
            if !pdpt.entries[pdpt_idx].is_present() { return None; }
            if pdpt.entries[pdpt_idx].is_huge() {
                return Some(pdpt.entries[pdpt_idx].phys_addr() + (virt & 0x3FFF_FFFF)); 
            }

            let pd = &*((pdpt.entries[pdpt_idx].phys_addr() + HHDM_OFFSET) as *const PageTable);
            if !pd.entries[pd_idx].is_present() { return None; }
            if pd.entries[pd_idx].is_huge() {
                return Some(pd.entries[pd_idx].phys_addr() + (virt & 0x1F_FFFF)); 
            }

            let pt = &*((pd.entries[pd_idx].phys_addr() + HHDM_OFFSET) as *const PageTable);
            if !pt.entries[pt_idx].is_present() { return None; }
            
            Some(pt.entries[pt_idx].phys_addr() + (virt & 0xFFF)) 
        }
    }

    #[inline(always)]
    fn flush_tlb(&self, virt_addr: u64) {
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) virt_addr, options(nostack, preserves_flags));
        }
    }
}

pub fn map_range(virt_start: u64, pages: u64, writable: bool) -> Result<(), &'static str> {
    let mut ptm = PageTableManager::active();
    for i in 0..pages {
        let virt = virt_start + i * PAGE_SIZE;
        let phys_opt = PFA.lock().alloc_pages(0); 
        
        if let Some(phys) = phys_opt {
            ptm.map(virt, phys, PAGE_SIZE, writable, false, false, false);
        } else {
            return Err("Map Range: Fiziksel bellek yetersiz!");
        }
    }
    Ok(())
}

pub fn map_range_ex(virt_start: u64, pages: u64, writable: bool, user: bool, execute: bool) -> Result<(), &'static str> {
    let mut ptm = PageTableManager::active();
    for i in 0..pages {
        let virt = virt_start + i * PAGE_SIZE;
        let phys_opt = PFA.lock().alloc_pages(0);
        
        if let Some(phys) = phys_opt {
            ptm.map(virt, phys, PAGE_SIZE, writable, user, false, execute);
        } else {
            return Err("Map Range Ex: Fiziksel bellek yetersiz!");
        }
    }
    Ok(())
}

pub fn enable_nxe() {
    unsafe {
        Efer::update(|flags| flags.insert(EferFlags::NO_EXECUTE_ENABLE));
    }
}

pub fn init() {
    enable_nxe();
}

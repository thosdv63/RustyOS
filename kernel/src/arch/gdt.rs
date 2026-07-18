use x86_64::structures::gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use core::ptr::addr_of;
use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
use x86_64::instructions::tables::load_tss;

static mut TSS: TaskStateSegment = TaskStateSegment::new();

pub struct Selectors {
    pub code: SegmentSelector,
    pub data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub user_data: SegmentSelector,
    pub tss: SegmentSelector,
}

static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();
pub static mut SELECTORS: Option<Selectors> = None;

const STACK_SIZE: usize = 4096 * 4;
static mut DF_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
// Kernel stack to be used when ring3->ring0 transitions after a syscall/interrupt
static mut KERNEL_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

pub fn init() {
    unsafe {
        let stack_start = VirtAddr::from_ptr(addr_of!(DF_STACK));
        let stack_end = stack_start + STACK_SIZE as u64;
        TSS.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;

        // RSP0: Kernel stack to be used when switching from ring3 to ring0
        let kstack_start = VirtAddr::from_ptr(addr_of!(KERNEL_STACK));
        TSS.privilege_stack_table[0] = kstack_start + STACK_SIZE as u64;

        let code_sel = GDT.append(Descriptor::kernel_code_segment());
        let data_sel = GDT.append(Descriptor::kernel_data_segment());
        // User segments (ring 3, DPL=3)
        let user_data_sel = GDT.append(Descriptor::user_data_segment());
        let user_code_sel = GDT.append(Descriptor::user_code_segment());
        let tss_sel = GDT.append(Descriptor::tss_segment(&*addr_of!(TSS)));

        SELECTORS = Some(Selectors {
            code: code_sel,
            data: data_sel,
            user_code: user_code_sel,
            user_data: user_data_sel,
            tss: tss_sel,
        });

        GDT.load();
        let sel = SELECTORS.as_ref().unwrap();
        CS::set_reg(sel.code);
        load_tss(sel.tss);
        SS::set_reg(sel.data);
        DS::set_reg(sel.data);
        ES::set_reg(sel.data);
    }
}

// Update RSP0 of TSS (scheduler will call it every time a process is passed)
pub fn set_kernel_stack(stack_top: u64) {
    unsafe {
        TSS.privilege_stack_table[0] = VirtAddr::new(stack_top);
    }
}
/* NOTE:
The process structure here isn't currently in use,
Because I haven't yet created an ELF Loader. 
And I probably won't be adding it without help. 
If you have the necessary knowledge to help, 
please contact me.
*/

use alloc::string::String;
use x86_64::structures::paging::PhysFrame;

#[derive(Debug, Clone, Default)]
#[repr(C, packed)]
pub struct Registers {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rbp: u64, pub r8: u64,
    pub r9: u64,  pub r10: u64, pub r11: u64, pub r12: u64,
    pub r13: u64, pub r14: u64, pub r15: u64,
    
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[derive(Debug, PartialEq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Dead,
}

pub struct Process {
    pub pid: u64,
    pub name: String,
    pub cr3: PhysFrame, 
    pub kernel_stack_top: u64, 
    pub context: Registers,
    pub state: ProcessState,
}

impl Process {
    pub fn new(pid: u64, name: String, cr3: PhysFrame, kstack_top: u64, entry_point: u64, user_stack: u64) -> Self {
        let mut context = Registers::default();
        
        let sel = unsafe { crate::arch::gdt::SELECTORS.as_ref().unwrap() };
        context.cs = (sel.user_code.0 | 3) as u64; // RPL=3
        context.ss = (sel.user_data.0 | 3) as u64; // RPL=3
        context.rip = entry_point;
        context.rsp = user_stack;
        context.rflags = 0x202; // Interrupts on (IF=1)

        Self {
            pid,
            name,
            cr3,
            kernel_stack_top: kstack_top,
            context,
            state: ProcessState::Ready,
        }
    }
}
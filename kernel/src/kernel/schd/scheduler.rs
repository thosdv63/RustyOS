use alloc::vec::Vec;
use core::arch::asm;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub rsp: u64,
}

// Task durumu
#[derive(Clone, Copy, PartialEq)]
pub enum TaskState {
    Ready,    
    Running,  
    Finished,
}

// Bir task (gorev)
pub struct Task {
    pub id: usize,
    pub context: Context,
    pub state: TaskState,
    pub stack: Vec<u8>,
}

const STACK_SIZE: usize = 64 * 1024; // 64 KB stack per task

impl Task {
    // Create new task
    pub fn new(id: usize, entry: fn()) -> Task {
        let mut stack = alloc::vec![0u8; STACK_SIZE];

        let stack_top = stack.as_mut_ptr() as u64 + STACK_SIZE as u64;
        let stack_top = stack_top & !0xF;

        // Prepare the stack "as if it were already saved" 
        // We place the registers that will be popped in the first switch 
        // Order: It must be the REVERSE order of popping by context_switch.
        unsafe {
            let mut sp = stack_top as *mut u64;

            sp = sp.offset(-1);
            *sp = task_exit as u64;

            sp = sp.offset(-1);
            *sp = entry as u64;

            for _ in 0..6 {
                sp = sp.offset(-1);
                *sp = 0;
            }

            Task {
                id,
                context: Context { rsp: sp as u64 },
                state: TaskState::Ready,
                stack,
            }
        }
    }
}

extern "C" fn task_exit() {
    loop {
        // task bitti, baska task'a gecmeyi bekle
        unsafe { asm!("hlt"); }
    }
}

pub struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
    started: bool,
}

static mut SCHEDULER: Option<Scheduler> = None;

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            tasks: Vec::new(),
            current: 0,
            started: false,
        }
    }

    pub fn add_task(&mut self, entry: fn()) {
        let id = self.tasks.len();
        self.tasks.push(Task::new(id, entry));
    }
}

// Start global scheduler
pub fn init() {
    unsafe {
        SCHEDULER = Some(Scheduler::new());
    }
}

// Add task
pub fn spawn(entry: fn()) {
    unsafe {
        if let Some(sched) = SCHEDULER.as_mut() {
            sched.add_task(entry);
        }
    }
}

// when timer clicks two, this will called
pub fn schedule() {
    unsafe {
        let sched = match SCHEDULER.as_mut() {
            Some(s) => s,
            None => return,
        };
        if !sched.started || sched.tasks.len() < 2 {
            return;
        }

        let prev = sched.current;
        let next = (sched.current + 1) % sched.tasks.len();
        if prev == next { return; }

        // DEBUG
        let r = crate::renderer();
        use core::fmt::Write;
        r.set_color(0x00FF00FF);
        let _ = write!(r, "[{}->{}]", prev, next);

        sched.tasks[prev].state = TaskState::Ready;
        sched.tasks[next].state = TaskState::Running;
        sched.current = next;

        let prev_ctx = &mut sched.tasks[prev].context as *mut Context;
        let next_ctx = &sched.tasks[next].context as *const Context;

        context_switch(prev_ctx, next_ctx);
    }
}

// Context Switch (assembly)
// prev: context of the current task (RSP is saved here)
// next: context of the next task (RSP is loaded from here)
#[unsafe(naked)]
unsafe extern "C" fn context_switch(_prev: *mut Context, _next: *const Context) {
    core::arch::naked_asm!(
        // save Callee-saved registers (to current task's stack)
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Save current RSP to prev->rsp (rdi = prev)
        "mov [rdi], rsp",

        // Load new RSP to next->rsp (rsi = next)
        "mov rsp, [rsi]",

        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        "ret",
    );
}

// Jump first task
pub fn start() {
    unsafe {
        let sched = match SCHEDULER.as_mut() {
            Some(s) => s,
            None => return,
        };
        if sched.tasks.is_empty() { return; }

        sched.started = true;
        sched.current = 0;
        sched.tasks[0].state = TaskState::Running;

        let next_ctx = &sched.tasks[0].context as *const Context;
        let mut dummy = Context { rsp: 0 };
        context_switch(&mut dummy as *mut Context, next_ctx);
    }
}
// basic ring buffer
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Event {
    pub kind: u32,   // 1 = keyboard, 2 = mouse
    pub data1: i32,  // keyboard: char | mouse: x
    pub data2: i32,  // keyboard: 0 | mouse: y
    pub data3: i32,  // keyboard: 0 | mouse: buton bits
}

const QUEUE_SIZE: usize = 256;

static mut QUEUE: [Event; QUEUE_SIZE] = [Event { kind: 0, data1: 0, data2: 0, data3: 0 }; QUEUE_SIZE];
static mut HEAD: usize = 0; // writing position
static mut TAIL: usize = 0; // reading position

pub unsafe fn push(ev: Event) {
    let next = (HEAD + 1) % QUEUE_SIZE;
    if next == TAIL {
        TAIL = (TAIL + 1) % QUEUE_SIZE;
    }
    QUEUE[HEAD] = ev;
    HEAD = next;
}

pub unsafe fn pop() -> Option<Event> {
    if HEAD == TAIL {
        return None;
    }
    let ev = QUEUE[TAIL];
    TAIL = (TAIL + 1) % QUEUE_SIZE;
    Some(ev)
}

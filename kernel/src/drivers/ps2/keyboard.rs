use x86_64::instructions::port::Port;

fn wait_ib() { // Clear the input buffer (before writing the command)
    let mut st = Port::<u8>::new(0x64);
    let mut g = 0u32;
    unsafe { while st.read() & 0x02 != 0 { g += 1; if g > 500_000 { return; } } }
}
fn wait_ob() -> bool { // Let the exit buffer fill up (before reading)
    let mut st = Port::<u8>::new(0x64);
    let mut g = 0u32;
    unsafe { while st.read() & 0x01 == 0 { g += 1; if g > 500_000 { return false; } } }
    true
}
fn drain() {
    let mut st = Port::<u8>::new(0x64);
    let mut dp = Port::<u8>::new(0x60);
    let mut g = 0u32;
    unsafe { while st.read() & 0x01 != 0 { let _ = dp.read(); g += 1; if g > 4096 { return; } } }
}

// makes the ps/2 keyboard work.
// When typing the mouse init config byte, enter the keyboard IRQ (bit0) and set-1
pub fn init() {
    let mut cmd = Port::<u8>::new(0x64);
    let mut data = Port::<u8>::new(0x60);
    unsafe {
        drain();
        wait_ib(); cmd.write(0xAEu8); // open keyboard port
        drain();

        wait_ib(); cmd.write(0x20u8); // read config byte
        let mut cfg = if wait_ob() { data.read() } else { 0x45 };
        cfg |= 0x01; // bit0: open keyboard IRQ (IRQ1)
        cfg &= !0x10; // bit4: open keyboard clock (0 = open)
        cfg |= 0x40; // bit6: open set-2 -> set-1 translation (our table is set-1)
        wait_ib(); cmd.write(0x60u8); // write config byte
        wait_ib(); data.write(cfg);
        drain();

        wait_ib(); data.write(0xF4u8); // start scan
        for _ in 0..300_000 { core::hint::spin_loop(); }
        drain();
    }
}

static mut SHIFT_PRESSED: bool = false;
static mut CAPS_LOCK: bool = false;

// Normal Keys
const SCANCODE_TABLE: [char; 58] = [
    '\0', '\x1B', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\x08',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n', '\0',
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`', '\0', '\\',
    'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', '\0', '*', '\0', ' '
];

// Characters and symbols that appear when Shift is pressed
const SCANCODE_TABLE_SHIFTED: [char; 58] = [
    '\0', '\x1B', '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', '\x08',
    '\t', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', '\n', '\0',
    'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~', '\0', '|',
    'Z', 'X', 'C', 'V', 'B', 'N', 'M', '<', '>', '?', '\0', '*', '\0', ' '
];

pub unsafe fn handle_scancode(scancode: u8) {
    match scancode {
        // Shift keys were pressed (Left Shift: 0x2A, Right Shift: 0x36)
        0x2A | 0x36 => { SHIFT_PRESSED = true; return; }
        // Finger removed from Shift keys (Break Code = Make + 0x80)
        0xAA | 0xB6 => { SHIFT_PRESSED = false; return; }
        // Caps Lock Pressed (Only reverses the state when pressed)
        0x3A => { CAPS_LOCK = !CAPS_LOCK; return; }
        _ => {}
    }

    // We only capture make (key press) events
    if scancode < 0x80 {
        let index = scancode as usize;
        if index < SCANCODE_TABLE.len() {
            let ch = SCANCODE_TABLE[index];
            let shifted_ch = SCANCODE_TABLE_SHIFTED[index];

            // letter control
            let is_letter = ch >= 'a' && ch <= 'z';

            // determine which character to print on the screen.
            let final_char = if is_letter {
                // The letter takes into account both Caps Lock and Shift functions (XOR logic)
                if SHIFT_PRESSED ^ CAPS_LOCK { shifted_ch } else { ch }
            } else {
                // For symbols or numbers, just check the Shift key (Caps Lock doesn't affect numbers)
                if SHIFT_PRESSED { shifted_ch } else { ch }
            };
            
            if final_char != '\0' {
                // Put it in the event queue instead of typing it on the screen
                crate::kernel::pscy::event::push(crate::kernel::pscy::event::Event {
                    kind: 1, // keyboard
                    data1: final_char as i32,
                    data2: 0,
                    data3: 0,
                });
            }
        }
    }
}
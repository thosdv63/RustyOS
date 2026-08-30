use crate::drivers::usb::{PROTO_KEYBOARD, PROTO_MOUSE};
use crate::kernel::pscy::event::{self, Event};

static mut PREV_KEYS: [u8; 6] = [0; 6];
static mut CAPS: bool = false;

const KEYMAP: [(char, char); 0x39 - 0x04] = [
    ('a','A'),('b','B'),('c','C'),('d','D'),('e','E'),('f','F'),('g','G'),('h','H'),
    ('i','I'),('j','J'),('k','K'),('l','L'),('m','M'),('n','N'),('o','O'),('p','P'),
    ('q','Q'),('r','R'),('s','S'),('t','T'),('u','U'),('v','V'),('w','W'),('x','X'),
    ('y','Y'),('z','Z'),
    ('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),
    ('6','^'),('7','&'),('8','*'),('9','('),('0',')'),
    ('\n','\n'),       // 0x28 Enter
    ('\x1B','\x1B'),   // 0x29 Esc
    ('\x08','\x08'),   // 0x2A Backspace
    ('\t','\t'),       // 0x2B Tab
    (' ',' '),         // 0x2C Space
    ('-','_'),         // 0x2D
    ('=','+'),         // 0x2E
    ('[','{'),         // 0x2F
    (']','}'),         // 0x30
    ('\\','|'),        // 0x31
    ('\0','\0'),       // 0x32 non-US #
    (';',':'),         // 0x33
    ('\'','"'),        // 0x34
    ('`','~'),         // 0x35
    (',','<'),         // 0x36
    ('.','>'),         // 0x37
    ('/','?'),         // 0x38
];

fn usage_to_char(usage: u8, shift: bool, caps: bool) -> char {
    if usage < 0x04 || usage as usize >= 0x04 + KEYMAP.len() { return '\0'; }
    let (lo, hi) = KEYMAP[(usage - 0x04) as usize];
    let is_letter = lo >= 'a' && lo <= 'z';
    if is_letter {
        if shift ^ caps { hi } else { lo }
    } else if shift { hi } else { lo }
}

unsafe fn keyboard_report(d: &[u8]) {
    if d.len() < 8 { return; }
    let modi = d[0];
    let shift = (modi & 0x02) != 0 || (modi & 0x20) != 0;

    let caps_now = d[2..8].iter().any(|&k| k == 0x39);
    let caps_prev = PREV_KEYS.iter().any(|&k| k == 0x39);
    if caps_now && !caps_prev { CAPS = !CAPS; }

    for i in 0..6 {
        let k = d[2 + i];
        if k == 0 || k == 1 || k == 0x39 { continue; } // nop / rollover / capslock
        if PREV_KEYS.contains(&k) { continue; }        

        let ch = usage_to_char(k, shift, CAPS);
        if ch != '\0' {
            event::push(Event { kind: 1, data1: ch as i32, data2: 0, data3: 0 });
        }
    }

    for i in 0..6 { PREV_KEYS[i] = d[2 + i]; }
}

// Mouse Cursor
static mut MX: i32 = -1;
static mut MY: i32 = -1;

unsafe fn screen() -> (i32, i32) {
    let (_, w, h, _) = crate::FB_INFO;
    (w as i32, h as i32)
}

unsafe fn mouse_report(d: &[u8]) {
    if d.len() < 3 { return; }
    let (w, h) = screen();
    if w == 0 || h == 0 { return; }

    if MX < 0 { MX = w / 2; MY = h / 2; }

    let buttons = (d[0] & 0x07) as i32;
    let dx = d[1] as i8 as i32;
    let dy = d[2] as i8 as i32;

    MX += dx;
    MY += dy; // HID: down = positive (inverse of PS/2)

    if MX < 0 { MX = 0; }
    if MY < 0 { MY = 0; }
    if MX > w - 1 { MX = w - 1; }
    if MY > h - 1 { MY = h - 1; }

    event::push(Event { kind: 2, data1: MX, data2: MY, data3: buttons });
}

pub fn on_report(proto: u8, data: &[u8]) {
    unsafe {
        match proto {
            PROTO_KEYBOARD => keyboard_report(data),
            PROTO_MOUSE => mouse_report(data),
            _ => {}
        }
    }
}

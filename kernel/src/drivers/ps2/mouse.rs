use x86_64::instructions::port::Port;

// Mouse cursor matrix (12x19)
// 0: Transparent, 1: White Inner Filling, 2: Black Frame
const CURSOR_WIDTH: usize = 12;
static mut MOUSE_COLOR: u32 = 0x00FFFFFF; // White at start
const CURSOR_HEIGHT: usize = 19;
const CURSOR_MAP: [[u8; CURSOR_WIDTH]; CURSOR_HEIGHT] = [
    [2,2,0,0,0,0,0,0,0,0,0,0],
    [2,1,2,0,0,0,0,0,0,0,0,0],
    [2,1,1,2,0,0,0,0,0,0,0,0],
    [2,1,1,1,2,0,0,0,0,0,0,0],
    [2,1,1,1,1,2,0,0,0,0,0,0],
    [2,1,1,1,1,1,2,0,0,0,0,0],
    [2,1,1,1,1,1,1,2,0,0,0,0],
    [2,1,1,1,1,1,1,1,2,0,0,0],
    [2,1,1,1,1,1,1,1,1,2,0,0],
    [2,1,1,1,1,1,1,1,1,1,2,0],
    [2,1,1,1,1,1,2,2,2,2,2,2],
    [2,1,1,2,1,1,2,0,0,0,0,0],
    [2,1,2,0,2,1,1,2,0,0,0,0],
    [2,2,0,0,2,1,1,2,0,0,0,0],
    [0,0,0,0,0,2,1,1,2,0,0,0],
    [0,0,0,0,0,2,1,1,2,0,0,0],
    [0,0,0,0,0,0,2,1,1,2,0,0],
    [0,0,0,0,0,0,2,1,1,2,0,0],
    [0,0,0,0,0,0,0,2,2,0,0,0],
];

// The mouse cursor's current coordinates
static mut MOUSE_X: i32 = 400;
static mut MOUSE_Y: i32 = 300;

// Old mouse cursor coordinates
static mut OLD_X: i32 = 400;
static mut OLD_Y: i32 = 300;
static mut FIRST_DRAW: bool = true;

static mut BACK_BUFFER: [u32; CURSOR_WIDTH * CURSOR_HEIGHT] = [0; CURSOR_WIDTH * CURSOR_HEIGHT];

unsafe fn mouse_wait(type_bit: u8) {
    let mut timeout = 100_000;
    let mut port = Port::<u8>::new(0x64);
    while timeout > 0 {
        let status = port.read();
        if type_bit == 0 && (status & 1) == 1 { return; } // Ready for reading
        if type_bit == 1 && (status & 2) == 0 { return; } // Ready for writing
        timeout -= 1;
    }
}

pub unsafe fn poll() {
    let mut status_port = Port::<u8>::new(0x64);
    while (status_port.read() & 0x21) == 0x21 {
        handle_interrupt();
    }
}

unsafe fn mouse_write(cmd: u8) {
    mouse_wait(1);
    Port::<u8>::new(0x64).write(0xD4); // specify that we will send a command to the mouse
    mouse_wait(1);
    Port::<u8>::new(0x60).write(cmd);
}

unsafe fn mouse_read() -> u8 {
    mouse_wait(0);
    Port::<u8>::new(0x60).read()
}

pub unsafe fn init() {
    let mut cmd_port = Port::<u8>::new(0x64);
    let mut data_port = Port::<u8>::new(0x60);

    mouse_wait(1);
    cmd_port.write(0xA8);

    mouse_wait(1);
    cmd_port.write(0x20);
    mouse_wait(0);
    let mut status = data_port.read();
    status |= 2; 
    status &= !0x20; 
    
    mouse_wait(1);
    cmd_port.write(0x60);
    mouse_wait(1);
    data_port.write(status);

    mouse_write(0xF6);
    let _ = mouse_read(); 

    mouse_write(0xF4); 
    let _ = mouse_read();
}

unsafe fn render_mouse() {
    let renderer = crate::renderer();
    let fb_width = renderer.width as i32;
    let fb_height = renderer.height as i32;

    if MOUSE_X < 0 { MOUSE_X = 0; }
    if MOUSE_Y < 0 { MOUSE_Y = 0; }
    if MOUSE_X > fb_width - CURSOR_WIDTH as i32 { MOUSE_X = fb_width - CURSOR_WIDTH as i32; }
    if MOUSE_Y > fb_height - CURSOR_HEIGHT as i32 { MOUSE_Y = fb_height - CURSOR_HEIGHT as i32; }

    if !FIRST_DRAW {
        for cy in 0..CURSOR_HEIGHT {
            for cx in 0..CURSOR_WIDTH {
                if CURSOR_MAP[cy][cx] != 0 {
                    let ox = OLD_X + cx as i32;
                    let oy = OLD_Y + cy as i32;
                    let bg_color = BACK_BUFFER[cy * CURSOR_WIDTH + cx];
                    renderer.put_pixel(ox as usize, oy as usize, bg_color);
                }
            }
        }
    } else {
        FIRST_DRAW = false;
    }

    for cy in 0..CURSOR_HEIGHT {
        for cx in 0..CURSOR_WIDTH {
            let nx = MOUSE_X + cx as i32;
            let ny = MOUSE_Y + cy as i32;
            let current_pixel = renderer.get_pixel(nx as usize, ny as usize);
            BACK_BUFFER[cy * CURSOR_WIDTH + cx] = current_pixel;
        }
    }

    for cy in 0..CURSOR_HEIGHT {
        for cx in 0..CURSOR_WIDTH {
            let nx = MOUSE_X + cx as i32;
            let ny = MOUSE_Y + cy as i32;
            let pixel_type = CURSOR_MAP[cy][cx];

            if pixel_type == 1 {
                renderer.put_pixel(nx as usize, ny as usize, MOUSE_COLOR);
            } else if pixel_type == 2 {
                renderer.put_pixel(nx as usize, ny as usize, 0x00000000);
            }
        }
    }

    OLD_X = MOUSE_X;
    OLD_Y = MOUSE_Y;
}

static mut MOUSE_CYCLE: u8 = 0;
static mut MOUSE_PACKET: [u8; 3] = [0; 3];

pub unsafe fn handle_interrupt() {
    let mut data_port = Port::<u8>::new(0x60);
    let byte = data_port.read();

    match MOUSE_CYCLE {
        0 => {
            if (byte & 0x08) == 0x08 {
                MOUSE_PACKET[0] = byte;
                MOUSE_CYCLE = 1;
            }
        }
        1 => {
            MOUSE_PACKET[1] = byte;
            MOUSE_CYCLE = 2;
        }
        2 => {
            MOUSE_PACKET[2] = byte;
            MOUSE_CYCLE = 0;

            let flags = MOUSE_PACKET[0];
            let mut x_move = MOUSE_PACKET[1] as i32;
            let mut y_move = MOUSE_PACKET[2] as i32;

            if (flags & 0x01) == 0x01 {
                MOUSE_COLOR = 0x0000FF00; // Left Click: Green
            } else if (flags & 0x02) == 0x02 {
                MOUSE_COLOR = 0x00FF0000; // Right Click: Red
            } else if (flags & 0x04) == 0x04 {
                MOUSE_COLOR = 0x000000FF; // Middle Click: Blue
            } else {
                MOUSE_COLOR = 0x00FFFFFF; // No Click: White
            }

            if (flags & 0x10) == 0x10 { x_move |= !0xFF; }
            if (flags & 0x20) == 0x20 { y_move |= !0xFF; }

            MOUSE_X += x_move;
            MOUSE_Y -= y_move;

            if MOUSE_X < 0 { MOUSE_X = 0; }
            if MOUSE_Y < 0 { MOUSE_Y = 0; }
            if MOUSE_X > 1280 - 1 { MOUSE_X = 1280 - 1; }
            if MOUSE_Y > 800 - 1 { MOUSE_Y = 800 - 1; }

            let buttons = (flags & 0x07) as i32; // bit0=left, bit1=right, bit2=middle

            // push to event queue
            crate::kernel::pscy::event::push(crate::kernel::pscy::event::Event {
                kind: 2, // mouse
                data1: MOUSE_X,
                data2: MOUSE_Y,
                data3: buttons,
            });
        }
        _ => MOUSE_CYCLE = 0,
    }
}

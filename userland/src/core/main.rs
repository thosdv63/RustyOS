#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
extern crate alloc;

mod syscall;
#[path = "heap.rs"]
mod heap;
#[path = "../renderer/mod.rs"]
mod renderer;
#[path = "../ui/mod.rs"]
mod ui;
#[path = "../apps/mod.rs"]
mod apps;

use core::arch::naked_asm;
use core::panic::PanicInfo;
use renderer::Renderer;

const CURSOR_W: usize = 12;
const CURSOR_H: usize = 19;
const CURSOR: [[u8; CURSOR_W]; CURSOR_H] = [
    [2,2,0,0,0,0,0,0,0,0,0,0],[2,1,2,0,0,0,0,0,0,0,0,0],[2,1,1,2,0,0,0,0,0,0,0,0],
    [2,1,1,1,2,0,0,0,0,0,0,0],[2,1,1,1,1,2,0,0,0,0,0,0],[2,1,1,1,1,1,2,0,0,0,0,0],
    [2,1,1,1,1,1,1,2,0,0,0,0],[2,1,1,1,1,1,1,1,2,0,0,0],[2,1,1,1,1,1,1,1,1,2,0,0],
    [2,1,1,1,1,1,1,1,1,1,2,0],[2,1,1,1,1,1,2,2,2,2,2,2],[2,1,1,2,1,1,2,0,0,0,0,0],
    [2,1,2,0,2,1,1,2,0,0,0,0],[2,2,0,0,2,1,1,2,0,0,0,0],[0,0,0,0,0,2,1,1,2,0,0,0],
    [0,0,0,0,0,2,1,1,2,0,0,0],[0,0,0,0,0,0,2,1,1,2,0,0],[0,0,0,0,0,0,2,1,1,2,0,0],
    [0,0,0,0,0,0,0,2,2,0,0,0],
];

fn draw_cursor(r: &Renderer, mx: usize, my: usize) {
    for cy in 0..CURSOR_H { for cx in 0..CURSOR_W {
        match CURSOR[cy][cx] {
            1 => r.put_pixel(mx + cx, my + cy, 0x00FFFFFF),
            2 => r.put_pixel(mx + cx, my + cy, 0x00000000),
            _ => {}
        }
    }}
}

unsafe extern "C" {
    static __bss_start: u8;
    static _end: u8;
}

#[no_mangle]
#[link_section = ".text._start"]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
    "cld",
    "lea rdi, [rip + {bss_s}]",
    "lea rcx, [rip + {bss_e}]",
    "sub rcx, rdi",
    "xor eax, eax",
    "rep stosb",
    "and rsp, -16",
    "sub rsp, 8", // call sonrasında RSP % 16 == 0 kalması için zorunlu
    "xor ebp, ebp",
    "call {m}",
    "ud2",
    bss_s = sym __bss_start,
    bss_e = sym _end,
    m = sym userland_main,
);
}

extern "C" fn userland_main() -> ! {
    heap::init();
    syscall::sys_print(">>> [USERLAND] baslatiliyor...\n");

    let mut fb: [u64; 5] = [0; 5];
    syscall::sys_get_framebuffer(fb.as_mut_ptr() as u64);
    let base = unsafe { core::ptr::read_volatile(&fb[0]) };
    let width = unsafe { core::ptr::read_volatile(&fb[1]) };
    let height = unsafe { core::ptr::read_volatile(&fb[2]) };
    let stride = unsafe { core::ptr::read_volatile(&fb[3]) };
    let back = unsafe { core::ptr::read_volatile(&fb[4]) };
    let r = Renderer::new(base, width, height, stride, back);

    let mut mx: i32 = (width / 2) as i32;
    let mut my: i32 = (height / 2) as i32;

    if ui::oobe::needed() { ui::oobe::run(&r, width as usize, height as usize); }
    ui::login::run(&r, width as usize, height as usize);

    use ui::window_mgr::WindowManager;
    let mut wm = WindowManager::new(width as i32, height as i32);
    let mut am = ui::app_mgr::AppManager::new();
    ui::desktop_manager::init();

    let mut under: [u32; CURSOR_W * CURSOR_H] = [0; CURSOR_W * CURSOR_H];

    ui::desktop::draw(&r);
    ui::desktop_manager::draw_icons(&r);
    wm.draw(&r);
    am.draw_apps(&r, &wm);
    ui::taskbar::draw(&r);
    wm.draw_taskbar_buttons(&r);
    r.save_rect(mx as usize, my as usize, CURSOR_W, CURSOR_H, &mut under);
    draw_cursor(&r, mx as usize, my as usize);
    r.present();

    syscall::sys_play_startup();

    let mut event: [i32; 4] = [0; 4];
    let mut old_mx = mx as usize;
    let mut old_my = my as usize;

    loop {
        let mut redraw_all = false;
        let mut moved = false;
        while syscall::sys_poll_event(event.as_mut_ptr() as u64) == 1 {
            let kind = unsafe { core::ptr::read_volatile(&event[0]) };
            if kind == 2 {
                mx = unsafe { core::ptr::read_volatile(&event[1]) };
                my = unsafe { core::ptr::read_volatile(&event[2]) };
                let raw = unsafe { core::ptr::read_volatile(&event[3]) };
                let btn_down = raw & 1 == 1;
                let rbtn = raw & 2 == 2;
                if mx < 0 { mx = 0; }
                if my < 0 { my = 0; }
                if mx >= (width as i32) - CURSOR_W as i32 { mx = width as i32 - CURSOR_W as i32; }
                if my >= (height as i32) - CURSOR_H as i32 { my = height as i32 - CURSOR_H as i32; }

                if btn_down {
                    if let Some(wid) = wm.taskbar_button_at(mx, my) {
                        wm.restore_by_id(wid);
                        redraw_all = true;
                    } else if ui::taskbar_manager::handle_click(mx, my, height as usize, width as usize) {
                        if ui::taskbar_manager::take_open_window_request() {
                            am.launch_with_path(&mut wm,
                                ui::taskbar_manager::take_app_kind(),
                                ui::taskbar_manager::take_app_path());
                        }
                        redraw_all = true;
                    }
                }

                let menu_open = ui::taskbar_manager::is_menu_open();
                let mut left_used = false;
                if !menu_open {
                    if wm.handle_mouse(mx, my, btn_down) { redraw_all = true; left_used = true; }
                    if wm.is_busy() { left_used = true; }
                    if am.route_click(&wm, mx, my, btn_down, rbtn) { redraw_all = true; left_used = true; }
                    am.cleanup(&wm);
                    if let Some(kid) = ui::app_mgr::take_kill() {
                        wm.close_by_id(kid);
                        am.cleanup(&wm);
                        redraw_all = true;
                    }
                    if let Some((k, p)) = ui::app_mgr::take_app_request() {
                        am.launch_with_path(&mut wm, k, p);
                        redraw_all = true;
                    }
                }
                let block = left_used || wm.over_any(mx, my);
                if ui::desktop_manager::handle_mouse(mx, my, btn_down, rbtn, height as i32, menu_open, block) {
                    redraw_all = true;
                }
                if let Some((k, p)) = ui::desktop_manager::take_app_request() {
                    am.launch_with_path(&mut wm, k, p);
                    redraw_all = true;
                }
                moved = true;
            } else if kind == 1 {
                let ch = unsafe { core::ptr::read_volatile(&event[1]) };
                if ui::desktop_manager::handle_key(ch) { redraw_all = true; }
                else if am.route_key(&wm, ch) { redraw_all = true; }
            }
        }

        if redraw_all {
            ui::desktop::draw(&r);
            ui::desktop_manager::draw_icons(&r);
            ui::desktop_manager::draw_selection(&r);
            wm.draw(&r);
            am.draw_apps(&r, &wm);
            ui::taskbar::draw(&r);
            wm.draw_taskbar_buttons(&r);
            ui::taskbar_manager::draw_menu(&r, width as usize, height as usize);
            ui::desktop_manager::draw_context_menu(&r);
            let nx = mx as usize; let ny = my as usize;
            r.save_rect(nx, ny, CURSOR_W, CURSOR_H, &mut under);
            draw_cursor(&r, nx, ny);
            if let Some((dx, dy, dw, dh)) = wm.take_drag_dirty() {
    r.present_rect(dx as usize, dy as usize, dw as usize, dh as usize);
    r.present_rect(old_mx, old_my, CURSOR_W, CURSOR_H);
    r.present_rect(nx, ny, CURSOR_W, CURSOR_H);
} else {
    r.present();
}
            old_mx = nx; old_my = ny;
        } else if moved {
            let nx = mx as usize; let ny = my as usize;
            r.restore_rect(old_mx, old_my, CURSOR_W, CURSOR_H, &under);
            r.save_rect(nx, ny, CURSOR_W, CURSOR_H, &mut under);
            draw_cursor(&r, nx, ny);
            r.present_rect(old_mx, old_my, CURSOR_W, CURSOR_H);
            r.present_rect(nx, ny, CURSOR_W, CURSOR_H);
            old_mx = nx; old_my = ny;
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop {} }

#[alloc_error_handler]
fn alloc_error(_layout: core::alloc::Layout) -> ! { loop {} }

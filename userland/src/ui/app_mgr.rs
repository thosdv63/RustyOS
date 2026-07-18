use alloc::vec::Vec;
use alloc::boxed::Box;
use crate::apps::app_compiler::{App, AppEvent};
use crate::apps::apps::hakkinda::Hakkinda;
use crate::apps::apps::paint::Paint;
use crate::apps::apps::explorer::Explorer;
use crate::apps::apps::notepad::Notepad;
use crate::ui::window_mgr::WindowManager;
use crate::ui::window::{WinState, TITLE_H};
use crate::renderer::Renderer;
use alloc::string::String;

struct AppInstance {
    window_id: u32,
    app: Box<dyn App>,
}

pub struct AppManager {
    instances: Vec<AppInstance>,
    prev_btn: bool,
    prev_rbtn: bool,
}

impl AppManager {
    pub fn new() -> Self { AppManager { instances: Vec::new(), prev_btn: false, prev_rbtn: false } }

    pub fn launch(&mut self, wm: &mut WindowManager, app_kind: u32) {
        self.launch_with_path(wm, app_kind, String::new());
    }

    pub fn launch_with_path(&mut self, wm: &mut WindowManager, app_kind: u32, path: String) {
        let app: Box<dyn App> = match app_kind {
            2 => Box::new(Paint::new(path.clone())),
            3 => Box::new(Explorer::new_at(path)),
            4 => Box::new(crate::apps::apps::regedit::Regedit::new()),
            5 => Box::new(crate::apps::apps::gorevmgr::GorevMgr::new()),
            6 => Box::new(Notepad::new_at(path)),
            7 => Box::new(crate::apps::apps::cmd::Cmd::new()),
            8 => Box::new(crate::apps::apps::resim::Resim::new(path.clone())),
            9 => Box::new(crate::apps::apps::hesap::Hesap::new()),
            10 => Box::new(crate::apps::apps::ayarlar::Ayarlar::new()),
            _ => Box::new(Hakkinda::new()),
        };
        wm.open(app.title(), app_kind);
        if let Some(w) = wm.windows.last() {
            self.instances.push(AppInstance { window_id: w.id, app });
        }
    }

    pub fn cleanup(&mut self, wm: &WindowManager) {
        self.instances.retain(|i| wm.windows.iter().any(|w| w.id == i.window_id));
    }

    pub fn draw_apps(&mut self, r: &Renderer, wm: &WindowManager) {
        unsafe {
            #[allow(static_mut_refs)]
            let t = TASKS.get_or_insert_with(Vec::new);
            t.clear();
            for w in wm.windows.iter() { t.push((w.id, w.title.clone())); }
        }
        for win in wm.windows.iter() {
            if win.state == WinState::Minimized { continue; }
            if let Some(inst) = self.instances.iter_mut().find(|i| i.window_id == win.id) {
                let bx = (win.x + 4).max(0) as usize;
                let by = (win.y + TITLE_H + 4).max(0) as usize;
                let bw = (win.w - 8).max(0) as usize;
                let bh = (win.h - TITLE_H - 8).max(0) as usize;
                inst.app.draw(r, bx, by, bw, bh);
            }
        }
    }

    pub fn route_click(&mut self, wm: &WindowManager, mx: i32, my: i32, btn: bool, rbtn: bool) -> bool {
        let prev = self.prev_btn; self.prev_btn = btn;
        let prev_r = self.prev_rbtn; self.prev_rbtn = rbtn;
        let active = wm.active_id();
        if active == 0 { return false; }
        if let Some(win) = wm.windows.iter().find(|w| w.id == active) {
            if win.state == WinState::Minimized { return false; }
            let bx = win.x + 4;
            let by = win.y + TITLE_H + 4;
            let inside = mx >= bx && mx < bx + (win.w - 8) && my >= by && my < by + (win.h - TITLE_H - 8);
            if !inside { return false; }
            if let Some(inst) = self.instances.iter_mut().find(|i| i.window_id == active) {
                if rbtn && !prev_r {
                    return inst.app.on_event(&AppEvent::RClick { x: mx - bx, y: my - by });
                }
                if btn {
                    let ev = if !prev { AppEvent::Click { x: mx - bx, y: my - by } }
                             else { AppEvent::Drag { x: mx - bx, y: my - by } };
                    return inst.app.on_event(&ev);
                }
            }
        }
        false
    }

    pub fn route_key(&mut self, wm: &WindowManager, ch: i32) -> bool {
        let active = wm.active_id();
        if active == 0 { return false; }
        if let Some(inst) = self.instances.iter_mut().find(|i| i.window_id == active) {
            let c = match ch {
                32..=126 => ch as u8 as char,
                13 | 10 => '\n',
                27 => '\u{1b}',
                8 | 127 => '\u{8}',
                _ => return false,
            };
            return inst.app.on_event(&AppEvent::Key { ch: c });
        }
        false
    }
}

static mut TASKS: Option<Vec<(u32, String)>> = None;
static mut KILL: u32 = 0;
static mut APP_REQ: Option<(u32, String)> = None;

#[allow(static_mut_refs)]
pub fn tasks() -> &'static Vec<(u32, String)> {
    unsafe { TASKS.get_or_insert_with(Vec::new) }
}
pub fn request_kill(id: u32) { unsafe { KILL = id; } }
pub fn take_kill() -> Option<u32> {
    unsafe { if KILL == 0 { None } else { let k = KILL; KILL = 0; Some(k) } }
}

// Bir uygulama baska bir uygulamayi acmak isterse (Gezgin -> Not Defteri)
pub fn request_app(kind: u32, path: String) {
    unsafe { APP_REQ = Some((kind, path)); }
}
#[allow(static_mut_refs)]
pub fn take_app_request() -> Option<(u32, String)> {
    unsafe { APP_REQ.take() }
}
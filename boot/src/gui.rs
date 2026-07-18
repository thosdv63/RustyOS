use uefi::system;
use uefi::proto::console::text::{Color, Output};
use uefi::CString16;
use alloc::string::String;

pub const FG: Color = Color::LightGray;
pub const BG: Color = Color::Black;
pub const BAR_FG: Color = Color::Black;
pub const BAR_BG: Color = Color::LightGray;

static mut COLS: usize = 80;
static mut ROWS: usize = 25;

fn out<F: FnMut(&mut Output)>(f: F) {
    system::with_stdout(f);
}

pub fn init() {
    out(|o| {
        let _ = o.enable_cursor(false);
        let _ = o.set_color(FG, BG);
        let _ = o.clear();
        if let Ok(Some(m)) = o.current_mode() {
            unsafe { COLS = m.columns(); ROWS = m.rows(); }
        }
    });
}

pub fn cols() -> usize { unsafe { COLS } }
pub fn rows() -> usize { unsafe { ROWS } }

fn spaces(n: usize) -> String {
    let mut s = String::new();
    for _ in 0..n { s.push(' '); }
    s
}

fn put(o: &mut Output, col: usize, row: usize, s: &str) {
    let _ = o.set_cursor_position(col, row);
    if let Ok(cs) = CString16::try_from(s) {
        let _ = o.output_string(&cs);
    }
}

// Normal body text
pub fn text(col: usize, row: usize, s: &str) {
    out(|o| {
        let _ = o.set_color(FG, BG);
        put(o, col, row, s);
    });
}

// Full width gray header bar + centered text (1 column margin)
pub fn title_bar(row: usize, label: &str) {
    let w = cols().saturating_sub(2);
    out(|o| {
        let _ = o.set_color(BAR_FG, BAR_BG);
        put(o, 1, row, &spaces(w));
        let cx = 1 + w.saturating_sub(label.len()) / 2;
        put(o, cx, row, label);
        let _ = o.set_color(FG, BG);
    });
}

// Bottom grey bar: left / middle / right labels (ENTER=Select ...)
pub fn bottom_bar(row: usize, left: &str, mid: &str, right: &str) {
    let w = cols().saturating_sub(2);
    out(|o| {
        let _ = o.set_color(BAR_FG, BAR_BG);
        put(o, 1, row, &spaces(w));
        put(o, 3, row, left);
        put(o, 1 + w.saturating_sub(mid.len()) / 2, row, mid);
        put(o, (1 + w).saturating_sub(right.len() + 2), row, right);
        let _ = o.set_color(FG, BG);
    });
}

// Selection bar: gray bar + '>' at the right end if selected
pub fn entry(row: usize, label: &str, selected: bool) {
    let x0 = 4usize;
    let w = cols().saturating_sub(x0 * 2);
    out(|o| {
        if selected {
            let _ = o.set_color(BAR_FG, BAR_BG);
            put(o, x0, row, &spaces(w));
            put(o, x0 + 1, row, label);
            put(o, x0 + w - 1, row, ">");
        } else {
            let _ = o.set_color(FG, BG);
            put(o, x0, row, &spaces(w)); // clear old selection bar
            put(o, x0 + 1, row, label);
        }
        let _ = o.set_color(FG, BG);
    });
}

pub fn clear_row(row: usize) {
    let w = cols();
    out(|o| {
        let _ = o.set_color(FG, BG);
        put(o, 0, row, &spaces(w));
    });
}

pub fn clear_all() {
    out(|o| {
        let _ = o.set_color(FG, BG);
        let _ = o.clear();
    });
}
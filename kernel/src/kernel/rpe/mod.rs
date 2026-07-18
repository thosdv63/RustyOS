// ===========================================================
// RPE: Rusty Preinstallation Environment
// Payload embedded in kernel -> USB storage (xHCI) not required. // Flow: loading bar -> boot animation -> welcome ->
// PARTITION SELECTION (GPT) -> confirm -> installation -> reboot.
// ==============================================================
pub mod ui;
pub mod install;
pub mod gpt;
pub mod esp;

use core::sync::atomic::{AtomicBool, Ordering};
use core::fmt::Write;
use alloc::vec::Vec;
use alloc::string::String;

static RPE: AtomicBool = AtomicBool::new(false);
pub fn set_mode(v: bool) { RPE.store(v, Ordering::Relaxed); }
pub fn is_rpe() -> bool { RPE.load(Ordering::Relaxed) }

pub struct Row {
    pub header: bool,
    pub selectable: bool,
    pub protected: bool,
    pub line1: String,
    pub line2: String,
    pub disk_kind: u8,
    pub first_lba: u64,
    pub sectors: u64,
    pub part_index: u32,
}

static mut LOAD_PCT: u32 = 0;

pub fn loading_begin() {
    if !is_rpe() { return; }
    let r = unsafe { crate::renderer() };
    r.clear(0x00000000);
    let w = r.width; let h = r.height;
    let s = "Rusty dosyalari yukleniyor...";
    r.set_color(0x00D8D8D8);
    r.text_at(w.saturating_sub(s.len() * 18) / 2, h / 2 - 8, s);
    r.fill_rect(0, h - 30, w, 22, 0x00161616);
    r.draw_rect(0, h - 30, w, 22, 0x006A6A6A);
    unsafe { LOAD_PCT = 0; }
}

pub fn loading_progress(pct: u32) {
    if !is_rpe() { return; }
    let p = pct.min(100);
    unsafe { if p <= LOAD_PCT { return; } LOAD_PCT = p; }
    let r = unsafe { crate::renderer() };
    let w = r.width; let h = r.height;
    let fw = w.saturating_sub(4) * p as usize / 100;
    r.fill_rect(2, h - 28, fw, 18, 0x00A8A8A8);
    ui::delay_ms(60);
}

pub fn run() -> ! {
    x86_64::instructions::interrupts::disable();
    ui::flush_kbd();

    let rows = build_rows();
    let any_sel = rows.iter().any(|r| r.selectable);

    loop {
        ui::welcome();
        wait_enter();

        if !any_sel {
            ui::error_screen("Kurulabilecek bos FAT32 bolum yok. Once bir bolumu FAT32 hazirlayin.");
            wait_enter();
            continue;
        }

        loop {
            let sel = match partition_select(&rows) {
                Some(i) => i,
                None => break,                  // ESC -> hosgeldin
            };
            let row = &rows[sel];

            ui::confirm_part(row);
            if !wait_confirm() { continue; }    // ESC -> bolum listesi

            ui::install_begin();
            let mut cb = |s: usize, p: u32| ui::install_screen(s, p);
            match do_install(row, &mut cb) {
                Ok(()) => ui::done_and_reboot(row.part_index),
                Err(e) => { ui::error_screen(e); wait_enter(); }
            }
        }
    }
}

fn wait_enter() {
    loop { if ui::wait_key() == ui::K_ENTER { return; } }
}

fn wait_confirm() -> bool {
    loop {
        match ui::wait_key() {
            ui::K_ENTER => return true,
            ui::K_ESC => return false,
            _ => {}
        }
    }
}

// Sadece secilebilir satirlar arasinda gezer
fn partition_select(rows: &[Row]) -> Option<usize> {
    let mut sel = rows.iter().position(|r| r.selectable)?;
    loop {
        ui::partition_screen(rows, sel);
        match ui::wait_key() {
            ui::K_UP => {
                if let Some(p) = (0..sel).rev().find(|&i| rows[i].selectable) { sel = p; }
            }
            ui::K_DOWN => {
                if let Some(p) = ((sel + 1)..rows.len()).find(|&i| rows[i].selectable) { sel = p; }
            }
            ui::K_ENTER => return Some(sel),
            ui::K_ESC => return None,
            _ => {}
        }
    }
}

// Kurulum: gomulu payload -> secilen bolum + ESP. USB gerekmez.
fn do_install(row: &Row, cb: &mut dyn FnMut(usize, u32)) -> Result<(), &'static str> {
    let sectors32 = row.sectors.min(u32::MAX as u64) as u32;
    let total = match row.disk_kind {
        0 => crate::drivers::storage::nvme::info().map(|(_, bc)| bc).unwrap_or(0),
        _ => crate::drivers::storage::ahci::info().map(|(_, bc)| bc).unwrap_or(0),
    };
    let esp = gpt::find_esp(row.disk_kind, total);
    let mut pdev = crate::fs::offset::PartitionDevice::new(row.disk_kind, row.first_lba, row.sectors);
    install::install(&mut pdev, sectors32, row.disk_kind, esp, cb)
}

// Tum disklerin GPT'sini okuyup UI satirlarini olustur
fn build_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some((bs, bc)) = crate::drivers::storage::nvme::info() {
        push_disk(&mut rows, 0, "NVMe SSD", bs, bc);
    }
    if let Some((bs, bc)) = crate::drivers::storage::ahci::info() {
        push_disk(&mut rows, 1, "SATA Disk", bs, bc);
    }
    rows
}

fn push_disk(rows: &mut Vec<Row>, kind: u8, name: &str, block_size: u32, total: u64) {
    let mut h1 = String::from(name);
    if block_size != 512 {
        let _ = write!(h1, "  (512B degil - desteklenmiyor)");
        rows.push(Row { header: true, selectable: false, protected: false, line1: h1,
            line2: String::new(), disk_kind: kind, first_lba: 0, sectors: 0, part_index: 0 });
        return;
    }
    let gb = total * 512 / (1024 * 1024 * 1024);
    let _ = write!(h1, "  ({} GB)", gb);
    rows.push(Row { header: true, selectable: false, protected: false, line1: h1,
        line2: String::new(), disk_kind: kind, first_lba: 0, sectors: 0, part_index: 0 });

    let layout = {
        let mut d = crate::fs::offset::PartitionDevice::new(kind, 0, total);
        gpt::read_disk(&mut d, total)
    };
    if !layout.has_gpt {
        rows.push(Row { header: false, selectable: false, protected: true,
            line1: String::from("  GPT yok (MBR/bos disk - desteklenmiyor)"),
            line2: String::new(), disk_kind: kind, first_lba: 0, sectors: 0, part_index: 0 });
        return;
    }
    for p in &layout.partitions {
        let mut l1 = String::new();
        let _ = write!(l1, "  {}: {}", p.index, p.label);
        let mut l2 = String::new();
        let mb = p.sectors * 512 / (1024 * 1024);
        if mb >= 1024 { let _ = write!(l2, "{}.{} GB", mb / 1024, (mb % 1024) * 10 / 1024); }
        else { let _ = write!(l2, "{} MB", mb); }
        if !p.name.is_empty() { let _ = write!(l2, "   \"{}\"", p.name); }
        rows.push(Row {
            header: false, selectable: !p.protected, protected: p.protected,
            line1: l1, line2: l2,
            disk_kind: kind, first_lba: p.first_lba, sectors: p.sectors, part_index: p.index,
        });
    }
}
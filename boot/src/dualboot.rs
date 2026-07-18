use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use uefi::boot;
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::proto::media::file::{File, FileMode, FileAttribute, Directory, FileInfo};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::{CString16, Handle};
use crate::gui;
use core::time::Duration;

pub enum Target {
    Rusty { handle: Handle, path: &'static str },
    Efi   { handle: Handle, path: &'static str },
}

pub struct BootEntry {
    pub name: String,
    pub target: Target,
}

const TIMEOUT_SECS: i32 = 10;
const LIST_TOP: usize = 6;

// Scanning
pub fn scan() -> Vec<BootEntry> {
    let mut out: Vec<BootEntry> = Vec::new();

    // Our boot image
    let boot_dev: Option<Handle> =
        boot::open_protocol_exclusive::<LoadedImage>(boot::image_handle())
            .ok()
            .and_then(|li| li.device());

    let handles = boot::find_handles::<SimpleFileSystem>().unwrap_or_default();

    let mut rusty_disk = 0usize;
    let mut efi_disk = 0usize;

    for h in handles {
        let Ok(mut fs) = boot::open_protocol_exclusive::<SimpleFileSystem>(h) else { continue };
        let Ok(mut root) = fs.open_volume() else { continue };

        let is_boot = boot_dev == Some(h);
        let mut has_rusty = false;

        // Rusty kernel 
        if exists(&mut root, "kernel.elf") {
            has_rusty = true;
            let e = BootEntry {
                name: String::from("Rusty OS"),
                target: Target::Rusty { handle: h, path: "kernel.elf" },
            };
            if is_boot { out.insert(0, e); } else { out.push(e); }
        }
        // System Rusty kernel
        else if exists(&mut root, "RSYS\\KERNEL.ELF") {
            has_rusty = true;
            rusty_disk += 1;
            out.push(BootEntry {
                name: format!("Rusty OS - Yerel Disk {}", rusty_disk),
                target: Target::Rusty { handle: h, path: "RSYS\\KERNEL.ELF" },
            });
        }

        // Other operating systems
        if is_boot || has_rusty { continue; }
        if exists(&mut root, "EFI\\Microsoft\\Boot\\bootmgfw.efi") {
            out.push(BootEntry {
                name: String::from("Microsoft Windows"),
                target: Target::Efi { handle: h, path: "EFI\\Microsoft\\Boot\\bootmgfw.efi" },
            });
        } else if exists(&mut root, "EFI\\ubuntu\\grubx64.efi") {
            out.push(BootEntry {
                name: String::from("Ubuntu"),
                target: Target::Efi { handle: h, path: "EFI\\ubuntu\\grubx64.efi" },
            });
        } else if exists(&mut root, "EFI\\BOOT\\BOOTX64.EFI") {
            efi_disk += 1;
            out.push(BootEntry {
                name: format!("UEFI Operating System {}", efi_disk),
                target: Target::Efi { handle: h, path: "EFI\\BOOT\\BOOTX64.EFI" },
            });
        }
    }
    out
}

fn exists(root: &mut Directory, path: &str) -> bool {
    let Ok(cs) = CString16::try_from(path) else { return false };
    root.open(&cs, FileMode::Read, FileAttribute::empty()).is_ok()
}

// MENU -> I took care to design it to resemble Windows 7, which I am a fan of.
fn f8_row(n: usize) -> usize { LIST_TOP + n + 2 }
fn cd_row(n: usize) -> usize { f8_row(n) + 1 }
fn tools_lbl_row() -> usize { gui::rows().saturating_sub(8) }
fn tool_row() -> usize { gui::rows().saturating_sub(6) }

fn draw_static(n: usize) {
    gui::clear_all();
    gui::title_bar(1, "Rusty Boot Manager");
    gui::text(2, 3, "Select the operating system to start, or select a tool with TAB:");
    gui::text(2, 4, "(Select using the arrow keys, then press the ENTER key.)");
    gui::text(2, f8_row(n), "Press F8 if you want to specify advanced options for this selection.");
    gui::text(2, tools_lbl_row(), "Tools:");
    gui::bottom_bar(gui::rows() - 2, "ENTER=Select", "TAB=Menu", "ESC=Cancel");
}

fn draw_entries(entries: &[BootEntry], sel: usize, tool_focus: bool) {
    for (i, e) in entries.iter().enumerate() {
        gui::entry(LIST_TOP + i, &e.name, !tool_focus && i == sel);
    }
    gui::entry(tool_row(), "Rusty Memory Diagnostics", tool_focus);
}

fn draw_countdown(n: usize, cd: Option<i32>) {
    let row = cd_row(n);
    gui::clear_row(row);
    if let Some(s) = cd {
        gui::text(2, row,
            &format!("Time remaining until the selected system is automatically restarted:: {}", s));
    }
}

// Returns the index of the selected OS entry
pub fn menu(entries: &[BootEntry]) -> usize {
    let _ = uefi::system::with_stdin(|i| i.reset(false)); // clean old keys

    draw_static(entries.len());
    let mut sel = 0usize;
    let mut tool_focus = false;
    let mut countdown: Option<i32> = Some(TIMEOUT_SECS);
    let mut ticks = 0u32;

    draw_entries(entries, sel, tool_focus);
    draw_countdown(entries.len(), countdown);

    loop {
        let key = uefi::system::with_stdin(|i| i.read_key().ok().flatten());

        if let Some(k) = key {
            // We turn off the countdown with the first key pressed
            if countdown.is_some() {
                countdown = None;
                draw_countdown(entries.len(), None);
            }
            match k {
                Key::Special(ScanCode::UP) => {
                    if tool_focus { tool_focus = false; sel = entries.len() - 1; }
                    else if sel > 0 { sel -= 1; }
                    draw_entries(entries, sel, tool_focus);
                }
                Key::Special(ScanCode::DOWN) => {
                    if !tool_focus {
                        if sel + 1 < entries.len() { sel += 1; }
                        else { tool_focus = true; }
                    }
                    draw_entries(entries, sel, tool_focus);
                }
                Key::Special(ScanCode::ESCAPE) => {
                    if tool_focus { tool_focus = false; draw_entries(entries, sel, tool_focus); }
                }
                Key::Printable(c) => {
                    let code = u16::from(c);
                    if code == 0x0D { // ENTER
                        if tool_focus {
                            memtest();
                            draw_static(entries.len());
                            draw_entries(entries, sel, tool_focus);
                        } else {
                            return sel;
                        }
                    } else if code == 0x09 { // TAB
                        tool_focus = !tool_focus;
                        draw_entries(entries, sel, tool_focus);
                    }
                }
                _ => {}
            }
        } else {
            boot::stall(Duration::from_micros(50_000)); // 50ms
            if let Some(n) = countdown {
                ticks += 1;
                if ticks >= 20 { // 1 second
                    ticks = 0;
                    if n <= 1 { return sel; } // time end -> start default
                    countdown = Some(n - 1);
                    draw_countdown(entries.len(), countdown);
                }
            }
        }
    }
}

// CHAINLOAD = Start another OS .efi
pub fn chainload(handle: Handle, path: &str) {
    let Ok(mut fs) = boot::open_protocol_exclusive::<SimpleFileSystem>(handle) else { return };
    let Ok(mut root) = fs.open_volume() else { return };
    let Ok(cs) = CString16::try_from(path) else { return };
    let Ok(fh) = root.open(&cs, FileMode::Read, FileAttribute::empty()) else { return };
    let Some(mut file) = fh.into_regular_file() else { return };

    let mut info_buf = [0u8; 512];
    let Ok(info) = file.get_info::<FileInfo>(&mut info_buf) else { return };
    let size = info.file_size() as usize;
    if size == 0 { return; }

    let mut data = alloc::vec![0u8; size];
    if file.read(&mut data).is_err() { return; }

    if let Ok(img) = boot::load_image(
        boot::image_handle(),
        boot::LoadImageSource::FromBuffer { buffer: &data, file_path: None },
    ) {
        let _ = boot::start_image(img); // it wont return if is succes
    }
}

pub fn error_screen(msg: &str) {
    gui::clear_all();
    gui::title_bar(1, "Rusty Boot Manager");
    gui::text(2, 4, msg);
    gui::text(2, 6, "Press a key to continue...");
    wait_key();
}

fn wait_key() {
    let _ = uefi::system::with_stdin(|i| i.reset(false));
    loop {
        if uefi::system::with_stdin(|i| i.read_key().ok().flatten()).is_some() { return; }
        boot::stall(Duration::from_micros(30_000));
    }
}

// Tool : Memory Diagnosis
fn memtest() {
    gui::clear_all();
    gui::title_bar(1, "Rusty Memory Diagnostic Tool");
    gui::text(2, 3, "Memory is being tested, please wait...");

    let chunk_pages = 256usize; // 1MB
    let total = 64usize;        // 64MB
    let mut bad = 0u64;

    for i in 0..total {
        if let Ok(p) = boot::allocate_pages(
            boot::AllocateType::AnyPages,
            boot::MemoryType::LOADER_DATA,
            chunk_pages,
        ) {
            let ptr = p.as_ptr();
            let bytes = chunk_pages * 4096;
            unsafe {
                for &pat in &[0xAAu8, 0x55u8] {
                    core::ptr::write_bytes(ptr, pat, bytes);
                    let mut off = 0usize;
                    while off < bytes {
                        if core::ptr::read_volatile(ptr.add(off)) != pat { bad += 1; }
                        off += 4096;
                    }
                }
                let _ = boot::free_pages(p, chunk_pages);
            }
        }
        gui::text(2, 5, &format!("Ilerleme: %{}   ", (i + 1) * 100 / total));
    }

    if bad == 0 {
        gui::text(2, 7, "Result: No memory error found.");
    } else {
        gui::text(2, 7, &format!("Result: {} FAULTY block found!", bad));
    }
    gui::text(2, 9, "Press a key to return to the menu...");
    wait_key();
}
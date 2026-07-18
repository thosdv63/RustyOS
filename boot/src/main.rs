#![no_std]
#![no_main]
extern crate alloc;

mod gui;
mod dualboot;

use uefi::prelude::*;
use uefi::boot;
use uefi::proto::media::file::{File, FileAttribute, FileMode, RegularFile};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;
use core::panic::PanicInfo;
use uefi::mem::memory_map::MemoryMap;
use common::bootinfo::{BootInfo, Framebuffer, MemoryRegion};
use core::time::Duration;

fn print(s: &str) {
    if let Ok(cs) = CString16::try_from(s) {
        system::with_stdout(|out| { let _ = out.output_string(&cs); });
    }
}

fn volume_has_rpe_flag(handle: uefi::Handle) -> bool {
    if let Ok(mut fs) = boot::open_protocol_exclusive::<SimpleFileSystem>(handle) {
        if let Ok(mut root) = fs.open_volume() {
            // Check the different font sizes
            let names = ["RPE.FLAG", "rpe.flag", "RPE.flag"];
            
            for name_str in names.iter() {
                if let Ok(name) = CString16::try_from(*name_str) {
                    if root.open(&name, FileMode::Read, FileAttribute::empty()).is_ok() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// ELF LOADER: Moves PT_LOAD segments into memory, freezes enty addr
fn load_elf(kernel: &mut RegularFile) -> Option<u64> {
    kernel.set_position(0).ok()?;
    let mut header = [0u8; 64];
    kernel.read(&mut header).ok()?;

    let valid = header[0] == 0x7f && header[1] == b'E'
        && header[2] == b'L' && header[3] == b'F'
        && header[4] == 2 && header[18] == 0x3e;
    if !valid { return None; }

    let e_phoff = u64::from_le_bytes(header[32..40].try_into().unwrap());
    let e_phentsize = u16::from_le_bytes(header[54..56].try_into().unwrap()) as usize;
    let e_phnum = u16::from_le_bytes(header[56..58].try_into().unwrap()) as usize;
    let e_entry = u64::from_le_bytes(header[24..32].try_into().unwrap());

    let mut ph_table = alloc::vec![0u8; e_phentsize * e_phnum];
    kernel.set_position(e_phoff).ok()?;
    kernel.read(&mut ph_table).ok()?;

    for i in 0..e_phnum {
        let off = i * e_phentsize;
        let phdr = &ph_table[off..off + e_phentsize];

        let p_type = u32::from_le_bytes(phdr[0..4].try_into().unwrap());
        if p_type != 1 { continue; }

        let p_offset = u64::from_le_bytes(phdr[8..16].try_into().unwrap());
        let p_paddr  = u64::from_le_bytes(phdr[24..32].try_into().unwrap());
        let p_filesz = u64::from_le_bytes(phdr[32..40].try_into().unwrap()) as usize;
        let p_memsz  = u64::from_le_bytes(phdr[40..48].try_into().unwrap()) as usize;

        let page_aligned = p_paddr & !0xfff;
        let page_offset = (p_paddr - page_aligned) as usize;
        let total_pages = (page_offset + p_memsz + 0xfff) / 0x1000;

        let _ = boot::allocate_pages(
            boot::AllocateType::Address(page_aligned),
            boot::MemoryType::LOADER_DATA,
            total_pages,
        );

        let dest = p_paddr as *mut u8;
        kernel.set_position(p_offset).ok()?;

        let mut buf = alloc::vec![0u8; p_filesz];
        kernel.read(&mut buf).ok()?;
        unsafe {
            core::ptr::copy(buf.as_ptr(), dest, p_filesz);
            if p_memsz > p_filesz {
                core::ptr::write_bytes(dest.add(p_filesz), 0, p_memsz - p_filesz);
            }
        }
    }
    Some(e_entry)
}

// Load the kernel from the given path on the given disk (handle)
fn load_kernel_from(handle: uefi::Handle, path: &str) -> Option<u64> {
    let mut fs = boot::open_protocol_exclusive::<SimpleFileSystem>(handle).ok()?;
    let mut root = fs.open_volume().ok()?;
    let name = CString16::try_from(path).ok()?;
    let fh = root.open(&name, FileMode::Read, FileAttribute::empty()).ok()?;
    let mut file = fh.into_regular_file()?;
    load_elf(&mut file)
}

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().unwrap();

    // RUSTY BOOT MANAGER
    gui::init();
    let entries = dualboot::scan();

   if entries.is_empty() {
    gui::title_bar(1, "Rusty Boot Manager");
    gui::text(2, 4, "ERROR: No Rusty kernel was found on any disk!");
    loop { boot::stall(Duration::from_micros(1_000_000)); }
}

    let mut had_fail = false;
    let mut boot_handle: Option<uefi::Handle> = None;
    let e_entry: u64 = loop {
        let idx = if entries.len() == 1 && !had_fail { 0 } else { dualboot::menu(&entries) };

        match &entries[idx].target {
            dualboot::Target::Rusty { handle, path } => {
                match load_kernel_from(*handle, path) {
                    Some(e) => { boot_handle = Some(*handle); break e; }
                    None => {
                        dualboot::error_screen("Failed to load kernel (file could not be read or ELF is incorrect).");
                        had_fail = true;
                    }
                }
            }
            dualboot::Target::Efi { handle, path } => {
                dualboot::chainload(*handle, path);
                dualboot::error_screen("The operating system failed to start.");
                had_fail = true;
            }
        }
    };
    gui::clear_all();

    // === RPE mode detection
    let is_rpe = boot_handle.map(volume_has_rpe_flag).unwrap_or(false);

    print("Rusty bootloader: kernel loaded!\r\n");
    print("PT_LOAD segments loaded!\r\n");

    // Framebuffer from GOP
    use uefi::proto::console::gop::GraphicsOutput;
    use core::fmt::Write;

    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>()
        .expect("GOP handle not found.");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("GOP could not be opened.");

    let mode_info = gop.current_mode_info();
    let (fb_width, fb_height) = mode_info.resolution();
    let fb_stride = mode_info.stride();
    let fb_base = gop.frame_buffer().as_mut_ptr() as u64;

    print("--- Framebuffer Info ---\r\n");
    let mut fbuf = arrayvec::ArrayString::<256>::new();
    let _ = write!(fbuf, "Base: 0x{:x}\r\nWidth: {}\r\nHeight: {}\r\nStride: {}\r\n",
        fb_base, fb_width, fb_height, fb_stride);
    if let Ok(cs) = CString16::try_from(fbuf.as_str()) {
        system::with_stdout(|out| { let _ = out.output_string(&cs); });
    }
    print("GOP was successfully obtained!\r\n");

    // Memory Map preview ===
    let mmap = boot::memory_map(boot::MemoryType::LOADER_DATA)
        .expect("Memory map could not be retrieved.");

    let mut region_count = 0usize;
    let mut total_usable_pages = 0u64;
    for desc in mmap.entries() {
        region_count += 1;
        if desc.ty == boot::MemoryType::CONVENTIONAL {
            total_usable_pages += desc.page_count;
        }
    }
    let total_usable_mb = (total_usable_pages * 4096) / (1024 * 1024);

    print("--- Memory Map ---\r\n");
    let mut mbuf = arrayvec::ArrayString::<256>::new();
    let _ = write!(mbuf, "Number of regions: {}\r\nAvailable RAM: {} MB\r\n",
        region_count, total_usable_mb);
    if let Ok(cs) = CString16::try_from(mbuf.as_str()) {
        system::with_stdout(|out| { let _ = out.output_string(&cs); });
    }
    print("Memory map readed!\r\n");

    // Prepare framebuffer struct
    let framebuffer = Framebuffer {
        base: fb_base as *mut u8,
        width: fb_width as u64,
        height: fb_height as u64,
        stride: fb_stride as u64,
    };

    // Allocate pages for the Memory Region array (a maximum of 256 regions is sufficient)
    const MAX_REGIONS: usize = 256;
    let regions_page = boot::allocate_pages(
        boot::AllocateType::AnyPages,
        boot::MemoryType::LOADER_DATA,
        2,
    ).expect("The region page could not be allocated.");
    let regions_ptr = regions_page.as_ptr() as *mut MemoryRegion;

    let bootinfo_page = boot::allocate_pages(
        boot::AllocateType::AnyPages,
        boot::MemoryType::LOADER_DATA,
        1,
    ).expect("BootInfo page could not be detached.");
    let boot_info_ptr = bootinfo_page.as_ptr() as *mut BootInfo;

    // ACPI 2.0+ Configuration Table GUID
    let acpi_guid = uefi::guid!("8868e871-e4f1-11d3-bc22-0080c73c8881");

    let mut rsdp_addr: u64 = 0;
    if let Some(entry) = system::with_config_table(|table| {
        table.iter().find(|e| e.guid == acpi_guid).map(|e| e.address as u64)
    }) {
        rsdp_addr = entry;
    }

    print("BootInfo prepared, exit_boot_services is being called...\r\n");

    // Exit + get memory map
    let final_mmap = unsafe { boot::exit_boot_services(None) };

    let mut region_count: usize = 0;
    for desc in final_mmap.entries() {
        if region_count >= MAX_REGIONS { break; }
        let usable = matches!(
            desc.ty,
            boot::MemoryType::CONVENTIONAL
                | boot::MemoryType::BOOT_SERVICES_CODE
                | boot::MemoryType::BOOT_SERVICES_DATA
        );
        unsafe {
            core::ptr::write(regions_ptr.add(region_count), MemoryRegion {
                start: desc.phys_start,
                page_count: desc.page_count,
                usable: if usable { 1 } else { 0 },
            });
        }
        region_count += 1;
    }

    unsafe {
        core::ptr::write(boot_info_ptr, BootInfo {
            framebuffer,
            memory_regions: regions_ptr,
            memory_region_count: region_count as u64,
            rsdp_addr,
            rpe_mode: if is_rpe { 1 } else { 0 },
        });
    }

    // Jump to kernel, end of bootloader!
    unsafe {
        core::arch::asm!(
            "mov rdi, {0}",
            "jmp {1}",
            in(reg) boot_info_ptr,
            in(reg) e_entry,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    let mut buf = arrayvec::ArrayString::<512>::new();
    let _ = write!(buf, "PANIC: {}\r\n", info);
    if let Ok(cs) = CString16::try_from(buf.as_str()) {
        system::with_stdout(|out| { let _ = out.output_string(&cs); });
    }
    loop {}
}
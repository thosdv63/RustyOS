#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(static_mut_refs)]
#![feature(stmt_expr_attributes)]
extern crate alloc;

use core::panic::PanicInfo;
use common::bootinfo::BootInfo;
use core::include_bytes;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
pub static EMBEDDED_USERLAND: &[u8] = include_bytes!("../../userland/core.bin");

mod kernRenderer;
mod arch;
mod kernel;
mod mm;
mod drivers;
mod fs;

const BOOT_STACK_SIZE: usize = 128 * 1024;
#[repr(align(16))]
struct BootStack([u8; BOOT_STACK_SIZE]);
static mut BOOT_STACK: BootStack = BootStack([0; BOOT_STACK_SIZE]);

use kernRenderer::Renderer;

static mut RENDERER: Option<Renderer> = None;
static BOOT_ANIM_ACTIVE: AtomicBool = AtomicBool::new(false);
static BOOT_ANIM_TICK: AtomicU64 = AtomicU64::new(0);
const ANIM_TICK_BOL: u64 = 80;
use core::sync::atomic::AtomicU32;
pub static TOTAL_RAM_MB: AtomicU32 = AtomicU32::new(0);
pub static USED_RAM_MB: AtomicU32 = AtomicU32::new(24);
pub static SYS_TICKS: AtomicU64 = AtomicU64::new(0);
pub static POLL_COUNT: AtomicU64 = AtomicU64::new(0);
pub static mut FB_INFO: (u64, u64, u64, u64) = (0, 0, 0, 0);
pub static mut BACK_BUFFER_ADDR: u64 = 0;

static mut LOGO_BUFFER: [u32; 200 * 200] = [0; 40000];
static mut BOOT_FRAME: usize = 0;

static SIN_LUT: [i32; 64] = [
    0, 9, 19, 29, 38, 47, 55, 63, 70, 77, 83, 88, 92, 95, 98, 99,
    100, 99, 98, 95, 92, 88, 83, 77, 70, 63, 55, 47, 38, 29, 19, 9,
    0, -9, -19, -29, -38, -47, -55, -63, -70, -77, -83, -88, -92, -95, -98, -99,
    -100, -99, -98, -95, -92, -88, -83, -77, -70, -63, -55, -47, -38, -29, -19, -9
];

pub fn sysinfo_fill(p: *mut u32) {
    static mut LT: u64 = 0; static mut LP: u64 = 0; static mut MAXR: u64 = 1; static mut CPU: u32 = 0;
    unsafe {
        let t = SYS_TICKS.load(Ordering::Relaxed);
        let po = POLL_COUNT.load(Ordering::Relaxed);
        let dt = t.saturating_sub(LT);
        if dt >= 20 { // ~her guncelleme penceresi
            let dp = po.saturating_sub(LP);
            let rate = dp * 100 / dt;
            if rate > MAXR { MAXR = rate; }
            CPU = (100 - (rate * 100 / MAXR).min(100)) as u32;
            LT = t; LP = po;
        }
        core::ptr::write(p, TOTAL_RAM_MB.load(Ordering::Relaxed));
        core::ptr::write(p.add(1), USED_RAM_MB.load(Ordering::Relaxed));
        core::ptr::write(p.add(2), CPU);
        core::ptr::write(p.add(3), t as u32);
    }
}

pub unsafe fn renderer() -> &'static mut Renderer {
    #[allow(static_mut_refs)]
    RENDERER.as_mut().unwrap()
}

pub fn boot_anim_irq_tick() {
    // ses durdurma irq si
    SYS_TICKS.fetch_add(1, Ordering::Relaxed);
    drivers::audio::tick();

    if !BOOT_ANIM_ACTIVE.load(Ordering::Relaxed) { return; }
    let t = BOOT_ANIM_TICK.fetch_add(1, Ordering::Relaxed);
    if t % ANIM_TICK_BOL == 0 {
        update_boot_anim();
    }
}

fn boot_delay_ms(ms: u64) {
    let mut p = x86_64::instructions::port::Port::<u8>::new(0x80);
    for _ in 0..(ms * 1000) {
        unsafe { p.write(0u8); }
    }
}

fn update_boot_anim() {
    let frame = unsafe { BOOT_FRAME };
    if frame > 100 { return; }

    let renderer = unsafe { renderer() };
    let logo_w = 200;
    let logo_h = 200;

    let start_x = (renderer.width - logo_w) / 2;
    let start_y = (renderer.height - logo_h) / 2;

    unsafe {
        core::slice::from_raw_parts_mut(LOGO_BUFFER.as_mut_ptr(), 40000).fill(0x00000000);
    }

    if frame < 60 {
        let r = 70 - (frame as i32 * 70 / 60);
        let angle_offset = (frame * 2) % 64;

        let orbs = [
            (0, 0x00FF8C00),
            (16, 0x00FFD700),
            (32, 0x008B0000),
            (48, 0x00E0E0E0),
        ];

        for (base_angle, color) in orbs.iter() {
            let idx = (base_angle + angle_offset) % 64;
            let ox = 100 + (r * SIN_LUT[(idx + 16) % 64]) / 100;
            let oy = 100 + (r * SIN_LUT[idx]) / 100;

            let min_y = (oy - 12).max(0) as usize;
            let max_y = (oy + 12).min(199) as usize;
            let min_x = (ox - 12).max(0) as usize;
            let max_x = (ox + 12).min(199) as usize;

            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let dx = px as isize - ox as isize;
                    let dy = py as isize - oy as isize;
                    if dx * dx + dy * dy <= 144 {
                        unsafe { LOGO_BUFFER[py * logo_w + px] = *color; }
                    }
                }
            }
        }
    } else if frame < 65 {
        let radius = 15 + (frame - 60) * 8;
        let cx = 100_isize;
        let cy = 100_isize;

        let min_y = (cy - radius as isize).max(0) as usize;
        let max_y = (cy + radius as isize).min(199) as usize;
        let min_x = (cx - radius as isize).max(0) as usize;
        let max_x = (cx + radius as isize).min(199) as usize;

        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let dx = px as isize - cx;
                let dy = py as isize - cy;
                if dx * dx + dy * dy <= (radius * radius) as isize {
                    unsafe { LOGO_BUFFER[py * logo_w + px] = 0x00FFFFFF; }
                }
            }
        }
    } else {
        let r_mask: [u16; 16] = [
            0b0111111111100000,
            0b0111111111111000,
            0b0111000000011100,
            0b0111000000001110,
            0b0111000000001110,
            0b0111000000011100,
            0b0111111111111000,
            0b0111111111100000,
            0b0111000011100000,
            0b0111000001110000,
            0b0111000000111000,
            0b0111000000011100,
            0b0111000000001110,
            0b0111000000000111,
            0b0000000000000000,
            0b0000000000000000,
        ];

        let scale = 6;
        let r_start_x = 52;
        let r_start_y = 52;

        for row in 0..16 {
            for col in 0..16 {
                if (r_mask[row] >> (15 - col)) & 1 == 1 {
                    let color = if row < 7 {
                        0x00FF_DD_66
                    } else if row == 7 {
                        0x00FF_AA_00
                    } else {
                        let red = 255;
                        let green = 150_u32.saturating_sub((row as u32 - 8) * 15);
                        (red << 16) | (green << 8) | 0x00
                    };

                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = r_start_x + col * scale + dx;
                            let py = r_start_y + row * scale + dy;
                            unsafe { LOGO_BUFFER[py * logo_w + px] = color; }
                        }
                    }
                }
            }
        }
    }

    unsafe {
        renderer.draw_sprite(start_x, start_y, logo_w, logo_h, &LOGO_BUFFER);
        BOOT_FRAME += 1;
    }

    for _ in 0..200_000 {
        unsafe { core::arch::asm!("nop"); }
    }
}

fn play_boot_animation(renderer: &mut Renderer, w: usize, h: usize) {
    for _ in 0..160 {
        update_boot_anim();
        renderer.set_color(0x00FF8020);
        renderer.text_at(w / 2 - 165, h - 40, "Rusty Baslatiliyor...");
        boot_delay_ms(10);
    }
    boot_delay_ms(30);
}

fn mb_of(info: Option<(u32, u64)>) -> u32 {
    match info {
        Some((bs, bc)) => ((bs as u64).saturating_mul(bc) / (1024 * 1024)) as u32,
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn _start(boot_info: *const BootInfo) -> ! {
    let fb = unsafe { &(*boot_info).framebuffer };
    let r = Renderer::new(fb.base, fb.width as usize, fb.height as usize, fb.stride as usize);
    unsafe {
        RENDERER = Some(r);
        FB_INFO = (fb.base as u64, fb.width, fb.height, fb.stride);
    }
    let renderer = unsafe { renderer() };

    renderer.clear(0x00000000);
    renderer.set_color(0x00FFFFFF);

    renderer.set_color(0x00FF8020);
    renderer.text_at(fb.width as usize / 2 - 165, fb.height as usize - 40, "Rusty Baslatiliyor...");

    let rpe_mode = unsafe { (*boot_info).rpe_mode } != 0;
    kernel::rpe::set_mode(rpe_mode);
    kernel::rpe::loading_begin();

    arch::gdt::init();
    arch::idt::init();
    mm::init(boot_info);

    unsafe {
        let regs = (*boot_info).memory_regions;
        let n = (*boot_info).memory_region_count as usize;
        let mut pages = 0u64;
        for i in 0..n {
            let r = &*regs.add(i);
            if r.usable == 1 { pages += r.page_count; }
        }
        TOTAL_RAM_MB.store((pages * 4096 / 1024 / 1024) as u32, Ordering::Relaxed);
    }

    let rsdp = unsafe { (*boot_info).rsdp_addr };
    if let Ok(_) = arch::acpi::init(rsdp) {

        let info = arch::acpi::info().unwrap();
        unsafe { kernel::schd::apic::disable_pic(); }

        let lapic_phys = info.local_apic_addr as u64;
        let mut ptm = crate::mm::vmm::PageTableManager::active();
        if ptm.translate(lapic_phys).is_none() {
            ptm.map(lapic_phys, lapic_phys, 4096, true, false, true, false); 
        }
        let lapic = kernel::schd::lapic::Lapic::new(lapic_phys as usize);
        unsafe {
            lapic.enable();
            lapic.init_timer(10_000_000);
            x86_64::instructions::interrupts::enable();
        }

        let ioapic_phys = info.io_apic_addr as u64;
        if ptm.translate(ioapic_phys).is_none() {
            ptm.map(ioapic_phys, ioapic_phys, 4096, true, false, true, false);
        }
        let ioapic = kernel::schd::apic::IoApic::new(ioapic_phys as usize);
        unsafe {
            ioapic.route(1, 0x21);
            ioapic.route(12, 0x2C);
            drivers::ps2::mouse::init();
            drivers::ps2::keyboard::init();
        }

        let devices = drivers::pci::scan(info.pci_config_addr);

        let mut ptm = crate::mm::vmm::PageTableManager::active();
        for i in 0..256u64 {
            let addr = 0x0100_0000 + i * 0x1000;
            ptm.map(addr, addr, 4096, true, false, false, false);
        }

        drivers::audio::init(&devices);
        kernel::rpe::loading_progress(40); 

        let _ = drivers::storage::nvme::init(&devices);
        let _ = drivers::storage::ahci::init(&devices);
        let _ = drivers::storage::ide::init(&devices);
        let _ = drivers::usb::xhci::init(&devices);
        drivers::usb::storage::init_all();
        kernel::rpe::loading_progress(60); 

        // DRIVERS: NVMe -> C:, AHCI -> D:, USB -> E: F: ...
        {
            use alloc::sync::Arc;
            use spin::Mutex;
            use crate::fs::fat32::{Fat32, Fat32FileSystem};

            let mut letter = b'C';

            if let Some((bs, bc)) = drivers::storage::nvme::info() {
                if let Some(fs) = kernel::rpe::gpt::mount_fat_smart(0, bs, bc) {
                    let mb = (bc * 512 / (1024 * 1024)) as u32;
                    if kernel::rgst::drive_count() == 0 { kernel::rgst::set_fs(fs.clone()); }
                    kernel::rgst::add_drive(letter, "Yerel Disk", 0, mb, fs);
                    letter += 1;
                }
            }

            if let Some((bs, bc)) = drivers::storage::ahci::info() {
                if let Some(fs) = kernel::rpe::gpt::mount_fat_smart(1, bs, bc) {
                    let mb = (bc * 512 / (1024 * 1024)) as u32;
                    if kernel::rgst::drive_count() == 0 { kernel::rgst::set_fs(fs.clone()); }
                    kernel::rgst::add_drive(letter, "Yerel Disk", 0, mb, fs);
                    letter += 1;
                }
            }

            if let Some((bs, bc)) = drivers::storage::ide::info() {
                if letter <= b'Z' {
                    if let Some(fs) = kernel::rpe::gpt::mount_fat_smart(2, bs, bc) {
                        let mb = (bc * (bs as u64) / (1024 * 1024)) as u32;
                        if kernel::rgst::drive_count() == 0 { kernel::rgst::set_fs(fs.clone()); }
                        kernel::rgst::add_drive(letter, "Yerel Disk (IDE)", 0, mb, fs);
                        letter += 1;
                    }
                }
            }

            let usb_n = drivers::usb::storage::count();
            for i in 0..usb_n {
                if letter > b'Z' { break; }
                let arc: Arc<Mutex<dyn crate::fs::BlockDevice>> =
                    Arc::new(Mutex::new(drivers::usb::storage::UsbBlockDevice { idx: i }));
                let fat_res = { let mut d = arc.lock(); Fat32::new(&mut *d) };
                if let Ok(fat) = fat_res {
                    let fs = Fat32FileSystem {
                        fat: Arc::new(Mutex::new(fat)),
                        dev: arc.clone(),
                    };
                    let mb = mb_of(drivers::usb::storage::info(i));
                    if kernel::rgst::drive_count() == 0 { kernel::rgst::set_fs(fs.clone()); }
                    kernel::rgst::add_drive(letter, "Cikarilabilir Disk", 1, mb, fs);
                    letter += 1;
                }
            }

            if !kernel::rpe::is_rpe() {
                kernel::rgst::init_from_disk();
                kernel::rgst::refresh_cache();
                let renk = kernel::rgst::get_u32("Sistem/Masaustu/Renk", 0);
                kernel::rgst::CACHE_DESKTOP.store(renk, core::sync::atomic::Ordering::Relaxed);
            }
        }
        kernel::rpe::loading_progress(80);   // YENI

        if !kernel::rpe::is_rpe() {
            kernel::rgst::recovery::set_embedded_core(EMBEDDED_USERLAND);
            let missing = kernel::rgst::recovery::check();
            if !missing.is_empty() {
                kernel::rgst::recovery::run(&missing); // geri donmez -> reboot
            }
        }
    }

    kernel::schd::scheduler::init();
    kernel::rpe::loading_progress(100);

    for _ in 0..3_000_000 {
        unsafe { core::arch::asm!("nop"); }
    }

    renderer.clear(0x00000000);
    renderer.set_color(0x00FFFFFF);
    play_boot_animation(renderer, fb.width as usize, fb.height as usize);
    if kernel::rpe::is_rpe() {
        kernel::rpe::run();
    }

    let fb_base = fb.base as u64;
    let fb_size = fb.stride * fb.height * 4;
    let fb_pages = (fb_size + 0xFFF) / 0x1000;
    for i in 0..fb_pages {
        let addr = fb_base + i * 0x1000;
    }

    let fb_base = fb.base as u64;
    let fb_bytes = (fb.stride * fb.height * 4) as u64;
    let fb_pages = (fb_bytes + 0xFFF) / 0x1000;
    for i in 0..fb_pages {
        let addr = fb_base + i * 0x1000;
    }
    let mut ptm = crate::mm::vmm::PageTableManager::active();
    for i in 0..fb_pages {
        let addr = fb_base + i * 4096;
        // user = true, disable_cache = true
        ptm.map(addr, addr, 4096, true, true, true, false);
    }

    let back_buffer: u64 = 0x_1000_0000;
    let _ = crate::mm::vmm::map_range_ex(back_buffer, fb_pages, true, true, false);
    unsafe { crate::BACK_BUFFER_ADDR = back_buffer; }
    let bb_mb = ((fb.stride * fb.height * 4) / (1024 * 1024) + 1) as u32;
    USED_RAM_MB.store(20 + bb_mb, Ordering::Relaxed); // kernel+pcm+heap+userheap tahmini + backbuffer
    renderer.clear(0x00000000);

    let user_code: u64 = 0x400000;
    let user_stack: u64 = 0x300000;

    let mut ptm = crate::mm::vmm::PageTableManager::active();
    ptm.unmap(0x200000, 2 * 1024 * 1024);
    ptm.unmap(0x400000, 2 * 1024 * 1024);

    let _ = crate::mm::vmm::map_range_ex(user_stack, 16, true, true, false);
    let user_stack_top = user_stack + 16 * 0x1000;

    let user_heap: u64 = 0x_5000_0000_0000;
    let _ = crate::mm::vmm::map_range_ex(user_heap, 1024, true, true, false);

    let core_bin = match kernel::rgst::fsops::read_system_file("CORE.BIN") {
        Some(d) => d,
        None => {
            renderer.set_color(0x00FF3030);
            renderer.text("\n\n   KRITIK HATA: RSYS/CORE.BIN BULUNAMADI!\n   Sistem dosyalari eksik veya silinmis.\n   Rusty baslatilamiyor.\n");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    };

    let user_pages = ((core_bin.len() as u64) + 0xFFF) / 0x1000;
    let _ = crate::mm::vmm::map_range_ex(user_code, user_pages.max(16), true, true, true);

    unsafe {
        core::ptr::copy_nonoverlapping(
            core_bin.as_ptr(),
            user_code as *mut u8,
            core_bin.len(),
        );
    }

   unsafe {
        kernel::pscy::syscall::init();
        kernel::pscy::usermode::enter_user_mode(user_code, user_stack_top);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

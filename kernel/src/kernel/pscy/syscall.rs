use core::arch::naked_asm;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, Star, KernelGsBase, SFMask};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;

#[repr(align(16))]
struct KStack([u8; 32768]);

static mut KERNEL_STACK_TOP: u64 = 0;
static mut TMP_KERNEL_STACK: KStack = KStack([0; 32768]);

pub fn init() {
    unsafe {
        let sel = crate::arch::gdt::SELECTORS.as_ref().unwrap();

        Efer::update(|flags| flags.insert(EferFlags::SYSTEM_CALL_EXTENSIONS));
        Star::write(sel.user_code, sel.user_data, sel.code, sel.data).unwrap();
        LStar::write(VirtAddr::new(syscall_entry as u64));
        SFMask::write(RFlags::INTERRUPT_FLAG);

        #[allow(static_mut_refs)]
        let top = TMP_KERNEL_STACK.0.as_ptr() as u64 + 32768;
        KERNEL_STACK_TOP = top & !0xF; // 16

        KernelGsBase::write(VirtAddr::new(&KERNEL_STACK_TOP as *const _ as u64));
    }
}

#[unsafe(naked)]
pub unsafe extern "C" fn syscall_entry() {
    naked_asm!(
        "swapgs",
        "mov r10, rsp",
        "mov rsp, gs:[0]",

        "push r10",             
        "push r11",             
        "push rcx",           

        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, rax",

        "sub rsp, 8",
        "call syscall_handler_rust",
        "add rsp, 8",

        "pop rcx",
        "pop r11",
        "pop rsp",
        "swapgs",
        "sysretq"
    );
}

#[no_mangle]
pub extern "C" fn syscall_handler_rust(
    sys_num: u64, arg1: u64, arg2: u64, _arg3: u64, _arg4: u64, _arg5: u64
) -> u64 {
    match sys_num {
        0 => {
            let r = unsafe { crate::renderer() };
            let real_ptr = arg1;
            let s = unsafe { core::slice::from_raw_parts(real_ptr as *const u8, arg2 as usize) };
            if let Ok(str) = core::str::from_utf8(s) {
                r.text(str);
            }
            0
        },
        1 => { // sys_exit
            let r = unsafe { crate::renderer() };
            r.set_color(0x00FF0000);
            r.text("[SCHEDULER] Userland programi kapandi (Exit).\n");
            loop { unsafe { core::arch::asm!("hlt"); } }
        },
        2 => { // sys_get_framebuffer
            let real_ptr = arg1 as *mut u64;
            unsafe {
                let (base, w, h, stride) = crate::FB_INFO;
                core::ptr::write(real_ptr, base);
                core::ptr::write(real_ptr.add(1), w);
                core::ptr::write(real_ptr.add(2), h);
                core::ptr::write(real_ptr.add(3), stride);
                core::ptr::write(real_ptr.add(4), crate::BACK_BUFFER_ADDR);
            }
            0
        },
        3 => { // sys_poll_event
                let real_ptr = arg1 as *mut i32;
                crate::drivers::usb::xhci::poll();
                unsafe { crate::drivers::ps2::mouse::poll(); }
                crate::POLL_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                crate::drivers::audio::tick(); // close stream when sound end
                unsafe {
                    match crate::kernel::pscy::event::pop() {
                        Some(ev) => {
                            core::ptr::write(real_ptr, ev.kind as i32);
                            core::ptr::write(real_ptr.add(1), ev.data1);
                            core::ptr::write(real_ptr.add(2), ev.data2);
                            core::ptr::write(real_ptr.add(3), ev.data3);
                            1
                        }
                    None => 0,
                }
            }
        },
        4 => { // sys_get_time
            let real_ptr = arg1 as *mut i32;
            let (h, m, s, d, mo, y) = crate::drivers::rtc::read();
            unsafe {
                core::ptr::write(real_ptr, h as i32);
                core::ptr::write(real_ptr.add(1), m as i32);
                core::ptr::write(real_ptr.add(2), s as i32);
                core::ptr::write(real_ptr.add(3), d as i32);
                core::ptr::write(real_ptr.add(4), mo as i32);
                core::ptr::write(real_ptr.add(5), y as i32);
            }
            0
        },
        5 => {
            if arg1 == 0 {
                crate::drivers::power::shutdown();
            } else {
                crate::drivers::power::reboot();
            }
        },
        6 => {
            use core::sync::atomic::Ordering;
            match arg1 {
                0 => crate::kernel::rgst::CACHE_DESKTOP.load(Ordering::Relaxed) as u64,
                1 => crate::kernel::rgst::CACHE_TASKBAR.load(Ordering::Relaxed) as u64,
                2 => crate::kernel::rgst::CACHE_YETKI.load(Ordering::Relaxed) as u64,
                _ => 0,
            }
        },
        7 => {
            crate::kernel::rgst::set_u32("Sistem/Masaustu/Renk", arg1 as u32);
            0
        },
        8 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::list_dir_call(buf)
        },
        9 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::create_file_call(buf)
        },
        10 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::delete_file_call(buf)
        },
        11 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::rename_call(buf)
        },
        12 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::create_dir_call(buf)
        },
        13 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::move_call(buf)
        },
        14 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::list_call(buf)
        },
        15 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::set_call(buf)
        },
        16 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::read_file_call(buf)
        },
        17 => {
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::write_file_call(buf)
        },
        18 => { // sys_play_startup: startup sound
            // crate::drivers::audio::play(crate::STARTUP_SOUND);
            0
        },
        19 => { // sys_play_file: buf = [u16 len][path]
            let buf = unsafe { core::slice::from_raw_parts_mut(arg1 as *mut u8, arg2 as usize) };
            crate::kernel::rgst::fsops::play_file_call(buf)
        },
        20 => { // sys_stop_sound
            crate::drivers::audio::stop();
            0
        },
        21 => { crate::sysinfo_fill(arg1 as *mut u32); 0 },
        _ => u64::MAX,
    }
}

use core::arch::asm;
use alloc::vec::Vec;
use alloc::vec;

#[inline(never)]  // inline ETME (her cagri ayri olsun)
pub fn sys_print(text: &str) {
    let ptr = text.as_ptr() as u64;
    let len = text.len() as u64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 0u64,
            in("rdi") ptr,
            in("rsi") len,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),  // syscall'in bozdugu register'lari bildir
        );
    }
}

// Syscall 1: Exit
#[inline(always)]
pub fn sys_exit(code: u64) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") 1,                  
            in("rdi") code,               
            out("rcx") _,
            out("r11") _,
        );
    }
    loop {}
}

// Syscall 2: Get Framebuffer
#[inline(never)]
pub fn sys_get_framebuffer(info_ptr: u64) {
    unsafe {
        asm!(
            "syscall",
            in("rax") 2u64,
            in("rdi") info_ptr,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
            options(nostack),  // ama memory clobber lazim
        );
    }
}

// Syscall 3: Poll Event. event_ptr'ye olay yazilir. Donus: 1=olay var, 0=yok
#[inline(never)]
pub fn sys_poll_event(event_ptr: u64) -> u64 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 3u64,
            in("rdi") event_ptr,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
        );
    }
    ret
}

// Syscall 4: Get Time. ptr'ye [hour,min,sec,day,month,year] yazilir
#[inline(never)]
pub fn sys_get_time(time_ptr: u64) {
    unsafe {
        asm!(
            "syscall",
            in("rax") 4u64,
            in("rdi") time_ptr,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
        );
    }
}

// Syscall 5: Power. 0=shutdown, 1=reboot
#[inline(never)]
pub fn sys_power(action: u64) {
    unsafe {
        asm!(
            "syscall",
            in("rax") 5u64,
            in("rdi") action,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
        );
    }
}

#[inline(never)]
pub fn sys_reg_get_id(id: u64) -> u32 {
    let ret: u64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 6u64,
            in("rdi") id,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
        );
    }
    ret as u32
}

#[inline(never)]
pub fn sys_set_desktop_color(color: u32) {
    unsafe {
        asm!(
            "syscall",
            in("rax") 7u64,
            in("rdi") color as u64,
            out("rcx") _,
            out("r11") _,
            clobber_abi("sysv64"),
        );
    }
}

fn pack_path(buf: &mut [u8], path: &str) {
    let l = path.len().min(buf.len() - 2);
    buf[0..2].copy_from_slice(&(l as u16).to_le_bytes());
    buf[2..2+l].copy_from_slice(&path.as_bytes()[..l]);
}

#[inline(never)]
pub fn sys_list_dir(path: &str, buf: &mut [u8]) -> u64 {
    pack_path(buf, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 8u64, in("rdi") buf.as_mut_ptr() as u64,
        in("rsi") buf.len() as u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}
#[inline(never)]
pub fn sys_create_file(path: &str) -> u64 {
    let mut b = [0u8; 128]; pack_path(&mut b, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 9u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 128u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}
#[inline(never)]
pub fn sys_delete_file(path: &str) -> u64 {
    let mut b = [0u8; 128]; pack_path(&mut b, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 10u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 128u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

#[inline(never)]
pub fn sys_create_dir(path: &str) -> u64 {
    let mut b = [0u8; 128]; pack_path(&mut b, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 12u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 128u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}
#[inline(never)]
pub fn sys_rename(old_full: &str, new_name: &str) -> u64 {
    let mut b = [0u8; 192];
    let l1 = old_full.len().min(120);
    b[0..2].copy_from_slice(&(l1 as u16).to_le_bytes());
    b[2..2+l1].copy_from_slice(&old_full.as_bytes()[..l1]);
    let p = 2 + l1;
    let l2 = new_name.len().min(32);
    b[p..p+2].copy_from_slice(&(l2 as u16).to_le_bytes());
    b[p+2..p+2+l2].copy_from_slice(&new_name.as_bytes()[..l2]);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 11u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 192u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

#[inline(never)]
pub fn sys_move(src_full: &str, dst_dir: &str) -> u64 {
    let mut b = [0u8; 224];
    let l1 = src_full.len().min(120);
    b[0..2].copy_from_slice(&(l1 as u16).to_le_bytes());
    b[2..2+l1].copy_from_slice(&src_full.as_bytes()[..l1]);
    let p = 2 + l1;
    let l2 = dst_dir.len().min(96);
    b[p..p+2].copy_from_slice(&(l2 as u16).to_le_bytes());
    b[p+2..p+2+l2].copy_from_slice(&dst_dir.as_bytes()[..l2]);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 13u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 224u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

#[inline(never)]
pub fn sys_reg_list(buf: &mut [u8]) -> u64 {
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 14u64, in("rdi") buf.as_mut_ptr() as u64,
        in("rsi") buf.len() as u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

// Syscall 16: dosya oku. buf'a icerik yazilir, okunan bayt sayisi doner.
#[inline(never)]
pub fn sys_read_file(path: &str, buf: &mut [u8]) -> u64 {
    pack_path(buf, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 16u64, in("rdi") buf.as_mut_ptr() as u64,
        in("rsi") buf.len() as u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

// Syscall 17: dosya yaz. 0=ok, 1=hata, 2=yetki
#[inline(never)]
pub fn sys_write_file(path: &str, data: &[u8]) -> u64 {
    let plen = path.len().min(200);
    let dlen = data.len();
    let total = 2 + plen + 4 + dlen;
    let mut b: Vec<u8> = vec![0u8; total];
    b[0..2].copy_from_slice(&(plen as u16).to_le_bytes());
    b[2..2+plen].copy_from_slice(&path.as_bytes()[..plen]);
    let p = 2 + plen;
    b[p..p+4].copy_from_slice(&(dlen as u32).to_le_bytes());
    b[p+4..p+4+dlen].copy_from_slice(data);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 17u64, in("rdi") b.as_ptr() as u64,
        in("rsi") total as u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

// Syscall 18: gomulu acilis sesini cal
#[inline(never)]
pub fn sys_play_startup() -> u64 {
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 18u64, lateout("rax") ret,
        out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

// Syscall 19: diskteki ham PCM (.raw) dosyasini cal
#[inline(never)]
pub fn sys_play_file(path: &str) -> u64 {
    let mut b = [0u8; 128]; pack_path(&mut b, path);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 19u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 128u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}

// Syscall 20: sesi durdur
#[inline(never)]
pub fn sys_stop_sound() {
    unsafe { asm!("syscall", in("rax") 20u64,
        out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
}

#[inline(never)]
pub fn sys_sysinfo(out: &mut [u32; 4]) {
    unsafe { asm!("syscall", in("rax") 21u64, in("rdi") out.as_mut_ptr() as u64,
        out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
}

// "Yol=tip:deger" satirini registry'e yaz (syscall 15)
#[inline(never)]
pub fn sys_reg_set_line(line: &str) -> u64 {
    let mut b = [0u8; 200];
    let l = line.len().min(196);
    b[0..2].copy_from_slice(&(l as u16).to_le_bytes());
    b[2..2+l].copy_from_slice(&line.as_bytes()[..l]);
    let ret: u64;
    unsafe { asm!("syscall", in("rax") 15u64, in("rdi") b.as_ptr() as u64,
        in("rsi") 200u64, lateout("rax") ret, out("rcx") _, out("r11") _, clobber_abi("sysv64")); }
    ret
}
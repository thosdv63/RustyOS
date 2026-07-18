use x86_64::instructions::port::Port;

pub fn reboot() -> ! {
    unsafe {
        // Chipset reset 0xCF9 — for modern intel
        let mut cf9 = Port::<u8>::new(0xCF9);
        cf9.write(0x02);
        crate::drivers::usb::udelay(500);
        cf9.write(0x06);                       // hard reset
        crate::drivers::usb::mdelay(20);
        cf9.write(0x0E);                       // power loop reset
        crate::drivers::usb::mdelay(20);
        // 8042 — waiting
        let mut st = Port::<u8>::new(0x64);
        for _ in 0..100_000 { if st.read() & 0x02 == 0 { break; } }
        Port::<u8>::new(0x64).write(0xFE);
        crate::drivers::usb::mdelay(20);
        // Triple fault — empty IDT + int = reset
        let idtr: [u8; 10] = [0; 10];
        core::arch::asm!("lidt [{0}]", "int3", in(reg) &idtr, options(noreturn));
    }
}

// find S5 in DST
fn find_s5(dsdt: &[u8]) -> Option<(u16, u16)> {
    let mut i = 0;
    while i + 8 < dsdt.len() {
        if &dsdt[i..i + 4] == b"_S5_" && dsdt[i + 4] == 0x12 {   // PackageOp
            let mut p = i + 5;
            p += 1 + (dsdt[p] >> 6) as usize;  // PkgLength 1-4 byte
            p += 1;                             // NumElements
            let rd = |q: &mut usize| -> u16 {
                let v = if dsdt[*q] == 0x0A { *q += 1; dsdt[*q] } else { dsdt[*q] };
                *q += 1; v as u16
            };
            let mut q = p;
            return Some((rd(&mut q), rd(&mut q)));
        }
        i += 1;
    }
    None
}

pub fn shutdown() -> ! {
    // Real Hardware: ACPI S5
    if let Some((pm1a, pm1b, dsdt)) = crate::arch::acpi::s5_info() {
        if let Some((ta, tb)) = find_s5(dsdt) {
            unsafe {
                Port::<u16>::new(pm1a).write((ta << 10) | (1 << 13)); // SLP_TYP | SLP_EN
                if pm1b != 0 { Port::<u16>::new(pm1b).write((tb << 10) | (1 << 13)); }
            }
            crate::drivers::usb::mdelay(200);
        }
    }
    // QEMU fallback
    unsafe {
        Port::<u16>::new(0x604).write(0x2000);
        Port::<u16>::new(0xB004).write(0x2000);
        Port::<u16>::new(0x600).write(0x2000);
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
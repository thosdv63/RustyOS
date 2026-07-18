pub struct ExceptionInfo {
    pub vector: u8,        // exception number (0-31)
    pub error_code: u64,   // error code
    pub address: u64,      // error addr (instruction pointer or CR2)
}

// Translate exception number
fn vector_name(vector: u8) -> &'static str {
    match vector {
        0 => "Divide By Zero",
        1 => "Debug",
        2 => "Non-Maskable Interrupt",
        3 => "Breakpoint",
        4 => "Overflow",
        5 => "Bound Range Exceeded",
        6 => "Invalid Opcode",
        7 => "Device Not Available",
        8 => "Double Fault",
        10 => "Invalid TSS",
        11 => "Segment Not Present",
        12 => "Stack Segment Fault",
        13 => "General Protection Fault",
        14 => "Page Fault",
        16 => "x87 Floating Point",
        17 => "Alignment Check",
        18 => "Machine Check",
        19 => "SIMD Floating Point",
        _ => "Unknown Exception",
    }
}

// Panic screen
pub fn handle(info: ExceptionInfo) -> ! {
    let r = unsafe { crate::renderer() };
    
    r.clear(0x00AA0000);
    r.set_color(0x00FFFFFF);
    r.text("\n  *** KERNEL PANIC ***\n\n");

    r.set_color(0x00FFFF00);
    r.text("  Sebep: ");
    r.text(vector_name(info.vector));
    r.text("\n");

    r.set_color(0x00FFFFFF);
    use core::fmt::Write;
    let _ = write!(r, "  Vector: {}\n", info.vector);
    let _ = write!(r, "  Error Code: 0x{:x}\n", info.error_code);
    let _ = write!(r, "  Address: 0x{:x}\n", info.address);
    r.text("\n  System stopped.\n");  

    loop {
        x86_64::instructions::hlt();
    }
}
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::structures::idt::PageFaultErrorCode;
use crate::arch::gdt::DOUBLE_FAULT_IST_INDEX;
use crate::kernel::intr::panic_manager::{self, ExceptionInfo};
use crate::drivers::ps2::{keyboard};

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        IDT.divide_error.set_handler_fn(h_divide);
        IDT.breakpoint.set_handler_fn(h_breakpoint);
        IDT.invalid_opcode.set_handler_fn(h_invalid_op);
        IDT.general_protection_fault.set_handler_fn(h_gpf);
        IDT.page_fault.set_handler_fn(h_page_fault);
        
        // Double Fault uses a secure stack.
        IDT.double_fault
            .set_handler_fn(h_double_fault)
            .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        
        // Hardware interrupts
        IDT[0x20].set_handler_fn(h_timer);
        
        // Connecting keyboard handler to IDT
        IDT[0x21].set_handler_fn(h_keyboard); 
        IDT[0x2C].set_handler_fn(h_mouse);
        
        IDT[0xFF].set_handler_fn(h_spurious);

        IDT.load();
    }
}

// Other panic handlers
extern "x86-interrupt" fn h_divide(f: InterruptStackFrame) {
    panic_manager::handle(ExceptionInfo { vector: 0, error_code: 0, address: f.instruction_pointer.as_u64() });
}
extern "x86-interrupt" fn h_breakpoint(f: InterruptStackFrame) {
    panic_manager::handle(ExceptionInfo { vector: 3, error_code: 0, address: f.instruction_pointer.as_u64() });
}
extern "x86-interrupt" fn h_invalid_op(f: InterruptStackFrame) {
    panic_manager::handle(ExceptionInfo { vector: 6, error_code: 0, address: f.instruction_pointer.as_u64() });
}
extern "x86-interrupt" fn h_gpf(f: InterruptStackFrame, ec: u64) {
    panic_manager::handle(ExceptionInfo { vector: 13, error_code: ec, address: f.instruction_pointer.as_u64() });
}
extern "x86-interrupt" fn h_page_fault(_f: InterruptStackFrame, ec: PageFaultErrorCode) {
    let addr = x86_64::registers::control::Cr2::read_raw();
    panic_manager::handle(ExceptionInfo { vector: 14, error_code: ec.bits(), address: addr });
}
extern "x86-interrupt" fn h_double_fault(f: InterruptStackFrame, ec: u64) -> ! {
    panic_manager::handle(ExceptionInfo { vector: 8, error_code: ec, address: f.instruction_pointer.as_u64() });
}

// Timer handler
extern "x86-interrupt" fn h_timer(_f: InterruptStackFrame) {
    unsafe {
        crate::boot_anim_irq_tick();
        crate::drivers::usb::xhci::poll();
        core::ptr::write_volatile((0xFEE00000_usize + 0x00B0) as *mut u32, 0);
        crate::kernel::schd::scheduler::schedule();
    }
}

// Keyboard Handler
extern "x86-interrupt" fn h_keyboard(_f: InterruptStackFrame) {
    unsafe {
        let mut port = x86_64::instructions::port::Port::new(0x60);
        let scancode: u8 = port.read();
        
        // We are redirecting to the main keyboard module
        keyboard::handle_scancode(scancode);

        core::ptr::write_volatile((0xFEE00000_usize + 0x00B0) as *mut u32, 0);
    }
}

extern "x86-interrupt" fn h_mouse(_f: InterruptStackFrame) {
    unsafe {
        // We ensure that we are calling the module correctly
        crate::drivers::ps2::mouse::handle_interrupt();

        // We are notifying LAPIC that we have received the interrupt (EOI)
        core::ptr::write_volatile((0xFEE00000_usize + 0x00B0) as *mut u32, 0);
    }
}

extern "x86-interrupt" fn h_spurious(_f: InterruptStackFrame) {}
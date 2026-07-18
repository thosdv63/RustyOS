pub mod gdt;
pub mod idt;
pub mod acpi;

pub fn init() {
    gdt::init();
    idt::init();
}
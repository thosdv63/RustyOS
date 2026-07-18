use core::arch::asm;

pub unsafe fn enter_user_mode(entry: u64, user_stack_top: u64) -> ! {
    let sel = crate::arch::gdt::SELECTORS.as_ref().unwrap();
    let user_cs = (sel.user_code.0 | 3) as u64;
    let user_ds = (sel.user_data.0 | 3) as u64;

    asm!(
        // first set data segments
        "mov ds, {ds:x}",
        "mov es, {ds:x}",
        "mov fs, {ds:x}",
        "mov gs, {ds:x}",
        // iretq frame: SS, RSP, RFLAGS, CS, RIP (inverse push)
        "push {ss}",
        "push {rsp}",
        "push 0x202",
        "push {cs}",
        "push {rip}",
        "iretq",
        ds = in(reg) user_ds,
        ss = in(reg) user_ds,
        rsp = in(reg) user_stack_top,
        cs = in(reg) user_cs,
        rip = in(reg) entry,
        options(noreturn)
    );
}
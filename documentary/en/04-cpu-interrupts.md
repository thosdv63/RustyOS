# 04 — CPU and Interrupts

With memory working, the kernel turns to the processor itself. A modern x86_64 CPU
needs several tables filled in before it will run privileged and unprivileged code
side by side, deliver interrupts, and accept system calls. Rusty OS sets all of
this up under `arch`, and layers the scheduler and syscall machinery on top.

## Descriptor tables

The Global Descriptor Table, in `arch/gdt`, defines the segments the processor
uses: kernel code and data, user code and data, and a Task State Segment. The TSS
carries two things that matter later. The first is an interrupt-stack-table entry
for the double-fault handler, so that even a catastrophic fault has a known-good
stack to run on. The second is the privilege stack (RSP0), the kernel stack the
processor switches to when a ring-3 program traps into ring 0 — the scheduler
updates this every time it hands the CPU to a different process, so each one
returns to the kernel on its own stack.

The Interrupt Descriptor Table, in `arch/idt`, wires up exception and interrupt
handlers. CPU exceptions — divide-by-zero, invalid opcode, general protection
faults, page faults, double faults — route into a panic manager that draws a
diagnostic screen. Hardware interrupts are wired too: the timer, the keyboard, and
the mouse each get a handler, and each ends by signaling end-of-interrupt to the
local APIC. The timer handler is where the system's periodic work happens: it
advances a boot animation, polls the USB controller, and calls the scheduler.

## Interrupt controllers and ACPI

Rusty OS uses the APIC rather than the legacy PIC. In `arch/apic`, the old
programmable interrupt controllers are remapped and then fully masked, taking them
out of the picture, and the I/O APIC is programmed to route hardware IRQ lines to
the right IDT vectors. The local APIC, in `arch/lapic`, is enabled and its timer
is configured in periodic mode to fire the scheduler tick.

To find all of this hardware, the kernel parses ACPI tables in `arch/acpi`,
starting from the RSDP pointer the bootloader captured. It reads the interrupt
model to locate the local and I/O APIC addresses, counts the processors, and finds
the PCI configuration base. It also digs out the power-management registers and the
DSDT, which the shutdown path needs to find the ACPI S5 (power-off) values. The AML
interpreter that a full ACPI implementation would provide is not present — its
methods are left unimplemented — so anything that would normally go through AML,
like reading the clock or powering off, is done through direct port access instead.

## The scheduler

The scheduler, in `kernel/schd`, is cooperative and deliberately simple. Each task
owns a 64 KB stack, prepared so that the very first context switch into it looks
just like a return from a previous switch. Switching is a short piece of assembly:
it pushes the callee-saved registers onto the current task's stack, saves the stack
pointer, loads the next task's stack pointer, pops its registers back, and returns —
landing in the middle of wherever that task last left off.

## System calls and ring 3

The boundary between the kernel and the userland is the system call, set up in
`kernel/pscy/syscall`. Using the processor's fast SYSCALL/SYSRET instructions, the
kernel registers an entry point and a dedicated kernel stack. When the userland
executes `syscall`, the processor jumps into the kernel with the call number in one
register and arguments in others; a small assembly stub rearranges them into the
C calling convention and dispatches to a Rust handler that implements every call —
printing text, polling events, reading the clock, filesystem operations, registry
access, playing sound, and more.

Entering ring 3 in the first place is handled in `kernel/pscy/usermode`: the kernel
sets the user data segments, builds an interrupt-return frame on the stack with the
user code segment, stack pointer, and entry address, and executes `iretq`. The
processor drops to ring 3 and begins running the userland — which, from that point
on, can only reach back across the boundary through the syscalls just described.
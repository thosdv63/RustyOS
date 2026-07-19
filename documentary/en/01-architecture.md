# 01 — Architecture

Rusty OS is organized as a Cargo workspace with three members — `kernel`, `boot`,
and `common` — plus a separate `userland` package that is compiled on its own and
embedded into the system as a flat binary. Keeping these as distinct compilation
units matters, because they run in very different worlds: the bootloader runs
under UEFI firmware services, the kernel runs bare-metal in ring 0, and the
userland runs as an unprivileged ring-3 process. They cannot share a runtime, so
the code is split along those boundaries.

The one thing they genuinely need to share is the shape of the data the bootloader
hands to the kernel. That lives in the `common` crate, which is deliberately tiny:
it defines the `BootInfo` structure — the framebuffer description, the memory map,
the ACPI pointer, and a flag indicating whether the system is running as an
installer — and little else. Both `boot` and `kernel` depend on `common`, so the
bootloader fills in a `BootInfo` and the kernel reads the exact same layout back
out. Nothing else crosses that line.

## The execution flow

From power-on to a running desktop, control passes through four distinct stages:

```
  UEFI Firmware
       │  loads and starts
       ▼
  Bootloader (boot crate, ring 0, UEFI services)
       │  loads kernel ELF, fills BootInfo, exits boot services
       ▼
  Kernel (kernel crate, ring 0, bare metal)
       │  brings up memory, drivers, filesystem; loads core.bin
       ▼
  Userland (userland package, ring 3)
          the desktop, windows, and applications
```

Each arrow is a genuine handoff. The bootloader jumps to the kernel's entry point
with a pointer to `BootInfo` in a register. The kernel, once it has everything
ready, drops the processor into ring 3 and begins executing the userland binary.
From that point on, the userland can only reach back into the kernel through
system calls.

## How the source is laid out

The kernel crate is where most of the code lives, and it is divided by
responsibility. Architecture-specific setup — the descriptor tables, the interrupt
table, ACPI — sits under `arch`. Hardware drivers are grouped under `drivers`,
with storage, USB, audio, and PS/2 each in their own subtree. The filesystem, with
its FAT32 implementation and VFS layer, is under `fs`. The higher-level kernel
services — the scheduler, the process and syscall machinery, the event queue, the
registry, and the installer — live under `kernel`. Memory management has its own
home in `mm`, and the framebuffer renderer used during early boot and for kernel
panics sits in its own rendering module.

The userland package mirrors some of this structure but is far simpler. It has its
own heap allocator, its own renderer, a thin syscall wrapper library, an
application framework, and the UI layer that implements the desktop. Because it is
compiled to a flat binary and loaded at a fixed address, it also carries a small
amount of low-level startup code to prepare itself before `main` runs.

## A note on addresses

Because the kernel identity-maps physical memory and hands out fixed virtual
regions for specific purposes, a handful of addresses recur throughout the
codebase. They are worth having in mind while reading later chapters:

| Region | Address | Purpose |
|--------|---------|---------|
| Userland / core.bin | `0x000000`–`0x500000` | reserved; DMA never allocates here |
| Back buffer | `0x10000000` | where the userland draws before presenting |
| Framebuffer | `0x80000000` | the physical screen, copied from the back buffer |
| Kernel heap | `0x44440000000` | linked-list allocator for the kernel |
| Userland heap | `0x50000000000` | linked-list allocator for the userland |

These are not arbitrary trivia — they are load-bearing. The reserved low region,
for instance, exists precisely so that a DMA transfer from a disk controller can
never scribble over the running userland, a class of bug that is nearly impossible
to debug after the fact.
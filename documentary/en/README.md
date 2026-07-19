# Rusty OS — Documentation (English)

A complete, plain-language walkthrough of Rusty OS, from the first instruction the
CPU executes to the applications running on the desktop. Read it top to bottom for
the full picture, or jump to the subsystem you care about.

*Türkçe için: [`../tr/`](../tr/)*

## Contents

1. [Overview](00-overview.md) — what Rusty OS is and why it exists
2. [Architecture](01-architecture.md) — the workspace and how the pieces fit together
3. [Boot Process](02-boot-process.md) — from firmware to kernel entry
4. [Memory](03-memory.md) — the frame allocator, paging, and the heap
5. [CPU and Interrupts](04-cpu-and-interrupts.md) — descriptor tables, APIC, ACPI, the scheduler, syscalls
6. [Drivers](05-drivers.md) — PCI, storage, USB, input, audio
7. [Filesystem](06-filesystem.md) — FAT32, the VFS, GPT, partition safety
8. [Registry](07-registry.md) — settings, persistence, recovery
9. [Graphics](08-graphics.md) — the framebuffer, the renderer, the cursor
10. [Userland and Syscalls](09-userland-and-syscalls.md) — core.bin, the syscall ABI, the event loop
11. [Desktop Environment](10-desktop-environment.md) — windows, taskbar, start menu, OOBE, login
12. [Applications](11-applications.md) — the built-in programs
13. [RPE Installer](12-rpe-installer.md) — installing onto real hardware
14. [Building and Running](13-building-and-running.md) — toolchain, QEMU, make targets
15. [Known Issues](14-known-issues.md) — the rough edges and the roadmap
# 00 — Overview

Rusty OS is a desktop operating system for 64-bit Intel and AMD machines,
written entirely in Rust with no operating system underneath it. There is no
Linux kernel doing the hard parts, no C runtime, no `std` library — the moment
the firmware hands over control, everything that happens is code from this
repository. It boots, manages memory, talks to disks and USB devices, draws a
windowed desktop, and runs real applications, and it does all of this on bare
metal.

The project is inspired by Windows 7. That inspiration is deliberate and shows up
everywhere: a glossy taskbar with a round start button, draggable windows with
minimize/maximize/close controls, a two-panel start menu, an out-of-box setup
wizard, and a login screen. The palette is not Windows blue, though — it's a warm
orange and amber, the "Rusty" identity the name plays on. The goal was never to
clone Windows pixel for pixel, but to answer a simpler question: what would it
feel like to build that entire experience yourself, from the first instruction
the CPU executes?

## Why it exists

This operating system is a personal project with a long history behind it. It began with the first machine that sparked interest in how computers actually worked beneath the surface. There were earlier attempts before Rusty OS; among them was a version written in C that progressed to a working AHCI disk drive. RustyOS is the rewrite that finally reached the finish line: a system complete enough to be installed and used on a real computer.

The philosophy throughout is to understand every layer rather than to import it.
Where most projects would pull in a crate to parse a filesystem or drive a piece
of hardware, Rusty OS implements it directly, because the point of the exercise is
the implementation. The result is a codebase where you can trace a single
keypress from the moment the keyboard controller raises an interrupt, through the
kernel's handler, into an event queue, across the syscall boundary, and finally
into a text box being redrawn on screen.

## What it can do

By version 1, Rusty OS boots through its own UEFI bootloader, initializes the
processor's descriptor tables and interrupt controllers, sets up paging and a
heap, and enumerates the PCI bus to find its hardware. It drives NVMe and AHCI
storage, USB devices through an xHCI controller, PS/2 keyboards and mice, and
Intel HDA or AC'97 audio. It reads and writes a FAT32 filesystem through a small
virtual filesystem layer, keeps its settings in a plain-text registry, and runs a
userland in ring 3 that communicates with the kernel through a compact system-call
interface.

On top of that kernel sits a full desktop environment — a window manager, a
taskbar, a start menu, desktop icons you can drag and rename — and a set of
built-in applications: a file explorer, a text editor, a paint program, a
calculator, a registry editor, a task manager, a settings panel, a command prompt,
and an image viewer.

Finally, Rusty OS ships with its own installer, the Rusty Preinstallation
Environment, which can write the system onto a real GPT disk alongside an existing
Windows or Linux installation without damaging them.

## How to read this documentation

The chapters that follow move from the bottom of the stack to the top. The next
chapter lays out the overall architecture and how the codebase is organized; after
that, each subsystem gets its own chapter, roughly in the order the machine itself
brings them to life — boot, memory, the processor, drivers, the filesystem, the
registry, graphics, the userland, the desktop, the applications, and the
installer. The final chapters cover how to build and run everything, and give an
honest account of the parts that are still rough.
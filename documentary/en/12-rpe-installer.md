# 12 — RPE Installer

Building an operating system that runs in an emulator is one thing; getting it onto
a real computer, next to Windows and Linux, without destroying anything, is another.
That is the job of the Rusty Preinstallation Environment — RPE — a self-contained
installer that turns a USB stick into something you can boot and use to deploy Rusty
OS onto a real disk. It lives under `kernel/rpe`.

## Why it needs nothing else

The clever part of the RPE's design is that it carries everything it installs inside
itself. The payload — the bootloader, the kernel, and the userland's `CORE.BIN` — is
embedded directly into the RPE kernel image at build time using `include_bytes!`.
This matters enormously on real hardware: it means the installer does not depend on
being able to read from the USB stick as a storage device. Even if USB mass storage
enumeration fails on a particular machine, the installation still works, because the
files are already in memory. The installer only needs to be able to write to the
target disk, not read from where it came.

## The two-pass build

Embedding the payload creates a chicken-and-egg problem: the kernel needs to contain
a copy of itself. The Makefile solves this with a two-pass build. On the first pass
it compiles with a tiny stub payload, producing the normal kernel that will actually
be installed. That kernel is then stripped of its debug symbols — shrinking it
dramatically — and copied into the payload directory. On the second pass the kernel
is compiled again, this time embedding that real, stripped kernel as its payload,
producing the RPE kernel that goes onto the USB stick.

## Writing to the disk safely

The installer, in `kernel/rpe/install`, formats the chosen partition as FAT32 —
computing the geometry, laying down the boot sector and file allocation tables,
creating the directory structure, and writing the payload files into place, all
without using the heap, so it works in the constrained installer environment. It
writes the bootloader, two copies of the kernel, `CORE.BIN`, and a fresh registry.

Crucially, every write goes through the same `PartitionDevice` from the filesystem
layer that enforces partition boundaries. The installer physically cannot write
outside the partition the user selected, because the bounds check lives in the
device itself. This is the guarantee that makes it safe to run on a disk that also
holds Windows and Linux.

Getting a bootloader onto the disk requires touching the EFI system partition, and
that is handled with extreme care in `kernel/rpe/esp`. It does not format the ESP —
it reads the existing FAT, finds free clusters, and adds only Rusty's bootloader,
writing `\EFI\Rusty\BOOTX64.EFI` always, and the fallback `\EFI\BOOT\BOOTX64.EFI`
only if that slot is empty, so it never overwrites the Windows Boot Manager. The
existing files — Windows, GRUB, anything already there — are left untouched.

## The installer experience

The front end, in `kernel/rpe/ui`, is a Windows 7-style installer with a blue
gradient, driven entirely by the PS/2 keyboard since interrupts are disabled during
installation. It reads every disk's GPT and presents the partitions, clearly marking
which are protected — the EFI, Windows, and Linux partitions the user cannot select —
and which are selectable empty FAT32 partitions. It walks through a welcome screen,
partition selection, a final confirmation with a deliberate one-second delay before
it accepts a keypress, and an installation progress checklist, ending in a countdown
reboot.

This is the piece that turned Rusty OS from a project that runs in QEMU into one
that has been installed on a real laptop's FAT32 partition and booted through F12 —
leaving the existing Windows, Linux, and EFI setup completely intact.
# 13 — Building and Running

Rusty OS is built with a Makefile that orchestrates several separate compilations
and assembles them into bootable disk images. This chapter covers what you need,
what the build actually does, and how to run the system — whether in an emulator or
on real hardware.

## Prerequisites

You'll need a Linux host with the Rust nightly toolchain, since the kernel relies on
unstable features and compiles for bare-metal targets. Two targets are used:
`x86_64-unknown-none` for the kernel and userland, and `x86_64-unknown-uefi` for the
bootloader. Beyond Rust, the build needs QEMU (version 10.1 or newer), the OVMF UEFI
firmware images, and `mtools` for populating FAT filesystems. For the GPT and
installer targets, `sgdisk` is also required.

## What the build does

The build has a specific order because of how the pieces depend on each other. The
userland is compiled first and converted to a flat binary, `core.bin`. Then the
kernel is compiled — and because the kernel embeds `core.bin`, its build cache is
cleared first so it always picks up a fresh copy. The bootloader is compiled
separately as a UEFI application. Finally, the images are assembled: a boot image
carrying the bootloader, kernel, and userland, and separate virtual disks — an NVMe
disk and a SATA disk, each formatted FAT32 and seeded with a system directory and a
default registry, plus a USB disk for testing.

## Running in QEMU

The main target builds everything and launches QEMU with a full complement of
virtual hardware:

```bash
make run
```

The QEMU invocation is deliberately rich, because it exercises every driver: it
attaches an NVMe controller, an AHCI SATA controller, an xHCI USB controller with a
keyboard, mouse, and storage device, and both Intel HDA and AC'97 audio. This is
what lets the same build be tested against all of its drivers at once. Two details
are worth knowing. KVM acceleration is essential — the pure software emulator is far
too slow to be usable — and an IOMMU device must not be added without an IOMMU
driver present, because it breaks NVMe DMA.

## The installer and installed-system targets

Three more targets cover the installer workflow. `make run-rpe` builds the RPE
installer USB image (through the two-pass build) and a virtual GPT disk laid out like
a real machine — an EFI partition, a Microsoft reserved partition, a Windows-like
NTFS partition, a Linux partition, and an empty target partition — then boots the USB
first so you can install onto that disk. `make run-installed` boots the resulting
disk directly, as though the machine were starting a system that has already been
installed. And `make gpt-disk` prepares just the partitioned test disk.

## A note on the build profile

One non-obvious build setting is worth calling out, because it was learned the hard
way. The development profile disables overflow checks and debug assertions. This is
not about performance — it's a correctness requirement. Rust nightly's pointer-read
precondition checks emit trap instructions that a bare-metal kernel has no handler
for, and leaving them enabled causes the kernel to crash on operations that are
actually fine. Turning them off in the profile is what lets the kernel run at all.

## Running on real hardware

The path to real hardware runs through the RPE. You write the RPE USB image to an
actual USB stick, boot the target machine from it via the firmware's boot menu
(typically F12), and use the installer to write Rusty OS onto an empty FAT32
partition. Because the installer only touches the partition you select and adds its
bootloader to the ESP without disturbing what's there, this can be done on a machine
that already runs Windows or Linux — which is exactly how Rusty OS came to run on a
real laptop.
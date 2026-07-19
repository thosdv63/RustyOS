# 06 — Filesystem

A disk driver moves raw blocks of bytes; a filesystem turns those blocks into
files and directories with names. Rusty OS uses FAT32 for this, chosen because it
is well understood, universally supported, and — importantly — the same format
used by the EFI system partition, which lets the installer write bootloaders onto
disks that a real firmware will recognize. Everything here lives under `fs`.

## The block-device abstraction

At the base is the `BlockDevice` trait, defined in `fs`: read a block, write a
block, report the block size. Every storage driver in the system — NVMe, AHCI, and
USB mass storage — implements it, which means the filesystem code is written
against this one interface and never needs to know what kind of hardware is
underneath. This is the seam that makes the same FAT32 implementation work
identically on an internal SSD, a SATA drive, or a USB stick.

## FAT32

The FAT32 implementation, in `fs/fat32`, reads the boot sector to learn the disk's
geometry — bytes per sector, sectors per cluster, where the file allocation table
begins, where the data region starts — and then everything else follows from
walking cluster chains. Directories are read by following their chains and
decoding directory entries, including long filename entries so that names longer
than the old 8.3 limit are handled. Writing a file allocates clusters, links them
together in the FAT, and writes a directory entry pointing at the first cluster.

A detail worth calling out is that writing a file needs to know which directory it
belongs to — its parent cluster — rather than assuming it always lives in the root.
This is what lets the filesystem support nested folders correctly, and it's the
kind of thing that only becomes obvious once you try to save a file two levels
deep and watch it land in the wrong place.

## The virtual filesystem layer

Above FAT32 sits a small VFS, in `fs/vfs`, which defines what any filesystem must
provide: an `INode` trait for reading, writing, and describing a file or directory,
and a `FileSystem` trait for finding the root. On top of that, `fs/file` wraps an
inode with an offset and provides the familiar read, write, and seek operations
that track a current position through a file. This layer is thin, but it's the
abstraction the rest of the kernel and the userland's file operations are built
against.

## Partitions and GPT

The most safety-critical piece is how Rusty OS handles partitioned disks. A real
computer's disk isn't a single FAT32 volume — it's a GPT-partitioned disk with
Windows, Linux, and EFI partitions that must never be touched. Two modules make
this safe.

`fs/offset` defines a `PartitionDevice`, which presents a single GPT partition as
though it were a standalone disk. It adds the partition's start offset to every
block access, and — this is the important part — refuses any access beyond the
partition's boundary. Because the bounds check lives in the device itself, it is
mathematically impossible for a write aimed at one partition to land in another.

`fs/gpt` reads the GPT: it parses the header and partition entries, decodes each
partition's type from its GUID, and classifies it — an EFI system partition, a
Microsoft reserved partition, a Windows NTFS volume, a Linux filesystem, and so on.
This classification drives the installer's safety logic, marking protected
partitions the user cannot select. It also provides the smart mount that detects
GPT before attempting to treat a whole disk as FAT — which prevents a protective
MBR from being mistaken for a filesystem — and locates the EFI system partition
when the installer needs to add a bootloader.

Across the running system, mounted disks are assigned drive letters in order: the
NVMe disk becomes `C:`, an AHCI disk `D:`, and USB drives take letters from `E:`
onward.
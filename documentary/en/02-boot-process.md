# 02 — Boot Process

When a UEFI machine powers on, the firmware looks for a bootable executable and
starts it with a rich set of services still available: it can allocate memory,
read files, query the graphics hardware, and enumerate devices. Rusty OS's
bootloader lives in the `boot` crate and takes full advantage of this window. Its
job is to find and load the kernel, describe the machine to it, and then get out
of the way.

## The boot manager

Before anything is loaded, the bootloader presents a boot manager — a text-mode
menu styled deliberately after the Windows 7 boot screen, down to the highlighted
selection bar and the countdown timer. This is more than decoration. The
bootloader scans every filesystem the firmware exposes, looking for bootable
targets: a Rusty kernel (either as `kernel.elf` in the root of the boot volume or
as `RSYS\KERNEL.ELF` on an installed disk), and other operating systems it can
recognize — Windows via its boot manager, Ubuntu via GRUB, or any generic UEFI
loader at the standard fallback path.

Each of those becomes an entry in the menu. If only one Rusty kernel is present
and nothing has failed, the bootloader can start it immediately; otherwise it
shows the list, complete with a ten-second countdown that any keypress cancels.
The menu also carries a small tool — a memory diagnostic that allocates and
pattern-tests RAM in one-megabyte chunks, reporting any faulty blocks. Selecting
another operating system instead of Rusty triggers a chainload: the bootloader
reads that OS's `.efi` file into memory and starts it, handing the machine over
entirely.

## Loading the kernel

Once a Rusty kernel is chosen, the bootloader parses it as an ELF file. It
validates the header, walks the program headers, and for each loadable segment it
allocates pages at the segment's physical address and copies the contents in,
zeroing any trailing space the segment requires beyond what's stored in the file.
The entry point address is remembered for the final jump.

With the kernel in memory, the bootloader assembles the `BootInfo` structure. It
queries the Graphics Output Protocol for the framebuffer's base address, width,
height, and stride. It reads the memory map and records which regions are usable
RAM. It looks up the ACPI 2.0 configuration table to find the RSDP pointer, which
the kernel will later need to discover the interrupt controllers and power
management hardware. And it checks whether the boot volume carries an `RPE.FLAG`
file — the marker that tells the kernel it should run as the installer rather than
booting the installed system.

## The point of no return

The final steps are the most delicate. The bootloader allocates pages for the
memory-region array and the `BootInfo` itself, then calls `exit_boot_services`.
This is a one-way door: after it, the firmware's services vanish, and the memory
map is frozen. The bootloader takes the final map the firmware returns, writes out
the usable regions, fills in the completed `BootInfo`, and then performs the
handoff — a short piece of assembly that places the `BootInfo` pointer in a
register and jumps to the kernel's entry point. The bootloader never returns; from
here, the kernel is in control.

```
  scan disks ─► show boot manager ─► load kernel ELF
                                          │
                                          ▼
              query GOP + memory map + ACPI + RPE flag
                                          │
                                          ▼
              exit_boot_services ─► jump to kernel entry
```

The care taken here — validating the ELF, reserving the right pages, capturing the
ACPI pointer before the firmware is dismissed — is what lets the kernel start from
a known, well-defined state on the other side.
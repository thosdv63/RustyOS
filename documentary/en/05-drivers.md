# 05 — Drivers

Drivers are where an operating system meets the physical world, and Rusty OS talks
to real hardware directly — no abstraction library sitting in between. Everything
in this chapter lives under `drivers`, and it all begins with finding the devices
in the first place.

## PCI and low-level I/O

The kernel discovers hardware by scanning the PCI bus in `drivers/pci`. Using the
memory-mapped configuration space (ECAM) whose base address ACPI provided, it
walks every bus, device, and function, reading each one's vendor and device ID,
its class and subclass, and its programming interface. From this scan it learns
what storage controllers, audio devices, and USB host controllers are present.
Two small helpers round out the module: reading a device's base address registers
(its BARs, where its registers or memory live) and enabling bus mastering, which a
device needs before it can perform DMA.

Underneath sits `drivers/io`, a thin wrapper over the two ways hardware is
accessed: port I/O for older devices, and memory-mapped I/O for modern ones. Every
MMIO access goes through a volatile read or write so the compiler never reorders or
elides it, and a memory fence is issued before ringing a device's doorbell — a
discipline that matters enormously when the hardware is reading the same memory the
CPU just wrote.

## Storage

Rusty OS drives three kinds of disk. The NVMe driver, in `drivers/storage/nvme`,
resets the controller, sets up admin and I/O submission and completion queues in
DMA memory, and issues commands by writing entries and ringing doorbells; it
identifies the namespace to learn the block size and count, then reads and writes
blocks by polling the completion queue. The AHCI driver, in `drivers/storage/ahci`,
handles SATA disks through command lists, FIS structures, and physical-region
descriptor tables, finding the first port with an attached drive and issuing READ
and WRITE DMA commands.
The IDE ATA drive in `drivers/storage/ide` executes READ and WRITE commands via PIO,
with Master and Slave support depending on whether it is Native PCI or
Legacy/Compatible.

All three present same small interface — a `BlockDevice` trait with read, write, and
block-size operations — so the filesystem layer above doesn't care which kind of
disk it's talking to.

## USB

USB is the most involved driver. The xHCI controller driver, in `drivers/usb/xhci`,
manages the host controller's ring-based command and event interface, allocates
device slots and endpoints, and enumerates attached devices. On real hardware this
required handling quirks that emulators simply don't have — writing certain
configuration only while the controller is halted, and explicitly resetting USB 2
ports to bring them into an active state.

On top of the host controller sit two class drivers. The HID driver, in
`drivers/usb/hid`, interprets keyboard and mouse reports, translating HID usage
codes into characters and mouse deltas into cursor movement. The mass-storage
driver, in `drivers/usb/storage`, speaks the Bulk-Only Transport protocol,
wrapping SCSI commands in command blocks to read and write USB drives — which also
expose themselves through the same `BlockDevice` interface as the internal disks.

## Input, audio, and the rest

For legacy input there is a PS/2 stack: a keyboard driver in `drivers/ps2/keyboard`
with scancode tables and shift/caps handling, and a mouse driver in
`drivers/ps2/mouse` that assembles three-byte packets through a small state machine
and moves the cursor. Both feed into the same event queue the USB HID driver uses,
so the rest of the system doesn't care where a keypress or a mouse movement came
from.

Audio has two backends behind a common module: an Intel HDA driver in
`drivers/audio/hda` that resets the controller, walks the codec to find a DAC and
output pin, and streams PCM through a buffer descriptor list, and an AC'97 driver
in `drivers/audio/ac97` as a fallback. Finally, `drivers/rtc` reads the real-time
clock through the CMOS ports and applies a timezone offset, and `drivers/power`
handles reboot and ACPI S5 shutdown. Throughout, new hardware is polled rather than
interrupt-driven — the USB event ring and the audio tick are both serviced from the
timer — a deliberate choice that keeps the driver logic synchronous and easier to
reason about.

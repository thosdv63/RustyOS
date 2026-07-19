# 09 — Userland and Syscalls

The userland is where Rusty OS stops being a kernel and starts being something a
person uses. It is compiled separately from the kernel into a flat binary called
`core.bin`, loaded at a fixed low address, and run in ring 3 — unprivileged, unable
to touch hardware directly, and reaching the kernel only through system calls.

## Getting started

Because the userland is a raw binary rather than a normal program with a loader,
it has to prepare its own environment before anything else runs. Its entry point,
in the userland's `main`, is a small piece of assembly that does two essential
things: it zeroes the BSS section, and it aligns the stack before calling into
Rust.

The BSS detail is a genuine war story. In a flat binary the BSS — the space for
zero-initialized statics — isn't stored in the file. On a cold boot this happened
to work, because the machine's RAM started out zeroed. But on a warm reboot the RAM
still held the previous session's garbage, so those statics, including the heap
allocator's own state, would come up corrupted. The fix was to zero the BSS by hand
as the very first thing the userland does, before any Rust code that might depend on
it. It's the kind of bug that's invisible until the day someone reboots without
powering off, and then it's baffling.

Once the environment is sound, the userland initializes its heap, fetches the
framebuffer information from the kernel, constructs a renderer, and — if setup hasn't
been done — runs the OOBE wizard and the login screen before building the desktop.

## The syscall ABI

Every interaction with the kernel goes through a system call. The convention is
compact: the call number goes in `rax`, arguments in `rdi` and `rsi`, and the
`syscall` instruction makes the jump. On the userland side, `syscall.rs` wraps each
one in a small function, carefully marked so the compiler treats each call as
distinct and preserves the registers the instruction clobbers.

The calls cover everything the userland needs and nothing it doesn't:

| # | Call | Purpose |
|---|------|---------|
| 0 | print | write text to the screen (early debug) |
| 2 | get framebuffer | fetch screen base, size, stride, back buffer |
| 3 | poll event | pull the next keyboard or mouse event |
| 4 | get time | read the real-time clock |
| 5 | power | shut down or reboot |
| 6–7 | registry cached read / set color | fast value access and desktop color |
| 8–13 | directory & file ops | list, create, delete, rename, mkdir, move |
| 14–15 | registry list / set | dump or set a registry line |
| 16–17 | read / write file | file contents in and out |
| 18–20 | audio | play startup sound, play a file, stop |
| 21 | sysinfo | RAM and CPU statistics |

A recurring pattern is how variable-length data crosses the boundary: paths and
file contents are packed into a buffer with a small length header, and the kernel
unpacks them on the other side. It's a modest ABI, but it's enough to build a file
manager, a text editor, and everything else on top of.

## The event loop and the app framework

At the userland's heart is a single loop. Each pass drains every pending input
event through the poll-event syscall, routes it to the desktop or the focused
application, and — if anything changed — redraws and presents. When only the mouse
moved, it takes the cheap path, restoring and repainting just the cursor's patch;
when a window changed, it repaints and presents the affected region.

Applications plug into this through a small contract, the `App` trait in
`app_compiler`: an application provides a title, a `draw` routine that renders into
a given rectangle, and an `on_event` handler that receives clicks, drags, and
keypresses in body-local coordinates and returns whether it needs to be redrawn.
Every built-in application implements exactly this trait, which is what lets the
window manager treat them all uniformly — a clean seam between the desktop and the
programs running inside it.
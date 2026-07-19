# 11 — Applications

An operating system is only as convincing as the things you can actually do on it,
and Rusty OS ships with a full set of built-in applications. They all live under
the userland's `apps` module, and every one of them implements the same `App`
trait — a title, a draw routine, an event handler — so the window manager treats
them uniformly. What varies is what each one does with the syscalls available to
it.

## File Explorer

The file explorer, in `apps/explorer`, is the most feature-complete of the set. It
lists the contents of a directory through the directory syscall, showing drives,
folders, and files with type and size. It keeps navigation history so back and
forward work, an address breadcrumb, and a sidebar of shortcuts. It supports
double-clicking to open — folders navigate in, and files launch the right
application, sending a `.BMP` to the image viewer, a `.RAW` to the audio driver, and
text-like files to the editor. It has a right-click context menu with cut, paste,
delete, and rename, and — importantly — it guards critical system files, refusing to
delete the registry, the kernel, `CORE.BIN`, or the system directories, with a
confirmation dialog for anything risky.

## Editing and drawing

Notepad, in `apps/notepad`, is a real text editor: it loads and saves files, tracks
lines and a cursor, scrolls both directions, and offers save, save-as, and new, with
a confirmation prompt when there are unsaved changes. Paint, in `apps/paint`, is a
small drawing program with a color palette, adjustable brush sizes, an eraser, and
the ability to open and save images — it draws strokes with a Bresenham line so
dragging produces a continuous mark, and reads and writes real BMP files.

## Utilities

The calculator, in `apps/hesap`, is a working floating-point calculator with the
usual operations and keyboard support. The registry editor, in `apps/regedit`, lists
every registry key and lets you edit values in place or add new ones, validating that
a value matches its declared type. The task manager, in `apps/gorevmgr`, shows live
CPU and RAM usage from the sysinfo syscall alongside the list of open windows, and
can terminate a selected one. The settings panel, in `apps/ayarlar`, offers a
categorized view — appearance, system, sound, power — where you can change the
desktop color, rename the user, test audio, and shut down or restart.

## Command Prompt

The command prompt, in `apps/cmd`, is a genuine shell with around twenty-five
commands. It resolves relative and absolute paths, changes drives the way Windows
does, and implements the familiar set: `dir`, `cd`, `cls`, `echo`, `type`, `copy`,
`del`, `ren`, `mkdir`, `move`, `date`, `time`, `ver`, and `color`, plus Rusty-specific
ones — `reg` to query the registry, `tasklist` and `taskkill` to manage windows,
`start` to launch applications, and `shutdown`. It even carries the classic sixteen-
color palette for its `color` command.

## Viewing and supporting code

The image viewer, in `apps/resim`, displays BMP images with a fit-to-window or
one-to-one toggle, and the about window, in `apps/hakkinda`, shows the version and —
as a small proof that the registry is live — reads the current desktop color back
out of it. Underneath several of these sits a shared BMP codec, in `apps/bmp`, that
decodes and encodes 24- and 32-bit uncompressed bitmaps, which is what lets Paint,
the image viewer, and the explorer all speak the same image format.

Taken together, these applications turn the kernel and desktop underneath them into
something that feels like a real, usable computer — you can browse files, write and
draw, do arithmetic, inspect the system, and drop to a command line, all without
ever leaving Rusty OS.
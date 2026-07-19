# 10 — Desktop Environment

The desktop is the part of Rusty OS a user actually sees and touches, and it's
built entirely in the userland from the primitives described so far. It lives under
the userland's `ui` module and it's a faithful, hand-drawn take on the Windows 7
desktop — windows you can drag and resize, a taskbar with a round start button, a
two-panel start menu, and icons on the desktop you can select, move, and rename.

## Windows and the window manager

A window, defined in `ui/window`, is a rectangle with a title, a body, a state
(normal, maximized, or minimized), and the usual title-bar controls. It knows how
to draw itself — the glossy title bar, the rounded frame, the minimize, maximize,
and close buttons — and how to figure out which region a click landed in.

The window manager, in `ui/window_mgr`, owns the list of open windows and treats
that list as z-order: the last window is the frontmost. It handles dragging a window
by its title bar, clamping it so it can't be lost off-screen or behind the taskbar;
it handles maximize and restore, remembering the previous position; and it brings a
clicked window to the front. It also draws the taskbar buttons for open windows and
maps clicks on them back to restoring or focusing the right window.

## The taskbar and start menu

The taskbar, in `ui/taskbar`, is the dark bar across the bottom with a bright edge,
a round glossy start orb with a hand-drawn "R," and a clock reading the real-time
clock for the time and date. Clicking the orb toggles the start menu.

That menu, in `ui/taskbar_manager`, is the two-panel Windows 7 layout: a white
panel of applications on the left — each with its own little hand-drawn icon — and
a translucent panel on the right with the user's avatar, shortcuts to their folders,
and shut-down and restart buttons. Selecting an application from the menu asks the
system to launch it, which the main loop picks up and turns into a new window.

## The desktop surface

The desktop background itself, in `ui/desktop`, is the gradient wallpaper with a
large glossy "R" logo and the Rusty OS name. Layered over it, `ui/desktop_manager`
handles the interactive icons — Computer, Recycle Bin, and whatever files and
folders live on the user's desktop. It supports selecting an icon, drag-selecting a
group with a rubber-band rectangle, dragging icons to new positions, double-clicking
to open, a right-click context menu for creating and deleting items, and inline
renaming. Opening a folder or a file routes a request back through the system to
launch the file explorer or the right application.

## Tying it together

The glue between windows and the programs inside them is the application manager, in
`ui/app_mgr`. When something asks to launch an application, it constructs the right
one, opens a window for it, and remembers the pairing. On each frame it draws each
application into its window's body, and it routes clicks, drags, and keypresses to
whichever application owns the focused window — translating screen coordinates into
the body-local coordinates the application expects. It also cleans up when windows
close and handles requests from one application to launch another, like the file
explorer opening a text file in the editor.

Two screens bookend the whole experience. Before the desktop ever appears, the OOBE
wizard in `ui/oobe` walks a first-time user through choosing a name, an optional
password, and a desktop color, then writes those into the registry and creates the
user's folders. After that, the login screen in `ui/login` presents the user's
avatar and, if a password was set, asks for it before letting them through to the
desktop. A shared theme module keeps every color in one place, so the whole
environment stays visually consistent.
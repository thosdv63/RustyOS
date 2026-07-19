# 08 — Graphics

Everything visible in Rusty OS is drawn one pixel at a time. There is no GPU
acceleration and no windowing library — just a linear framebuffer handed over by
the firmware, and code that writes colors into it. The interesting engineering is
in doing that fast enough and cleanly enough that a full Aero-style desktop feels
responsive.

## The framebuffer and double buffering

The bootloader captured the framebuffer's address, dimensions, and stride and
passed them to the kernel, which forwards them to the userland. Drawing directly to
that framebuffer, though, would cause visible tearing and flicker, because a
half-drawn frame would be shown on screen. Rusty OS solves this the standard way,
with double buffering. Everything is drawn into a back buffer in ordinary memory,
and only when a frame is complete is it copied to the real framebuffer — an
operation called presenting.

Two addresses recur here: the back buffer at `0x10000000` and the physical
framebuffer at `0x80000000`. The renderer draws into the former and presents to the
latter. Presenting a whole frame is a single fast memory copy, but the renderer can
also present just a rectangle — copying only a changed region — which is what makes
cursor movement and small updates cheap instead of repainting the entire screen
every time.

## The renderer

There are actually two renderers — one in the kernel for early boot and panic
screens, and a richer one in the userland for the desktop — but they share the same
foundation. At the bottom is a single-pixel write that bounds-checks against the
screen and, in the userland's case, converts the color into the framebuffer's byte
order. On top of that are the primitives everything else is built from: filled
rectangles, gradients, lines, and circles.

The userland renderer goes further, into the territory that gives the desktop its
Windows 7 look. It has vertical gradients, glossy fills that fade light-to-dark
with a highlight along the top edge, rounded rectangles, and alpha blending that
mixes a new color with whatever is already on the buffer. Together these produce
the glass-like buttons, the rounded window frames, and the translucent selection
rectangles that define the Aero aesthetic — all computed by hand, per pixel.

## Text

Text is drawn from an 8×8 bitmap font. Each character is eight bytes, one per row,
with each bit deciding whether a pixel is on. The renderer draws a character by
walking those bits and filling pixels, optionally scaled up by an integer factor so
the same font serves both small labels and large headings. The kernel and userland
carry slightly different font tables — the kernel's is oriented around the scancode
set it receives, the userland's around plain ASCII — but the drawing approach is
the same.

## The cursor

The mouse cursor is the one thing that moves constantly, and redrawing the whole
screen to follow it would be wasteful. Instead the renderer saves the small patch
of pixels underneath the cursor before drawing it — a 12×19 region — and restores
that patch when the cursor moves, before saving and drawing at the new position.
This save-restore trick means the cost of moving the mouse is proportional to the
size of the cursor, not the size of the screen, and it's why the pointer glides
smoothly even though every pixel is being managed by software.

```
   draw into back buffer  ─►  present (copy to framebuffer)
        ▲                          │
        │  save patch under cursor │  present only changed rect
        └──────────────────────────┘
```
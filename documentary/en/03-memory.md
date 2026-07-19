# 03 — Memory

Once the kernel has control, almost nothing else can happen until memory works.
Every allocation, every driver buffer, every window on screen ultimately comes
from the memory subsystem, and Rusty OS builds it in three layers: a physical
frame allocator that hands out 4 KB pages of real RAM, a page-table manager that
maps virtual addresses to those frames, and a heap allocator that carves small
objects out of a mapped region.

## The physical frame allocator

The lowest layer, in `mm/pfa`, tracks physical memory with a bitmap — one bit per
4 KB frame, set when the frame is in use and clear when it's free. At startup it
reads the memory map the bootloader passed in, finds the highest usable address,
and sizes the bitmap to cover all of RAM up to a four-gigabyte ceiling. The bitmap
itself has to live somewhere, so the allocator places it in the largest usable
region it can find, then marks everything as used and frees only the regions the
firmware reported as conventional memory.

What makes this allocator interesting is what it refuses to give out. Several
regions are reserved by hand. The first five megabytes — where the userland binary
and its stack live — are permanently marked used, and the allocator's normal
handout routine starts above them entirely. There is also a one-megabyte fallback
region reserved for audio. The reason is subtle but important: DMA-capable
hardware writes directly to physical memory, and if a disk or audio transfer were
handed a frame inside the running userland's memory, it would silently corrupt a
live program. By carving those regions out of the allocator's reach, an entire
category of impossible-to-debug crashes is designed away.

## The page-table manager

Above the frame allocator, `mm/ptm` deals in virtual memory. Rusty OS relies on
the identity mapping the firmware set up — virtual addresses equal physical
addresses — and extends it as needed. The manager can map a single page to a
frame, unmap it, or map a whole range at once, pulling fresh frames from the
allocator for each page. It also knows how to mark pages as user-accessible, which
matters when the kernel needs to expose memory to the ring-3 userland.

One recurring detail here is a small dance with the processor's write-protect bit.
The kernel runs with write protection enabled, which normally prevents even
ring-0 code from writing through read-only mappings. To edit page tables safely,
the manager briefly clears that bit, performs the mapping, and restores it —
bracketing each change so the protection is only ever off for an instant.

## The heap

The top layer, in `mm/heap`, is a linked-list allocator that satisfies the small,
frequent allocations that Rust's `alloc` types depend on — every `Vec`, `String`,
and `Box` in the kernel. On its first use it maps a one-megabyte region at a fixed
high address and seeds its free list with that single block. From then on,
allocation walks the list looking for a block large enough, splits off what it
needs, and returns the remainder to the pool; freeing simply pushes the region
back onto the list.

The kernel and the userland each have their own heap at their own fixed address —
the kernel's high in its address space, the userland's higher still. They share no
state, which keeps the boundary between privileged and unprivileged code clean:
the userland allocates from memory the kernel mapped for it, and never touches the
kernel's own pool.

```
   heap (Vec, String, Box)          ← small objects
        │ mapped into
   page-table manager (virtual → physical)
        │ frames from
   frame allocator (bitmap of 4 KB pages of RAM)
```
# 03 — Memory

When the kernel takes control, almost nothing can happen without memory working. Every allocation, every driver buffer, every window on screen ultimately comes from the memory subsystem, and Rusty OS builds this in three layers: a physical frame allocator that manages RAM using a Buddy Allocator algorithm, a virtual memory manager (VMM) that handles 4-level page tables and huge pages, and a heap allocator that carves dynamic objects out of a mapped virtual region.

## Physical Frame Allocator

At the lowest layer, `mm/pfa` manages physical memory using a **Buddy Allocator** algorithm. Instead of a bitmap approach, memory is tracked using block lists divided into orders ranging from 0 to 20 (`MAX_ORDER = 20`) — while Order 0 corresponds to a single 4 KB page, larger blocks like Order 9 (2 MB) and Order 18 (1 GB) can be allocated in a single step. At boot, it scans the memory map provided by the bootloader via `BootInfo` and aligns usable (`usable == 1`) regions before adding them to the free lists (`free_lists`) at the appropriate order.

For hardware compatibility and system stability, the first 1 MB region of physical memory (below `0x100000`) is kept entirely outside the allocator's control. Free block structures in physical memory are managed using the **HHDM (Higher Half Direct Map)** technique; physical addresses are converted into virtual addresses using the `0xFFFF_8000_0000_0000` offset, allowing free block nodes (`FreeBlock`) to be manipulated directly in memory.

## Page Table Manager

Above the frame allocator, `mm/vmm` manages the 4-level (PML4, PDPT, PD, PT) x86_64 virtual memory architecture. It handles mapping (`map`), unmapping (`unmap`), and address translation (`translate`) from virtual addresses to physical frames. Alongside standard 4 KB pages, it supports 2 MB and 1 GB **Huge Pages** for high-performance requirements.

The virtual memory manager accesses lower-level page tables directly via the HHDM offset. On the security side, the **NXE (No-Execute Enable)** feature is activated to prevent code execution in data regions using the `PTE_NO_EXECUTE` bit. Additionally, memory access is strictly controlled using flags such as `PTE_USER` (Ring-3 access), `PTE_WRITABLE`, and cache disabling (`PTE_NO_CACHE`, `PTE_WRITE_THROUGH`). Every page table modification flushes the TLB using the `invlpg` instruction.

## Heap

At the top layer, `mm/heap` houses the `LockedHeap` allocator, synchronized via `spin::Mutex`, which serves Rust's `alloc` types (`Vec`, `String`, `Box`). It implements Rust's `GlobalAlloc` interface.

Operating on a lazy initialization pattern, the heap maps a 1 MB region (`HEAP_SIZE`) at virtual address `0x4444_0000_0000` (`HEAP_START`) to physical frames via `vmm::map_range` upon the first allocation request. Internally, it uses a linked list (`FreeBlock`) structure; incoming allocation requests are satisfied from the best matching free block based on size and alignment (`align_up`) requirements, while any excess memory is returned to the pool as a free block.

```
   heap (Vec, String, Box)           ← LockedHeap (0x4444_0000_0000 / 1 MB)
       │ mapped via VMM map_range
   virtual memory manager (VMM)      ← 4-Level Page Table (4KB / 2MB / 1GB, NXE, HHDM)
       │ frames requested from here
   frame allocator (Buddy Alloc)     ← Order 0-20 (4KB - 4GB), HHDM-based

```

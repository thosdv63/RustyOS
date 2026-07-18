ENTRY(_start)
SECTIONS {
    . = 0x400000;
    .text : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    . = ALIGN(16);
    __bss_start = .;
    .bss : { *(.bss .bss.*) *(COMMON) }
    . = ALIGN(16);
    _end = .;
    /DISCARD/ : { *(.comment) *(.eh_frame) *(.note .note.*) }
}
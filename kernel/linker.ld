ENTRY(_start)

KERNEL_VMA = 0x100000;

SECTIONS {
    . = KERNEL_VMA;

    .text ALIGN(0x1000) : {
        *(.text .text.*)
    }

    .rodata ALIGN(0x1000) : {
        *(.rodata .rodata.*)
    }

    .data ALIGN(0x1000) : {
        *(.data .data.*)
    }

    .bss ALIGN(0x1000) : {
        *(.bss .bss.*)
        *(COMMON)
    }
}
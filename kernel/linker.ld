ENTRY(_start)
SECTIONS {
    . = 0x0;
    .text : { *(.text._start) *(.text .text.*) }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) }
    __kernel_end = .; /* end of the kernel image (STAT_PHYSMAP) */
    /DISCARD/ : { *(.eh_frame) }
}

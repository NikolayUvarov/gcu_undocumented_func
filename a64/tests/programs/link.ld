/* The differential program at a fixed address (215-PRT-0016). */
ENTRY(_start)
SECTIONS {
    . = 0x100000;
    .text : { *(.text._start) *(.text .text.*) }
    .rodata : ALIGN(16) { *(.rodata .rodata.*) }
    .data : ALIGN(16) { *(.data .data.*) }
    .bss : ALIGN(16) { *(.bss .bss.*) *(COMMON) }
}

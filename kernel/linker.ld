ENTRY(_start)
SECTIONS {
    . = 0x0;
    .text : { *(.text._start) *(.text .text.*) }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) }
    __kernel_end = .;
    /DISCARD/ : { *(.eh_frame) }
}

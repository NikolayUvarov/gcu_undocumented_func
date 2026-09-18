ENTRY(_start)
SECTIONS {
    . = 0x0;
    .text : {
        *(.text._start)
        *(.text .text.*)
    }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) }
    /DISCARD/ : {
        *(.eh_frame)
        *(.note.gnu.build-id)
        *(.dynamic)
        *(.dynsym)
        *(.hash)
        *(.gnu.hash)
        *(.dynstr)
        *(.rela.dyn)
        *(.got)
        *(.got.plt)
    }
}

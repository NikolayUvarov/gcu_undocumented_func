ENTRY(_start)
SECTIONS {
    . = 0x0;
    .text : {
        *(.text._start)
        *(.text .text.*)
    }
    .rodata : { *(.rodata .rodata.*) }
    .data : {
        BYTE(0xFF); /* Форсируем флаг PROGBITS для секции */
        *(.data .data.*)
        *(.bss .bss.*)
    }
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

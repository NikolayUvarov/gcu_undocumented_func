ENTRY(_start)
PHDRS {
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
    dynamic PT_DYNAMIC FLAGS(6);
    stack PT_GNU_STACK FLAGS(6);
}
SECTIONS {
    . = 0;
    .text : { *(.text._start) *(.text .text.*) } :text
    . = ALIGN(4096);
    .rodata : { *(.rodata .rodata.*) } :rodata
    .mind_request : { KEEP(*(.mind_request)) } :rodata
    .mind_about : { KEEP(*(.mind_about)) } :rodata
    .dynsym : { *(.dynsym) } :rodata
    .gnu.hash : { *(.gnu.hash) } :rodata
    .hash : { *(.hash) } :rodata
    .dynstr : { *(.dynstr) } :rodata
    .rela.dyn : { *(.rela.dyn) } :rodata
    . = ALIGN(4096);
    .data : { *(.data .data.*) *(.got .got.*) } :data
    .dynamic : { *(.dynamic) } :data :dynamic
    .bss (NOLOAD) : { *(.bss .bss.*) } :data
    /* The stack it asks for (PT_GNU_STACK, 262-KRN-0164): main's frame holds two controllers' hwdocs tables, about 80 KiB
       on aarch64, past the default 64 KiB (100000-KRN-0171 found it, gpio's first start in QEMU). */
    .stack (NOLOAD) : { . = . + 0x40000; } :stack
    /DISCARD/ : { *(.eh_frame) }
}

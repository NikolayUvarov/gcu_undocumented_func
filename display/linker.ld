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
    /* display's stack, PT_GNU_STACK's size (262-KRN-0164): 256 KiB, as step 3's frames hold the VBIOS interpreter, the
       undo's journal and the stages' plans (light::STEP3_STACK_BYTES), past the 64 KiB a task has without it. */
    .stack (NOLOAD) : { . = . + 0x40000; } :stack
    /DISCARD/ : { *(.eh_frame) }
}

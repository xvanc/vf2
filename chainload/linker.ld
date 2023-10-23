
SECTIONS {
    . = 0;
    PROVIDE(__image_base = .);

    PROVIDE(__text = .);
    .text : {
        *(.text._start)
        *(.text .text.*)
    }
    PROVIDE(__etext = .);

    .rodata                 : { *(.rodata .rodata.*) }
    .dynsym                 : { *(.dynsym) }
    .dynstr                 : { *(.dynstr) }
    .rela                   : { *(.rela*) }
    .dynamic                : { *(.dynamic) }
    .got                    : { *(.got) }
    .data.rel.ro            : { *(.data.rel.ro*) }
    .data                   : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    PROVIDE(__global_pointer$ = .);
    PROVIDE(__bss = .);
    .bss                    : {
        *(.sbss .sbss.*)
        *(.dynbss)
        *(.bss .bss.*)
        *(COMMON)
        . += 0x100000;
        . = ALIGN(128);
        PROVIDE(__boot_stackp = .);
    }
    PROVIDE(__ebss = .);

    /DISCARD/ : {
        *(.eh_frame_hdr)
        *(.eh_frame)
    }
}

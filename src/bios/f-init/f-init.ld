ENTRY(loader)

SECTIONS {

    . = 0x200;
    .text : { *(.startup*) *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) }
    /DISCARD/ : { *(.eh_frame*) }

}

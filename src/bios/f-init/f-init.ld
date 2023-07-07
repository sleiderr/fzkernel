MEMORY {

    bootinit : ORIGIN = 0x00, LENGTH = 0x07E00

}

ENTRY(loader)

SECTIONS {

    . = 0x200;
    .text : { *(.startup*) *(.text*) *(.got.plt*) } >bootinit
    .rodata : { *(.rodata*) } >bootinit
    .data : { *(.data*) } >bootinit
    .bss : { *(.bss*) }
    .fill : {

        FILL(0x00);
        . = 0x7e00;

    } > bootinit
    /DISCARD/ : { *(.eh_frame*) }

}

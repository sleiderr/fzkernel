ENTRY(_start);

SECTIONS {
    _boot_start = .;
    .text : { *(.startup*) *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    _boot_end = .;

    . = 446;
    _part_table = .;

    . = 510;
    .magic_number : {
        SHORT(0xaa55);
    }
    /DISCARD/ : {
        *(.eh_frame* *.got.plt*)
    }
}

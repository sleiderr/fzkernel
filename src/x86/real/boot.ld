ENTRY(_start);

SECTIONS {
    _boot_start = .;
    .text : { *(.text*) }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    _boot_end = .;

    . = 446;
    _part_table = .;

    . = 510;
    .magic_number : {
        SHORT(0xaa55);
    }
    . = ALIGN(512);
    .pad : {
        . = . + (511 - (. % 512)) % 512;
        BYTE(0)
    }
    /DISCARD/ : {
        *(.eh_frame* *.got.plt*)
    }
}
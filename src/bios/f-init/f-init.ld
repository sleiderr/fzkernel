ENTRY(_start)

SECTIONS {

    . = 0x200;

    .start : {
        *(.start)
    }

    .text : {
        *(.text .text.*)
    }

    .bss : {
        *(.bss .bss.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }

    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }

    .fill : {

        FILL(0x00);
        . = 0x7e00;

    }

}

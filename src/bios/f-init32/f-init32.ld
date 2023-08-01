ENTRY(_entry32)

SECTIONS {

    . = 0x5e00 + 0x7c00;

    .text : {
        *(.text .text.*)
    }

    .start : {
        *(.start)
    }
    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        _bss_start = .;
        *(.bss .bss.*)
        _bss_end = .;
    }

    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }

    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }

    .fill : {
        FILL(0x00);
        . = 0x7e00 + 0x7c00;
    }


}

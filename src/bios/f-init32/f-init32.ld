ENTRY(_entry32)

SECTIONS {

    . = 0x7e00 + 0x7c00;

    .start : {
        *(.start)
    }

    .interrupts : {
        KEEP(*(.int*))
    }

    .text : {
        *(.text .text.*)
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
        FILL(0xdeadc0de);
        . = 0x14000 + 0x7c00;
    }


}

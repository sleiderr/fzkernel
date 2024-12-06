ENTRY(_start)

SECTIONS {

    . = 0xFFFF8C0000000000;
    
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

    _bss_start = .;
    .bss : {
        *(.bss .bss.*)
    }
    _bss_end = .;

    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }

    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }

}

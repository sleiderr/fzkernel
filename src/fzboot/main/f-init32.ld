ENTRY(_entry32)

MEMORY {
main : ORIGIN = 0x5f00 + 0x7c00, LENGTH = 0x100000
}



SECTIONS {

    . = 0x5f00 + 0x7c00;

    .start : {
        *(.start)
    } > main

    .interrupts : {
        KEEP(*(.int*))
    }> main

    .tests : {
            KEEP(*(.test*))
        }> main

    .text : {
        *(.text .text.*)
    }> main

    .rodata : {
        *(.rodata .rodata.*)
    }> main

    .data : {
        *(.data .data.*)
    }> main

    _bss_start = .;
    .bss : {
        *(.bss .bss.*)
    }> main
    _bss_end = .;

    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }> main

    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }> main

    .fill : {
            FILL(0xdeadc0de);
            . = 0x100000;
        }> main

}

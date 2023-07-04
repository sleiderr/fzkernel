MEMORY {
    body : ORIGIN = 0, LENGTH = 442
    size : ORIGIN = 442, LENGTH = 2
    slba : ORIGIN = 444, LENGTH = 2
    boot : ORIGIN = 510, LENGTH = 2
}

ENTRY(_start);

SECTIONS {
    .text : { *(.startup*) *(.text*) } > body
    .rodata : { *(.rodata*) } > body
    .data : { *(.data*) } > body
    .bss : { *(.bss*) } > body
    .size : { KEEP(*(.size*)) } > size
    .slba : { KEEP(*(.slba*)) } > slba
    .boot : { KEEP(*(.boot*)) } > boot
    /DISCARD/ : { *(.eh_frame*) *(.comment*) *(.got.plt*) *(.got) }

}
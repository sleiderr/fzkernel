ENTRY(_start)

SECTIONS {
    load_addr = 0xB000;
    pm_jump_addr = (load_addr + pm_entry);
    .text : { *(.text*) }
    . = ALIGN(512);
        .pad : {
            . = . + (511 - (. % 512)) % 512;
            BYTE(0)
        }
}
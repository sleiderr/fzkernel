use core::arch::asm;

fn __kb_enable_a20() -> Result<(), ()> {


    if __a20_check() {
        return Ok(());
    }
    Err(())
}

fn __bios_enable_a20() -> Result<(), ()> {

    unsafe {
        asm!(
        "mov ax, 0x2401",
        "int 0x15"
        );
    }

    if __fast_a20_check() {
        return Ok(());
    }
    Err(())

}

fn __a20_check() -> bool {



}

fn __fast_a20_check() -> bool {

    let result: u16;

    unsafe {
        asm!(
        "cli",
        "xor ax, ax",
        "mov es, ax",
        "not ax",
        "mov ds, ax",
        "mov di, 0x0500",
        "mov si, 0x0510",
        "mov byte [es:di], 0x00",
        "mov byte [ds:si], 0xFF",
        "cmp byte [es:di], 0xFF",
        "mov ax, 0",
        "sti",
        "je 4",
        "mov ax, 1",
        "4: ret",
        out("ax") result
        );
    }

    if result == 0x00 {
        return false;
    }

    return true;

}

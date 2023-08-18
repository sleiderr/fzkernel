use core::arch::asm;
use fzboot::{
    errors::GenericError,
    x86::int::{disable_interrupts, enable_interrupts},
};
use fzboot::{
    errors::{CanFail, IOError},
    io::{inb, outb},
};

use fzboot::io::io_delay;
use fzboot::io::ps2::{input_wait, output_wait, read_ps2, send_data, send_ps2};

const A20_KTEST_LOOPS: u16 = 32;

pub fn enable_a20() -> GenericError {
    if __fast_a20_check() {
        return Ok(());
    }

    __bios_enable_a20()
        .or_else(|_| __kb_enable_a20())
        .or_else(|_| __fastg_enable_a20())
        .map_err(|_| ())
}

fn __fastg_enable_a20() -> GenericError {
    let mut sysctrl_prt_a = inb(0x92);
    outb(0x92, sysctrl_prt_a | 2);

    if __fast_a20_check() {
        return Ok(());
    }

    Err(())
}

fn __kb_enable_a20() -> CanFail<IOError> {
    disable_interrupts();

    input_wait(A20_KTEST_LOOPS)?;
    send_ps2(0xAD);

    input_wait(A20_KTEST_LOOPS)?;
    send_ps2(0xD0);

    output_wait(A20_KTEST_LOOPS)?;
    let ctrl_output: u8 = read_ps2();

    input_wait(A20_KTEST_LOOPS)?;
    send_ps2(0xD1);

    input_wait(A20_KTEST_LOOPS)?;
    send_data(ctrl_output | 2);

    input_wait(A20_KTEST_LOOPS)?;
    send_ps2(0xAE);

    if __a20_check(A20_KTEST_LOOPS) {
        return Ok(());
    }

    Err(IOError::IOTimeout)
}

fn __bios_enable_a20() -> GenericError {
    unsafe {
        asm!("mov ax, 0x2401", "int 0x15");
    }

    if __fast_a20_check() {
        return Ok(());
    }
    Err(())
}

fn __a20_check(mut loops: u16) -> bool {
    while loops > 0 {
        if __fast_a20_check() {
            return true;
        };
        io_delay();
        loops -= 1;
    }

    return false;
}

fn __fast_a20_check() -> bool {
    let result: u16;

    disable_interrupts();
    unsafe {
        asm!(
        "push es",
        "push ds",
        "push di",
        "push si",
        "xor ax, ax",
        "mov es, ax",
        "not ax",
        "mov ds, ax",
        "mov si, 0x0510",
        "mov di, 0x0500",
        "mov BYTE PTR es:[di], 0x00",
        "mov BYTE PTR ds:[si], 0xFF",
        "cmp BYTE PTR es:[di], 0xFF",
        "pop si",
        "pop di",
        "pop ds",
        "pop es",
        "mov ax, 0",
        "je 1f",
        "mov ax, 1",
        "1: nop",
        out("ax") result
        );
    }
    enable_interrupts();

    if result == 0x00 {
        return false;
    }

    return true;
}

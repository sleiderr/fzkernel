.code16

.global _start
_start:
xor ax, ax
mov es, ax
lea si, [entry_str]
call boot_log
call enable_a20
lea si, [a20_enable_str]
call boot_log
call setup_vesa
call setup_gdt
call pm_jump

boot_log:
    push si
    lea si, [log_prefix]
    call _print_str
    pop si
    call _print_str
    lea si, [newline]
    call _print_str
    ret

_print_str:
    push ax
    mov ah, 0xe
    .loop:
        lodsb
        and al, al
        jz str_end
        int 0x10
        jmp .loop
    str_end:
    pop ax
    ret

a20_enable_str:
    .ascii "A20 gate is enabled"
    .byte 0

log_prefix:
    .ascii "boot-init: "
    .byte 0

newline:
    .byte 13
    .byte 10
    .byte 0

entry_str:
    .ascii "real mode kernel bootstrap"
    .byte 0

#include "a20.S"
#include "mem.S"
#include "vesa.S"

.code32
.globl pm_entry
pm_entry: push 0x30000
ret
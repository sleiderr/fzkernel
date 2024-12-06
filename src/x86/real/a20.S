enable_a20:
    call enable_a20_bios
    call check_a20_status
    jnz exit
    call enable_a20_8042
    call check_a20_status
    jnz exit
    call enable_a20_bios
    call check_a20_status
    jnz exit

enable_a20_bios:
    mov ax, 0x2401
    int 0x15

enable_a20_8042:
    cli
    mov al, 0xAD
    out 0x64, al
    call io_wait
    mov al, 0xD0
    out 0x64, al
    call io_wait
    in al, 0x60
    push ax
    call io_wait
    mov al, 0xD1
    out 0x64, al
    call io_wait
    pop ax
    or al, 2
    mov al, 0xAE
    out 0x64, al
    call io_wait
    sti
    ret

io_wait:
    in al, 0x64
    test al, 2
    jnz io_wait
    ret

# Returns 1 in ax if the A20 line is enabled.
check_a20_status:
    push ds
    push di
    push si
    xor ax, ax
    not ax
    mov ds, ax
    mov si, 0x0510
    mov di, 0x0500
    mov BYTE PTR es:[di], 0x00
    mov BYTE PTR ds:[si], 0xFF
    cmp BYTE PTR es:[di], 0xFF
    pop si
    pop di
    pop ds
    mov ax, 0
    je exit
    mov ax, 1

exit: ret
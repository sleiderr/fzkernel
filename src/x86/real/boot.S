#define BOOT_RM_SECTORS_COUNT 0x4
#define MAX_SECTORS_PER_READ 0x7F
#define MAX_DISK_OP_RETRIES 5

#define POST_MBR_LOAD_SEG 0x0B00

.code16
ljmp 0x07c0, offset stack_setup

stack_setup:
mov ax, cs
mov ds, ax
mov es, ax
mov ss, ax
xor sp, sp

_boot:
    call clear
    call check_edd_support
    jc io_error

ext_load_boot:
    mov ah, 0x42
    xor al, al
    mov cx, 1
    mov bx, BOOT_SECTORS_COUNT
    read_loop: test bx, bx
    inc al
    cmp al, MAX_DISK_OP_RETRIES
    jg io_error
    jz read_complete
    cmp bx, MAX_SECTORS_PER_READ
    jle non_max_count
    mov WORD PTR dap_sectors_count, MAX_SECTORS_PER_READ
    jmp issue_read
    non_max_count: mov dap_sectors_count, bx
    issue_read: mov dap_lba_lo, cx
    lea si, [dap]
    int 0x13
    jc read_loop
    sub bx, dap_sectors_count
    add WORD PTR dap_segment, 0xfe0
    jmp read_loop
    read_complete:
    mov ax, POST_MBR_LOAD_SEG
    mov ds, ax
    ljmp POST_MBR_LOAD_SEG, 0x0
    jmp boot_fail

check_edd_support:
    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13
    ret

clear:
     push ax
     xor ah, ah
     mov al, 0x03
     int 0x10
     pop ax
     ret

puts:
    mov ah, 0xe
    .loop:
        lodsb
        and al, al
        jz str_end
        int 0x10
        jmp .loop
    str_end:
    ret

io_error:
    lea si, [io_err_msg]
    call puts

boot_fail:
    lea si, [reboot_msg]
    call puts
    xor ax, ax
    int 0x16
    int 0x19

drive_id: .byte 0x00

dap:
    .byte 0x10
    .byte 0x00
    dap_sectors_count: .word 0x0000
    dap_offset:        .word 0x0000
    dap_segment:       .word POST_MBR_LOAD_SEG
    dap_lba_lo:        .word 0x0000
    dap_lba_hi:        .word 0x0000
    .word 0x00
    .word 0x00

io_err_msg:
    .ascii "Failed to read from disk"
    .byte 13
    .byte 10
    .byte 0

reboot_msg:
    .ascii "Press any key to reboot..."
    .byte 13
    .byte 10
    .byte 0

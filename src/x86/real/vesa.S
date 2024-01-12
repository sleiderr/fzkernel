#define VESA_BUF 0x5000

#define VBE_INFO_MODE_PTR_OFF 0xE
#define VBE_MODE_INFO_WIDTH_OFF 0x12
#define VBE_MODE_INFO_HEIGHT_OFF 0x14
#define DEFAULT_WIDTH 1440
#define DEFAULT_HEIGHT 900

setup_vesa:
    push bp
    mov bp, sp
    sub sp, 4

    lea si, [vbe_setup_str]
    call boot_log

    mov WORD PTR [bp - 2], 0xFFFF                             # Difference between the preferred mode and the current best

    lea si, [vbe3_sig]                                        # Request the VBE VbeInfoBlock, specifies the "VBE2" header
    lea di, [VESA_BUF]
    mov bx, 4
    call memcpy
    mov ax, 0x4F00
    call vbe_int

    mov si, es:[VESA_BUF + VBE_INFO_MODE_PTR_OFF]             # Loads the address of the video mode list.
    mov di, VESA_BUF + 0x200                                  # Loads the address of the ModeInfoBlock buffer.
    .mode_loop:
    lodsw es:[si]
    cmp ax, 0xFFFF                                            # Last mode number in the table is 0xFFFF
    jz .set_mode

    mov cx, 0x4F01                                            # Load the mode info block structure
    xchg ax, cx
    call vbe_int
    mov ax, es:[VESA_BUF + 0x200 + VBE_MODE_INFO_WIDTH_OFF]   # Load the width of this mode.
    sub ax, DEFAULT_WIDTH
    mov bx , ax                                               # We take the absolute value of the difference between the
    neg ax                                                    # width of this mode and the preferred width.
    cmovl ax, bx
    mov dx, es:[VESA_BUF + 0x200 + VBE_MODE_INFO_HEIGHT_OFF]  # Same thing with the mode height.
    sub dx, DEFAULT_HEIGHT
    mov bx, dx
    neg dx
    cmovl dx, bx
    add ax, bx
    cmp ax, [bp -2]                                           # Compare the sum of the differences when the current best
    jg .mode_loop
    mov [bp - 4], cx                                          # Change the best mode if the difference is lower.
    jmp .mode_loop

    .set_mode:
    mov bx, [bp - 4]
    or bx, 0x4000                                             # Enable linear framebuffer for the selected mode.
    mov ax, 0x4F02
    call vbe_int

    mov di, VESA_BUF + 0x200                                  # Keeps the ModeInfoBlock structure in a known location
    mov ax, 0x4F01                                            # for further usage later on.
    mov cx, bx
    call vbe_int

    mov sp, bp
    pop bp
    ret

vbe_int:
    push bp
    int 0x10
    pop bp
    cmp ax, 0x004F
    jne .vbe_cmd_error
    ret

.vbe_cmd_error:
lea si, [vbe_cmd_error_str]
call _print_str
lp: hlt
jmp lp

vbe_setup_str:     .asciz "initializing display using VBE extensions"
vbe_cmd_error_str: .asciz "failed to initialize display\r\n"

vbe3_sig: .ascii "VBE2"
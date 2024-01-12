#define GDT_SIZE 0x18
#define GDT_BASE_ADDR 0x00005DA0
#define E820_MEM_BUFFER 0x00004804
#define E820_MAGIC 0x534D4150

# Copy a buffer into another
# DS:SI -> source buffer
# ES:DI -> destination buffer
# BX: number of bytes to copy
memcpy:
    xor cx, cx
    .mcpy_loop:
        lodsb
        mov es:[di], al
        inc di
        inc cx
        cmp cx, bx
        jnz .mcpy_loop
    sub di, bx
    ret

e820_map:
xor bx, bx
mov si, bx
lea di, [E820_MEM_BUFFER]
entry_loop: mov eax, 0xE820
mov ecx, 0x18
mov edx, E820_MAGIC
mov LONG PTR es:[di + 20], 1
int 0x15
jc map_exit
test ebx, ebx
jz map_exit
cmp eax, E820_MAGIC
jne map_exit

cmp cl, 20
jle acpi_ext
mov ecx, es:[di + 8]
or ecx,  es:[di + 12]
jz entry_loop

acpi_ext: test BYTE PTR es:[di + 20], 1
jz entry_loop

add di, 24
inc si
jmp entry_loop
map_exit: mov es:[E820_MEM_BUFFER - 0x4], si
ret



setup_gdt:
    lea si, [gdt_load_str]
    call boot_log
    lea si, [gdt]
    xor cx, cx
    mov es, cx
    lea di, [GDT_BASE_ADDR]
    mov bx, GDT_SIZE
    call memcpy

    ret

pm_jump:
    lea si, [pm_enter]
    call boot_log
    cli
    lgdt es:[GDT_BASE_ADDR]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    mov ax, 0x8
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    ljmp 0x10, offset pm_jump_addr

.globl pm_jump_addr
pm_jump_addr: .word 0

gdt:
    gdtr_llo_hi:   .word 0x0017
    gdtr_llo_lo:   .word GDT_BASE_ADDR
    gdtr_lhi:      .long 0x00000000
    gdt_cs_llo:    .long 0x0000ffff
    gdt_cs_lhi:    .long 0x00cf9300
    gdt_ds_llo:    .long 0x0000ffff
    gdt_ds_lhi:    .long 0x00cf9a00

gdt_load_str:
    .asciz "loading protected-mode segment descriptors"

pm_enter:
    .asciz "entering protected mode"
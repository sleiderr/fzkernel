.global _entry32

.section .start
_entry32:
.code16
cli
mov eax, cr0
or eax, 1
mov cr0, eax
mov ax, 0x8
mov ds, ax
mov es, ax
mov fs, ax
mov gs, ax
mov ss, ax
mov esp, 0x7000000
ljmp 0x10, offset _start

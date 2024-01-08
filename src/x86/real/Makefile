.PHONY: all clean

CC = clang
CFLAGS = -g -Wall
ASMFLAGS = -target i386-unknown-none -masm=intel -march=i386 -c
LDFLAGS = -m elf_i386
ASM_SOURCES := boot.S real.S
ASM_OBJ := boot.o real.o
ASM_OBJ_OUT := build/boot.o build/real.o
BUILD_DIR := ../../../build
OUTPUT_DIR := ../../../build/asm

all: $(ASM_OBJ) boot image

%.o:%.S
	$(CC) $(CFLAGS) $(ASMFLAGS) $< -o $(OUTPUT_DIR)/$@
	x86_64-unknown-linux-gnu-ld $(LDFLAGS) -v -T $(patsubst %.o,%.ld,$@) -o $(OUTPUT_DIR)/$(patsubst %.o,%.out,$@) $(OUTPUT_DIR)/$@
	x86_64-unknown-linux-gnu-objcopy -I elf32-i386 -O binary $(OUTPUT_DIR)/$(patsubst %.o,%.out,$@) $(OUTPUT_DIR)/$(patsubst %.o,%.bin,$@)

boot: $(ASM_OBJ)
	echo "creating disk image"

image:
	dd if=$(OUTPUT_DIR)/real.bin of=$(OUTPUT_DIR)/boot.bin seek=1 count=512
	mv $(OUTPUT_DIR)/boot.bin $(BUILD_DIR)/boot.bin

clean:
	rm -rf $(OUTPUT_DIR)/*
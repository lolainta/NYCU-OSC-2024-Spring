TARGET = aarch64-unknown-none-softfloat

BUILD_DIR = build
RPI3_DIR = rpi3

KERNEL_ELF = target/$(TARGET)/release/kernel
KERNEL_IMG = $(BUILD_DIR)/kernel8.img

BOOTLOADER_ELF = target/$(TARGET)/release/bootloader
BOOTLOADER_IMG = $(BUILD_DIR)/bootloader.img

PROG_IMG = prog/prog.img

PROGRAM_ELF = target/$(TARGET)/release/program
PROGRAM_IMG = $(BUILD_DIR)/program.img

INITRAMFS_CPIO = $(BUILD_DIR)/initramfs.cpio

DTB = $(RPI3_DIR)/bcm2710-rpi-3-b-plus.dtb

CARGO = cargo
CARGO_FLAGS = --release --target=$(TARGET)

# OBJDUMP = aarch64-linux-gnu-objdump
# OBJCOPY = aarch64-linux-gnu-objcopy
OBJDUMP = rust-objdump
OBJCOPY = rust-objcopy

QEMU = qemu-system-aarch64

export dir_guard=@mkdir -p $(@D)

OUTPUT_ELFS := $(KERNEL_ELF) $(BOOTLOADER_ELF) $(PROGRAM_ELF)
SENTINEL_FILE := .done

.PHONY: all clean run debug size FORCE

all: $(KERNEL_IMG) $(BOOTLOADER_IMG) $(INITRAMFS_CPIO) $(PROGRAM_IMG) size

clean:
	$(MAKE) -C prog clean
	$(CARGO) clean
	rm -f $(SENTINEL_FILE)
	rm -rf $(BUILD_DIR)

FORCE:

$(OUTPUT_ELFS): $(SENTINEL_FILE)

$(SENTINEL_FILE): FORCE
	$(CARGO) build $(CARGO_FLAGS)
	@touch $(SENTINEL_FILE)

$(KERNEL_IMG): $(KERNEL_ELF) FORCE
	$(dir_guard)
	$(OBJCOPY) -O binary $< $@

$(BOOTLOADER_IMG): $(BOOTLOADER_ELF) FORCE
	$(dir_guard)
	$(OBJCOPY) -O binary $< $@

$(PROGRAM_IMG): $(PROGRAM_ELF) FORCE
	$(dir_guard)
	$(OBJCOPY) -O binary $< $@

$(PROG_IMG): FORCE
	$(MAKE) -C prog

$(INITRAMFS_CPIO): $(wildcard initramfs/*) $(PROG_IMG) $(PROGRAM_IMG)
	$(dir_guard)
	cp $(PROG_IMG) initramfs/
	cp $(PROGRAM_IMG) initramfs/
	cd initramfs && find . | cpio -o -H newc > ../$@

run: all
	$(QEMU) -M raspi3b \
		-serial null -serial pty \
		-kernel $(BOOTLOADER_IMG) \
		-initrd $(INITRAMFS_CPIO) \
		-dtb $(DTB) --daemonize

debug: all size
	$(OBJDUMP) -D $(KERNEL_ELF) > $(BUILD_DIR)/kernel.S
	$(OBJDUMP) -D $(BOOTLOADER_ELF) > $(BUILD_DIR)/bootloader.S
	$(OBJDUMP) -D $(PROGRAM_ELF) > $(BUILD_DIR)/program.S
	# $(QEMU) -M raspi3b \
	# 	-serial null -serial pty \
	# 	-kernel $(BOOTLOADER_IMG) \
	# 	-initrd $(INITRAMFS_CPIO) \
	# 	-dtb $(DTB) -S -s

size: $(KERNEL_IMG) $(BOOTLOADER_IMG) $(PROG_IMG)
	@printf "Kernel: %d (0x%x) bytes\n" `stat -c %s $(KERNEL_IMG)` `stat -c %s $(KERNEL_IMG)`
	@printf "Bootloader: %d (0x%x) bytes\n" `stat -c %s $(BOOTLOADER_IMG)` `stat -c %s $(BOOTLOADER_IMG)`
	@printf "Initramfs: %d (0x%x) bytes\n" `stat -c %s $(INITRAMFS_CPIO)` `stat -c %s $(INITRAMFS_CPIO)`

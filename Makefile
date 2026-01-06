TRACE_EXE  	:= trace_exe
EXTMKFS	:= lwext4-mkfs
TARGET      := riscv64gc-unknown-none-elf
OUTPUT := target/$(TARGET)/release
KERNEL_LIB := $(OUTPUT)/libkernel.a
KERNEL_FILE := $(OUTPUT)/kernel
DEBUG_FILE  ?= $(KERNEL_FILE)
KERNEL_ENTRY_PA := 0x80200000
OBJDUMP     := rust-objdump --arch-name=riscv64
OBJCOPY     := rust-objcopy --binary-architecture=riscv64
BOOTLOADER  := ./boot/rustsbi-qemu.bin
BOOTLOADER  := default
KERNEL_BIN  := $(KERNEL_FILE).bin
IMG := tools/sdcard.img
FSMOUNT := ./diskfs
TFTPBOOT := /home/godones/projects/tftpboot/
SMP ?= 1
GUI ?=n
NET ?=y
#IMG1 := tools/fs1.img
RUSTFLAGS := $(RUSTFLAGS)
RUSTFLAGS +=  -Cforce-unwind-tables -Cpanic=unwind 
LINKER_SCRIPT := tools/linker-qemu.ld

VF2 ?=n
UNMATCHED ?=n
FEATURES :=
QEMU_ARGS :=
MEMORY_SIZE := 1024M
SLAB ?=n
TALLOC ?=y
BUDDY ?=n
FS ?=fat
INITRD ?=y
QEMU := qemu-system-riscv64
comma:= ,
empty:=
space:= $(empty) $(empty)
SD ?= n

ifeq ($(GUI),y)
QEMU_ARGS += -device virtio-gpu-device \
			 -device virtio-tablet-device \
			 -device virtio-keyboard-device
else
QEMU_ARGS += -nographic
endif


ifeq ($(VF2),y)
FEATURES += vf2
LINKER_SCRIPT = tools/linker-vf2.ld
ifeq ($(SD),n)
FEATURES += ramdisk
endif
else ifeq ($(UNMATCHED),y)
FEATURES += hifive ramdisk
else
FEATURES += qemu
endif

ifeq ($(SLAB),y)
FEATURES += slab
else ifeq ($(TALLOC),y)
FEATURES += talloc
else ifeq ($(BUDDY),y)
FEATURES += buddy
endif

ifeq ($(FS),fat)
FEATURES += fat
else ifeq ($(FS),ext)
FEATURES += ext
endif


ifeq ($(NET),y)
QEMU_ARGS += -device virtio-net-device,netdev=net0 \
			 -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
endif


ifeq ($(INITRD),y)
#FEATURES += initrd
QEMU_ARGS += -initrd tools/initrd/initramfs.cpio.gz
QEMU_ARGS += -append "rdinit=/init"
endif


FEATURES := $(subst $(space),$(comma),$(FEATURES))

define boot_qemu
	$(QEMU) \
        -M virt $(1)\
        -bios $(BOOTLOADER) \
        -drive file=$(IMG),if=none,format=raw,id=x0 \
        -device virtio-blk-device,drive=x0 \
        -kernel  kernel-qemu\
        -$(QEMU_ARGS) \
        -smp $(SMP) -m $(MEMORY_SIZE) \
        -serial mon:stdio
endef

all:

install:
	@if ! command -v gen_ksym >/dev/null 2>&1; then \
		echo "Installing gen_ksym..."; \
		RUSTFLAGS= cargo install --git https://github.com/Starry-OS/ksym --features=demangle; \
	else \
		echo "gen_ksym already installed."; \
	fi

build: install compile

compile:
	RUSTFLAGS="$(RUSTFLAGS)" \
		cargo build --release -p kernel \
		--target $(TARGET) \
		--features $(FEATURES)

	@riscv64-linux-gnu-ld -T $(LINKER_SCRIPT) \
		-o $(KERNEL_FILE) \
		$(KERNEL_LIB)
	@echo "Generating kernel symbols at $@"
	@nm -n -C $(KERNEL_FILE) | grep ' [Tt] ' | grep -v '\.L' | grep -v '$$x' | RUSTFLAGS= gen_ksym > kallsyms
	@-make copy_kallsyms 2>/dev/null || echo "Warning: Could not copy kallsyms (requires sudo)"

	@#$(OBJCOPY) $(KERNEL_FILE) --strip-all -O binary $(KERNEL_BIN)
	@cp $(KERNEL_FILE) ./kernel-qemu


copy_kallsyms:
	@-sudo umount $(FSMOUNT) 2>/dev/null || true
	@-sudo rm -rf $(FSMOUNT) 2>/dev/null || true
	@-mkdir $(FSMOUNT)
	@sudo mount $(IMG) $(FSMOUNT)
	@echo "copying kallsyms"
	@sudo cp kallsyms $(FSMOUNT)/kallsyms
	@echo "copying kallsyms done"
	@make unmount
	@rm kallsyms

initramfs: user
	make -C tools/initrd

user:
	@echo "Building user apps"
	@make all -C ./user/apps
	@make all -C ./user/c_apps ARCH=riscv64
	#@make all -C ./user/musl
	@echo "Building user apps done"

sdcard:$(FS) mount testelf user initramfs
	@make unmount

run:sdcard install compile
	@echo qemu booot $(SMP)
	$(call boot_qemu)
	@#rm ./kernel-qemu

fake_run:
	$(call boot_qemu)

board:install compile
	@rust-objcopy --strip-all $(KERNEL_FILE) -O binary $(OUTPUT)/testos.bin
	@cp $(OUTPUT)/testos.bin  $(TFTPBOOT)
	@cp $(OUTPUT)/testos.bin ./alien.bin

qemu:
	@rust-objcopy --strip-all $(OUTPUT)/boot -O binary $(OUTPUT)/testos.bin
	@cp $(OUTPUT)/testos.bin  $(TFTPBOOT)
	@cp $(OUTPUT)/testos.bin ./alien.bin

vf2:board
	@mkimage -f ./tools/vf2.its ./alien-vf2.itb
	@rm ./kernel-qemu
	@cp ./alien-vf2.itb $(TFTPBOOT)


unmatched:board
	@mkimage -f ./tools/fu740.its ./alien-unmatched.itb
	@rm ./kernel-qemu
	@cp ./alien-unmatched.itb $(TFTPBOOT)

f_test:
	qemu-system-riscv64 \
		-machine virt \
		-kernel kernel-qemu \
		-m 128M \
		-nographic \
		-smp 2 \
	    -drive file=./tools/sdcard.img,if=none,format=raw,id=x0  \
	    -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
	    -device virtio-net-device,netdev=net -netdev user,id=net

testelf:
	@echo "copying test elf"
	@if [ -d "tests/testbin-second-stage" ]; then \
		sudo cp tests/testbin-second-stage/* $(FSMOUNT) -r; \
		sed "s:/code/lmbench/bin/riscv64/:/tests/:g" tests/testbin-second-stage/hello | sudo tee $(FSMOUNT)/hello; \
	fi
	@echo "copying test elf done"

dtb:
	$(call boot_qemu, -machine dumpdtb=riscv.dtb)
	@dtc -I dtb -O dts -o riscv.dts riscv.dtb
	@rm riscv.dtb

jh7110:
	@dtc -I dtb -o dts -o jh7110.dts ./tools/jh7110-visionfive-v2.dtb

fat:
# check the file if exist 
	if [ ! -f $(IMG) ]; then \
		echo "Creating $(IMG)"; \
		dd if=/dev/zero of=$(IMG) bs=1M count=72; \
		mkfs.fat -F 32 $(IMG); \
	else \
		echo "$(IMG) already exists."; \
	fi

ext:
	if [ ! -f $(IMG) ]; then \
		echo "Creating $(IMG)"; \
		dd if=/dev/zero of=$(IMG) bs=1M count=128; \
		mkfs.ext4 $(IMG); \
	else \
		echo "$(IMG) already exists."; \
	fi

mount:
	@echo "Mounting $(IMG) to $(FSMOUNT)"
	@-sudo umount $(FSMOUNT)
	@-sudo rm -rf $(FSMOUNT)
	@-mkdir $(FSMOUNT)
	@sudo mount $(IMG) $(FSMOUNT)
	@sudo rm -rf $(FSMOUNT)/*
	@sudo cp tools/f1.txt $(FSMOUNT)
	@sudo mkdir $(FSMOUNT)/folder
	@sudo cp tools/f1.txt $(FSMOUNT)/folder
	@sudo mkdir -p $(FSMOUNT)/tests
	@echo "Copying DBFS test binaries to filesystem..."
	@sudo cp target/riscv64gc-unknown-none-elf/release/test_init $(FSMOUNT)/init
	@-sudo cp target/riscv64gc-unknown-none-elf/release/dbfs_test $(FSMOUNT)/dbfs_test 2>/dev/null || true
	@-sudo cp target/riscv64gc-unknown-none-elf/release/final_test $(FSMOUNT)/final_test 2>/dev/null || true
	@echo "DBFS test binaries copied successfully"

unmount:
	@echo "Unmounting $(FSMOUNT)"
	@sudo umount $(FSMOUNT)
	@-rm -rf $(FSMOUNT)

img-hex:
	@hexdump $(IMG) > test.hex
	@cat test.hex

gdb-server: sdcard install compile
	qemu-system-riscv64 \
            -M virt\
            -bios $(BOOTLOADER) \
            -device loader,file=kernel-qemu,addr=$(KERNEL_ENTRY_PA) \
            -drive file=$(IMG),if=none,format=raw,id=x0 \
            -device virtio-blk-device,drive=x0 \
			-$(QEMU_ARGS) \
            -kernel  kernel-qemu\
            -smp $(SMP) -m 1024M \
            -s -S

gdb-client:
	riscv64-unknown-elf-gdb -ex 'file kernel-qemu' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'

kernel_asm:
	@riscv64-unknown-elf-objdump -d $(KERNEL_FILE) > kernel.asm
	@vim kernel.asm
	@rm kernel.asm

docs:
	cargo doc --open -p  kernel --target riscv64gc-unknown-none-elf --features $(FEATURES)

clean:
	@cargo clean
	@-rm kernel-qemu
	@-rm alien-*
	@-sudo umount $(FSMOUNT)
	@-rm -rf $(FSMOUNT)
	@make clean -C tools/initrd


check:
	cargo check -p kernel --target riscv64gc-unknown-none-elf --features $(FEATURES)

fix:
	cargo fix --allow-dirty --allow-staged -p kernel --target riscv64gc-unknown-none-elf --features $(FEATURES)

dbfs:
	@echo "========================================="
	@echo "  DBFS 测试 - 一键构建和运行"
	@echo "========================================="
	@echo ""
	@echo "📦 (1/4) 构建 test_init..."
	cargo build --release --target riscv64gc-unknown-none-elf -p test_init
	@echo "✅ test_init 完成"
	@echo ""
	@echo "📦 (2/4) 构建 dbfs_test..."
	cargo build --release --target riscv64gc-unknown-none-elf -p dbfs_test
	@echo "✅ dbfs_test 完成"
	@echo ""
	@echo "📦 (3/4) 生成 initramfs..."
	make initramfs
	@echo "✅ initramfs 完成"
	@echo ""
	@echo "📦 (4/4) 构建内核..."
	make build
	@echo "✅ 内核构建完成"
	@echo ""
	@echo "🚀 启动 DBFS 测试..."
	@echo "========================================="
	@echo ""
	@echo "系统将："
	@echo "  1. 自动运行 DBFS 测试"
	@echo "  2. 显示测试结果"
	@echo "  3. 进入交互式 shell"
	@echo "  4. 输入 exit 退出并关机"
	@echo ""
	@echo "========================================="
	@echo ""
	make run

elle: install compile
	@echo "========================================="
	@echo "  Elle + Jepsen 分布式测试"
	@echo "========================================="
	@echo ""
	@echo "📦 构建 Elle 测试..."
	cargo build --release --target riscv64gc-unknown-none-elf -p test_init
	cargo build --release --target riscv64gc-unknown-none-elf -p final_test
	@echo "✅ Elle 测试构建完成"
	@echo ""
	@echo "📦 生成 initramfs..."
	make initramfs
	@echo "✅ initramfs 完成"
	@echo ""
	@echo "📦 构建内核..."
	make build
	@echo "✅ 内核构建完成"
	@echo ""
	@echo "🚀 启动 Elle 测试..."
	@echo "========================================="
	@echo ""
	@echo "Elle 测试说明："
	@echo "  - 在内核中执行: cd / && ./final_test"
	@echo "  - 或者通过 TCP 连接运行 Host 端的 Elle 测试"
	@echo ""
	@echo "========================================="
	@echo ""
	make run

help:
	@echo "Usage: make [target]"
	@echo "  run [SMP=?] [GUI=?] [FS=?] [LOG=?]: build kernel and run qemu"
	@echo "  	 SMP: number of cores, default 1, max 8"
	@echo "  	 GUI: enable gui, default n"
	@echo "  	 FS: file system, default fat, options: fat, ext"
	@echo "  	 LOG: enable log, default n, options: TRACE, DEBUG, INFO, WARN, ERROR"
	@echo "  build [SMP=?] [LOG=?]: build kernel"
	@echo "  sdcard [GUI=?] [FS=?]: build sdcard"
	@echo "  	 GUI: enable gui, it's available only when running qemu"
	@echo "  	 FS: file system, for vf2 or unmatched, only fat is available"
	@echo "  fake_run [SMP=?] [GUI=?]: run kernel without building, the SMP should same as build"
	@echo "  dbfs: build and run DBFS correctness tests"
	@echo "  elle: build and run Elle + Jepsen distributed tests"
	@echo "  vf2 [SMP=?] [LOG=?] [VF2=y]: build starfive2 board image"
	@echo "      SMP: number of cores, must >= 2"
	@echo "      VF2: must be y"
	@echo "  unmatched [SMP=?] [LOG=?] [UNMATCHED=y]: build unmatched board image"
	@echo "      SMP: number of cores, must >= 2"
	@echo "      UNMATCHED: must be y"
	@echo "  dtb: generate dtb"
	@echo "  gdb-server: run gdb server"
	@echo "  gdb-client: run gdb client"
	@echo "  kernel_asm: disassemble kernel"
	@echo "  docs: generate docs"
	@echo "  clean: clean"
	@echo "  check: check"
	@echo "  fix: auto-fix warnings"
	@echo "  help: help"

.PHONY: all install build run clean fake_run sdcard vf2 unmatched gdb-client gdb-server kernel_asm docs user initramfs dbfs elle


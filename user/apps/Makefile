APPS := $(shell ls -d */)
compile := cargo build --release --target riscv64gc-unknown-none-elf
mode := release
target := ../../target/riscv64gc-unknown-none-elf/$(mode)/
APPS_NAME := $(shell ls -d */ | cut -d '/' -f 1)
GUI ?=n
FSMOUNT := ../../diskfs
LD_SCRIPT := user/apps/linker.ld

export RUSTFLAGS := ${RUSTFLAGS} -C link-arg=-T$(LD_SCRIPT)

all:build
	@echo "Moving apps to $(FSMOUNT)"
	@mkdir -p $(FSMOUNT)
	@$(foreach dir, $(BUILD_CRATES), (sudo cp $(target)$(dir) $(FSMOUNT)/$(dir) 2>/dev/null || true);)
	@echo "Apps moved successfully"


build:
	@echo "Building apps"
	@$(foreach dir, $(BUILD_CRATES), (cd $(dir) && $(compile) && cd ..);)


BUILD_CRATES := \
	test_init \
	minimal_fs_test \
	run_test \
	tests \
	sleep \
	socket_test \
	final_test \
	dbfs_test \
	simple_test \
#	shell \
	print \

ifeq ($(GUI),y)
	BUILD_CRATES += sysinfo \
					todo \
					slint \
					memory-game \
					printdemo
endif
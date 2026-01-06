VERSION = 1.33.1
BB = busybox
COMPILER = riscv64-linux-musl-
ARCH = riscv64
initrd = initramfs


all:
	@make download
	@if [ -f initramfs.cpio.gz ]; then \
		echo "initramfs.cpio.gz exists"; \
	else \
	  	make config; \
		make build; \
		make initramfs; \
	fi


download:
	@if [ -d $(BB) ]; then \
    		echo "Busybox exists"; \
    	else \
    		echo "Busybox does not exist"; \
    		wget http://busybox.net/downloads/busybox-$(VERSION).tar.bz2; \
    		tar -xvf busybox-$(VERSION).tar.bz2 && mv busybox-$(VERSION) $(BB); \
	fi

config:
	sudo apt install libncurses5-dev libncursesw5-dev
	CROSS_COMPILE=$(COMPILER) ARCH=$(ARCH) make defconfig -C $(BB)
	CROSS_COMPILE=$(COMPILER) ARCH=$(ARCH) make menuconfig -C $(BB)

build:
	CROSS_COMPILE=$(COMPILER) ARCH=$(ARCH) make -j $(nproc) -C $(BB)
	CROSS_COMPILE=$(COMPILER) ARCH=$(ARCH) make install -C $(BB)

initramfs:
	@if [ -d $(initrd) ]; then \
		echo "initramfs exists"; \
		rm -rf $(initrd)/*; \
	else \
		echo "initramfs does not exist"; \
		mkdir $(initrd); \
	fi
	cp -r $(BB)/_install/* $(initrd)
	mkdir -p $(initrd)/dev
	mkdir -p $(initrd)/proc
	mkdir -p $(initrd)/sys
	mkdir -p $(initrd)/tmp
	@echo "Adding DBFS test_init as init program..."
	@if [ -f ../../target/riscv64gc-unknown-none-elf/release/test_init ]; then \
		echo "Copying test_init as /init"; \
		cp ../../target/riscv64gc-unknown-none-elf/release/test_init $(initrd)/init; \
		chmod +x $(initrd)/init; \
		echo "test_init successfully installed as init"; \
	else \
		echo "test_init not found, trying dbfs_test..."; \
		if [ -f ../../target/riscv64gc-unknown-none-elf/release/dbfs_test ]; then \
			echo "Copying dbfs_test as init (fallback)"; \
			cp ../../target/riscv64gc-unknown-none-elf/release/dbfs_test $(initrd)/init; \
			chmod +x $(initrd)/init; \
		else \
			echo "No test binary found, creating default init"; \
			echo "#!/bin/sh\nmount -t proc none /proc\nmount -t sysfs none /sys\nmount -t devtmpfs none /dev\nexec /bin/sh" > $(initrd)/init; \
			chmod +x $(initrd)/init; \
		fi \
	fi
	@echo "Copying dbfs_test to initramfs..."
	@if [ -f ../../target/riscv64gc-unknown-none-elf/release/dbfs_test ]; then \
		echo "Copying dbfs_test to root"; \
		cp ../../target/riscv64gc-unknown-none-elf/release/dbfs_test $(initrd)/dbfs_test; \
		chmod +x $(initrd)/dbfs_test; \
		echo "dbfs_test successfully added to initramfs"; \
	else \
		echo "Warning: dbfs_test not found"; \
	fi
	@echo "Copying final_test (Elle) to initramfs..."
	@if [ -f ../../target/riscv64gc-unknown-none-elf/release/final_test ]; then \
		echo "Copying final_test to root"; \
		cp ../../target/riscv64gc-unknown-none-elf/release/final_test $(initrd)/final_test; \
		chmod +x $(initrd)/final_test; \
		echo "final_test successfully added to initramfs"; \
	else \
		echo "Warning: final_test not found"; \
	fi
	@echo "Copying elle_dbfs_client to initramfs..."
	@if [ -f ../../elle_dbfs_client/target/release/elle_dbfs_client ]; then \
		echo "Copying elle_dbfs_client to /tests"; \
		mkdir -p $(initrd)/tests; \
		cp ../../elle_dbfs_client/target/release/elle_dbfs_client $(initrd)/tests/elle_dbfs_client; \
		chmod +x $(initrd)/tests/elle_dbfs_client; \
		echo "elle_dbfs_client successfully added to /tests"; \
	else \
		echo "Warning: elle_dbfs_client not found"; \
	fi
	@echo "Copying test binaries to initramfs..."
	@if [ -d ../../tests/testbin-second-stage ]; then \
		echo "Copying test binaries to /tests"; \
		mkdir -p $(initrd)/tests; \
		cp ../../tests/testbin-second-stage/*.sh $(initrd)/tests/ 2>/dev/null || true; \
		cp ../../tests/testbin-second-stage/hackbench $(initrd)/tests/ 2>/dev/null || true; \
		cp ../../tests/testbin-second-stage/arithoh $(initrd)/tests/ 2>/dev/null || true; \
		cp ../../tests/testbin-second-stage/dhry2 $(initrd)/tests/ 2>/dev/null || true; \
		cp ../../tests/testbin-second-stage/looper $(initrd)/tests/ 2>/dev/null || true; \
		cp ../../tests/testbin-second-stage/syscall $(initrd)/tests/ 2>/dev/null || true; \
		chmod +x $(initrd)/tests/* 2>/dev/null || true; \
		echo "Test binaries successfully added to /tests"; \
	else \
		echo "Warning: test binaries not found"; \
	fi
	cd $(initrd) && find . -print0 | cpio --null -ov --format=newc | gzip -9 > ../initramfs.cpio.gz


clean:
	make clean -C $(BB)
	rm initramfs.cpio.gz

.PHONY: clean initramfs
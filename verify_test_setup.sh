#!/bin/bash
# 验证测试套件设置

set -e

ALIEN_DIR="/home/ubuntu2204/Desktop/Alien"

echo "========================================"
echo "Alien OS 测试套件验证脚本"
echo "========================================"
echo ""

# 检查 final_test 二进制文件
echo "✓ 检查 final_test 二进制文件..."
if [ -f "$ALIEN_DIR/target/riscv64gc-unknown-none-elf/release/final_test" ]; then
    echo "  ✅ final_test 存在"
    ls -lh "$ALIEN_DIR/target/riscv64gc-unknown-none-elf/release/final_test"
else
    echo "  ❌ final_test 不存在，需要构建"
    echo "  运行: cargo build -p final_test --release --target riscv64gc-unknown-none-elf"
fi
echo ""

# 检查 elle_dbfs_client 二进制文件
echo "✓ 检查 elle_dbfs_client 二进制文件..."
if [ -f "$ALIEN_DIR/elle_dbfs_client/target/release/elle_dbfs_client" ]; then
    echo "  ✅ elle_dbfs_client 存在"
    ls -lh "$ALIEN_DIR/elle_dbfs_client/target/release/elle_dbfs_client"
else
    echo "  ❌ elle_dbfs_client 不存在，需要构建"
    echo "  运行: cd elle_dbfs_client && cargo build --release"
fi
echo ""

# 检查 initramfs
echo "✓ 检查 initramfs..."
if [ -f "$ALIEN_DIR/tools/initrd/initramfs.cpio.gz" ]; then
    echo "  ✅ initramfs.cpio.gz 存在"
    ls -lh "$ALIEN_DIR/tools/initrd/initramfs.cpio.gz"

    # 检查是否包含 elle_dbfs_client
    echo ""
    echo "  检查 initramfs 内容..."
    if mkdir -p /tmp/initramfs_check && cd /tmp/initramfs_check; then
        gunzip -c "$ALIEN_DIR/tools/initrd/initramfs.cpio.gz" | cpio -id 2>/dev/null || true
        if [ -f "./tests/elle_dbfs_client" ]; then
            echo "  ✅ elle_dbfs_client 已包含在 initramfs 中"
        else
            echo "  ⚠️  elle_dbfs_client 未在 initramfs 中找到"
            echo "  需要重新构建: cd tools/initrd && make initramfs"
        fi
        cd - > /dev/null
        rm -rf /tmp/initramfs_check
    fi
else
    echo "  ❌ initramfs.cpio.gz 不存在"
    echo "  运行: cd tools/initrd && make"
fi
echo ""

# 检查测试脚本
echo "✓ 检查测试脚本..."
if [ -f "$ALIEN_DIR/tests/run_elle_test.sh" ]; then
    echo "  ✅ run_elle_test.sh 存在"
    if [ -x "$ALIEN_DIR/tests/run_elle_test.sh" ]; then
        echo "  ✅ run_elle_test.sh 可执行"
    else
        echo "  ⚠️  run_elle_test.sh 不可执行"
        echo "  运行: chmod +x tests/run_elle_test.sh"
    fi
else
    echo "  ❌ run_elle_test.sh 不存在"
fi
echo ""

# 检查文档
echo "✓ 检查文档..."
for doc in "FINAL_TEST_README.md" "tests/ELLE_QUICK_START.md" "TEST_SUITE_UPDATE.md"; do
    if [ -f "$ALIEN_DIR/$doc" ]; then
        echo "  ✅ $doc 存在"
    else
        echo "  ❌ $doc 不存在"
    fi
done
echo ""

# 检查 kernel
echo "✓ 检查 kernel 二进制文件..."
if [ -f "$ALIEN_DIR/target/riscv64gc-unknown-none-elf/release/kernel" ]; then
    echo "  ✅ kernel 存在"
    ls -lh "$ALIEN_DIR/target/riscv64gc-unknown-none-elf/release/kernel"
else
    echo "  ❌ kernel 不存在，需要构建"
    echo "  运行: cargo build -p kernel --release --target riscv64gc-unknown-none-elf"
fi
echo ""

echo "========================================"
echo "验证完成！"
echo "========================================"
echo ""
echo "如果所有检查都通过（显示 ✅），你可以运行测试："
echo ""
echo "  快速测试："
echo "    cd $ALIEN_DIR"
echo "    qemu-system-riscv64 -machine virt -cpu rv64 -m 2048M -smp 2 \\"
echo "      -nographic -bios default \\"
echo "      -kernel target/riscv64gc-unknown-none-elf/release/kernel \\"
echo "      -drive file=tools/sdcard.img,if=none,format=raw,id=x0 \\"
echo "      -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \\"
echo "      -netdev user,id=net0,hostfwd=tcp::2222-:22 \\"
echo "      -device virtio-net-device,netdev=net0,bus=virtio-mmio-bus.1"
echo ""
echo "  在 QEMU 中运行："
echo "    ./final_test"
echo ""
echo "  完整 Elle 测试："
echo "    cd $ALIEN_DIR/tests"
echo "    ./run_elle_test.sh"
echo ""

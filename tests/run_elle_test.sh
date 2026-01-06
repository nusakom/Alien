#!/bin/bash
# Elle + Jepsen DBFS 测试自动化脚本

set -e

ALIEN_DIR="/home/ubuntu2204/Desktop/Alien"
ELLE_CLIENT_DIR="$ALIEN_DIR/elle_dbfs_client"
QEMU_SERIAL_SOCKET="/tmp/dbfs_elle.sock"

echo "========================================"
echo "Elle + Jepsen DBFS Test Runner"
echo "========================================"

# 步骤 1: 编译内核
echo ""
echo "Step 1: Building Alien kernel..."
cd "$ALIEN_DIR"
cargo build -p kernel --release --target riscv64gc-unknown-none-elf

# 步骤 2: 编译 Elle 客户端
echo ""
echo "Step 2: Building Elle client..."
cd "$ELLE_CLIENT_DIR"
cargo build --release

# 步骤 3: 清理旧的 socket
echo ""
echo "Step 3: Cleaning up..."
rm -f "$QEMU_SERIAL_SOCKET"

# 步骤 4: 启动 QEMU (后台)
echo ""
echo "Step 4: Starting QEMU with Alien kernel..."
cd "$ALIEN_DIR"

qemu-system-riscv64 \
  -machine virt \
  -cpu rv64 \
  -m 2048M \
  -smp 2 \
  -nographic \
  -bios default \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel \
  -drive file=tools/sdcard.img,if=none,format=raw,id=x0 \
  -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
  -device virtio-serial-device \
  -chardev socket,path=$QEMU_SERIAL_SOCKET,server=on,wait=off,id=dbfs_elle \
  -device virtio-serial-pci,id=virtio-serial0,chardev=dbfs_elle \
  -netdev user,id=net0,hostfwd=tcp::2222-:22 \
  -device virtio-net-device,netdev=net0,bus=virtio-mmio-bus.1 \
  &

QEMU_PID=$!
echo "QEMU started (PID: $QEMU_PID)"

# 等待 QEMU 启动
sleep 3

# 步骤 5: 运行 Elle 测试
echo ""
echo "Step 5: Running Elle test (50000 ops, 200 concurrent clients)..."
cd "$ELLE_CLIENT_DIR"

# 保存当前时间
START_TIME=$(date +%s)

# 运行测试
if ./target/release/elle_dbfs_client; then
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))

    echo ""
    echo "========================================"
    echo "Elle Test Completed Successfully!"
    echo "Duration: ${DURATION}s"
    echo "========================================"

    # 步骤 6: 分析结果
    echo ""
    echo "Step 6: Analyzing results..."

    if [ -f "history.json" ]; then
        NUM_OPS=$(jq '. | length' history.json)
        echo "Total operations recorded: $NUM_OPS"

        # TODO: 运行 elle analyze (需要安装 elle-cli)
        # elle analyze history.json --model list-append
    else
        echo "Warning: history.json not found"
    fi
else
    echo ""
    echo "========================================"
    echo "Elle Test Failed!"
    echo "========================================"
fi

# 步骤 7: 清理
echo ""
echo "Step 7: Cleaning up..."
kill $QEMU_PID 2>/dev/null || true
wait $QEMU_PID 2>/dev/null || true

echo ""
echo "Done!"
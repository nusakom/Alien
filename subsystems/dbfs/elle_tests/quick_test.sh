#!/bin/bash
# Elle 快速测试脚本

echo "========================================"
echo "🔬 Elle 快速测试"
echo "========================================"
echo ""
echo "检查 Mock 服务器状态..."

if lsof -i :12345 > /dev/null 2>&1; then
    echo "✅ Mock 服务器正在运行 (端口 12345)"
else
    echo "❌ Mock 服务器未运行"
    echo ""
    echo "请先在另一个终端启动 Mock 服务器："
    echo "  cd subsystems/dbfs/elle_tests"
    echo "  python3 mock_kernel_server.py"
    exit 1
fi

echo ""
echo "检查 Elle 客户端..."

ELLE_CLIENT="/home/ubuntu2204/Desktop/elle_dbfs_client/target/release/elle_dbfs_client"

if [ ! -f "$ELLE_CLIENT" ]; then
    echo "❌ Elle 客户端不存在"
    echo "  位置: $ELLE_CLIENT"
    echo ""
    echo "请先编译 Elle 客户端："
    echo "  cd /home/ubuntu2204/Desktop/elle_dbfs_client"
    echo "  cargo build --release"
    exit 1
fi

echo "✅ Elle 客户端存在"
echo ""
echo "========================================"
echo "🚀 开始 Elle 测试"
echo "========================================"
echo ""

# 运行 Elle 客户端
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client

echo ""
echo "========================================"
echo "✅ 测试完成"
echo "========================================"

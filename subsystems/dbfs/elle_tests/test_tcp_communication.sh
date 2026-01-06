#!/bin/bash
# Test script for Elle TCP Server + Host Client communication

set -e

ALIEN_DIR="$(cd "$SCRIPT_DIR/../.." /home/ubuntu2204/Desktop/Alien/home/ubuntu2204/Desktop/Alien pwd)"
CLIENT_DIR="$(cd "$SCRIPT_DIR/../../.." /home/ubuntu2204/Desktop/elle_dbfs_client/home/ubuntu2204/Desktop/elle_dbfs_client pwd)/elle_dbfs_client"
LOG_FILE="/tmp/elle_test_$(date +%Y%m%d_%H%M%S).log"

echo "========================================"
echo "Elle TCP Server Test Script"
echo "========================================"
echo "Log file: $LOG_FILE"
echo ""

# Step 1: Check if kernel binary exists
echo "Step 1: Checking kernel binary..."
if [ -f "$ALIEN_DIR/kernel-qemu" ]; then
    echo "✅ Kernel binary found: $ALIEN_DIR/kernel-qemu"
    ls -lh "$ALIEN_DIR/kernel-qemu"
else
    echo "❌ Kernel binary not found!"
    echo "   Expected: $ALIEN_DIR/kernel-qemu"
    exit 1
fi

# Step 2: Check if Host client exists
echo ""
echo "Step 2: Checking Host client..."
if [ -f "$CLIENT_DIR/target/release/elle_dbfs_client" ]; then
    echo "✅ Host client found: $CLIENT_DIR/target/release/elle_dbfs_client"
    ls -lh "$CLIENT_DIR/target/release/elle_dbfs_client"
else
    echo "⚠️  Host client not found!"
    echo "   Run: cd $CLIENT_DIR && cargo build --release"
fi

# Step 3: Check QEMU version
echo ""
echo "Step 3: Checking QEMU..."
if command -v qemu-system-riscv64 &> /dev/null; then
    echo "✅ QEMU found:"
    qemu-system-riscv64 --version | head -1
else
    echo "❌ QEMU not found!"
    echo "   Install: sudo apt install qemu-system-misc"
    exit 1
fi

# Step 4: Check network port availability
echo ""
echo "Step 4: Checking port 12345..."
if netstat -tlnp 2>/dev/null | grep -q ":12345 "; then
    echo "⚠️  Port 12345 already in use:"
    netstat -tlnp | grep ":12345 "
    echo "   Another process may be using the Elle port"
else
    echo "✅ Port 12345 is available"
fi

# Summary
echo ""
echo "========================================"
echo "Test Prerequisites: Complete"
echo "========================================"
echo ""
echo "To start the Elle test:"
echo ""
echo "1. In Terminal 1 (Kernel):"
echo "   cd $ALIEN_DIR"
echo "   ./kernel-qemu"
echo ""
echo "2. In Terminal 2 (Host Client):"
echo "   cd $CLIENT_DIR"
if [ -f "$CLIENT_DIR/target/release/elle_dbfs_client" ]; then
    echo "   ./target/release/elle_dbfs_client"
else
    echo "   cargo build --release"
    echo "   ./target/release/elle_dbfs_client"
fi
echo ""
echo "Expected output:"
echo "  - Kernel: 'Elle TCP Server configured on port 12345'"
echo "  - Client: 'Connected to Alien kernel'"
echo "  - Both: Transaction logs (TX-1, TX-2, ...)"
echo ""

# Elle 测试快速指南

## Elle 是什么？

Elle 是一个测试工具，用来验证并发操作的正确性。

在我们的系统里，它主要测试：
- 多个进程同时读写文件，会不会出错
- 事务之间会不会互相干扰
- 数据是否始终保持一致

## 怎么运行？

### 方式一：在 final_test 里运行（最简单）

```bash
cd /home/ubuntu2204/Desktop/Alien

# 启动 QEMU
qemu-system-riscv64 \
  -machine virt \
  -cpu rv64 \
  -m 2048M \
  -smp 2 \
  -nographic \
  -bios default \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel \
  -drive file=tools/sdcard.img,if=none,format=raw,id=x0 \
  -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

# 在 QEMU 中
./final_test
```

Elle 测试会作为第 6 个测试自动运行。

### 方式二：运行完整的 Elle 测试

这个方式更专业，会生成详细的测试报告。

```bash
cd /home/ubuntu2204/Desktop/Alien/tests
./run_elle_test.sh
```

这个脚本会：
1. 编译内核和 Elle 客户端
2. 启动 QEMU（2G 内存，2 个 CPU）
3. 运行 Elle 测试（50000 次操作，200 个并发）
4. 生成测试结果文件
5. 自动关闭 QEMU

### 方式三：手动运行（用于调试）

如果你想看详细的运行过程：

**第一步：编译所有东西**

```bash
cd /home/ubuntu2204/Desktop/Alien

# 编译内核
cargo build -p kernel --release --target riscv64gc-unknown-none-elf

# 编译 Elle 客户端
cd elle_dbfs_client
cargo build --release
```

**第二步：启动 QEMU**

在一个终端里运行：

```bash
cd /home/ubuntu2204/Desktop/Alien

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
  -chardev socket,path=/tmp/dbfs_elle.sock,server=on,wait=off,id=dbfs_elle \
  -device virtio-serial-pci,id=virtio-serial0,chardev=dbfs_elle
```

**第三步：在 QEMU 里运行 Elle**

系统启动后，输入：

```bash
cd /tests
./elle_dbfs_client
```

## Elle 测试的配置

当前的测试配置（在 `elle_dbfs_client/src/main.rs` 中）：

- **操作次数**: 50,000 次
- **并发数**: 200 个客户端
- **测试模型**: List-append（列表追加）
- **通信方式**: virtio-serial（Unix socket）

## 预期结果

### 成功的标志

测试成功后，你会看到：

1. **在 final_test 中**：
```
🔬 [6/6] Running Elle Distributed Systems Test...
✅ /tests/elle_dbfs_client - PASSED
```

2. **完整测试中**：
```
========================================
Elle Test Completed Successfully!
Duration: XXs
========================================
Total operations recorded: 50000
```

3. **生成的文件**：
- `history.json` - 包含所有操作记录
- 可以用这个文件做进一步分析

## 常见问题

### Q: Elle 客户端找不到？

A: 检查 Elle 客户端是否编译：

```bash
ls -la /home/ubuntu2204/Desktop/Alien/elle_dbfs_client/target/release/elle_dbfs_client
```

如果不存在，重新编译：

```bash
cd /home/ubuntu2204/Desktop/Alien/elle_dbfs_client
cargo build --release
```

### Q: QEMU 资源不够？

A: 增加内存和 CPU：

```bash
-m 4096M -smp 4
```

### Q: virtio-serial 连接失败？

A: 清理旧的 socket 文件：

```bash
rm -f /tmp/dbfs_elle.sock
```

### Q: 怎么分析测试结果？

A: 需要安装 elle-cli（可选）：

```bash
# 安装 elle-cli
npm install -g elle

# 分析结果
cd /home/ubuntu2204/Desktop/Alien/elle_dbfs_client
elle analyze history.json --model list-append

# 生成可视化报告
elle render history.html < history.json
```

## 技术细节（了解即可）

### Elle 的测试原理

Elle 通过以下方式验证并发正确性：

1. **记录所有操作**
   - 每个进程都记录自己的读写操作
   - 生成一个操作历史

2. **分析操作历史**
   - 检查是否违反了隔离性
   - 检查是否出现了数据不一致

3. **生成报告**
   - 告诉你哪些操作有问题
   - 给出最小反例（最简单的出错场景）

### 我们测试的场景

- **List-append**: 多个进程同时往一个列表里追加元素
- **并发读写**: 有的进程读，有的进程写
- **事务隔离**: 每个进程在事务里操作，互不干扰

### 通信机制

Elle 客户端通过 virtio-serial 与内核通信：

```
Elle Client (用户空间)
    ↓ virtio-serial
Unix Socket (/tmp/dbfs_elle.sock)
    ↓ virtio-serial
DBFS (内核空间)
```

## 相关文档

- [DBFS 文件系统说明](../subsystems/dbfs/README.md)
- [测试套件说明](../FINAL_TEST_README.md)
- [Alien OS 总体说明](../README.md)

## 总结

简单来说：
1. **快速测试**：在 QEMU 里跑 `./final_test`
2. **完整测试**：运行 `./run_elle_test.sh`
3. **看结果**：检查屏幕输出或 `history.json` 文件

Elle 测试能帮我们确认：**多进程并发操作文件时，数据不会出错**。

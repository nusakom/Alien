# 🔬 Elle 测试脚本集

这些脚本用于测试 Elle + Jepsen 分布式测试框架与 Alien OS 的集成。

## 📁 文件说明

### Shell 脚本

| 脚本 | 用途 | 说明 |
|------|------|------|
| `run_elle_test.sh` | **主测试脚本** | 自动化完整的 Elle 测试流程 |
| `test_tcp_communication.sh` | 通信测试 | 检查 TCP 连接的先决条件 |
| `test_single_transaction.sh` | 单事务测试 | 快速验证单个事务 |
| `test_small.sh` | 小规模测试 | 减少操作数的并发测试 |
| `test_simple_server.sh` | 简单服务器 | 基础 TCP echo 服务器 |

### Python 脚本

| 脚本 | 用途 | 说明 |
|------|------|------|
| `mock_kernel_server.py` | **模拟内核服务器** | 完整的 DBFS TCP 服务器模拟 |
| `simple_test_server.py` | 简单测试服务器 | 基础的 TCP 测试服务器 |

## 🚀 快速开始

### 前置条件

1. **编译 Alien 内核**：
```bash
cd /home/ubuntu2204/Desktop/Alien
make elle
```

2. **Elle 客户端** (如果使用)：
```bash
cd /home/ubuntu2204/Desktop/elle_dbfs_client
cargo build --release
```

### 测试方式

#### 方式 1: 使用 Mock 内核快速测试 (推荐用于开发)

**终端 1** - 启动 Mock 内核：
```bash
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py
```

**终端 2** - 运行 Elle 客户端：
```bash
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

#### 方式 2: 使用真实 Alien 内核测试

**终端 1** - 启动 Alien 内核：
```bash
cd /home/ubuntu2204/Desktop/Alien
make elle
# 系统会自动启动并进入 shell
```

**终端 2** - 运行 Elle 客户端：
```bash
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

#### 方式 3: 自动化完整测试

```bash
cd subsystems/dbfs/elle_tests
./run_elle_test.sh
```

## 📖 脚本详细说明

### 1. run_elle_test.sh

完整的 Elle 测试自动化流程。

**功能**：
- ✅ 编译 Alien 内核
- ✅ 编译 Elle 客户端
- ✅ 启动 QEMU (带 virtio-serial)
- ✅ 运行 Elle 测试
- ✅ 分析测试结果
- ✅ 自动清理

**使用**：
```bash
./run_elle_test.sh
```

**测试参数** (默认)：
- 操作数：50000
- 并发客户端：200

**输出**：
- `history.json` - 操作历史记录
- 测试统计信息

### 2. test_tcp_communication.sh

检查 TCP 通信的先决条件。

**功能**：
- ✅ 检查内核二进制文件
- ✅ 检查 Host 客户端
- ✅ 检查 QEMU 版本
- ✅ 检查端口可用性 (12345)

**使用**：
```bash
./test_tcp_communication.sh
```

### 3. test_single_transaction.sh

快速验证单个事务。

**功能**：
- ✅ 测试事务开始
- ✅ 测试文件创建
- ✅ 测试目录读取
- ✅ 测试事务提交

**使用**：
```bash
./test_single_transaction.sh
```

### 4. test_small.sh

小规模并发测试 (2 个并发任务)。

**功能**：
- ✅ 快速编译测试客户端
- ✅ 运行 2 个并发事务
- ✅ 验证并发安全性

**使用**：
```bash
./test_small.sh
```

### 5. mock_kernel_server.py

**最重要的测试工具！** 完整的 DBFS TCP 服务器模拟。

**功能**：
- ✅ 实现完整的 DBFS 协议
- ✅ 支持所有 8 种 DBFS 操作
- ✅ 模拟事务管理
- ✅ 详细的日志输出
- ✅ 错误处理

**支持的操作**：
1. `BeginTx` - 开始事务
2. `WriteFile` - 写入文件
3. `CreateFile` - 创建文件
4. `DeleteFile` - 删除文件
5. `Mkdir` - 创建目录
6. `Readdir` - 读取目录
7. `CommitTx` - 提交事务
8. `RollbackTx` - 回滚事务

**使用**：
```bash
# 默认端口 12345
python3 mock_kernel_server.py

# 自定义端口
python3 mock_kernel_server.py 9999
```

**协议格式**：
```
Request:  [Length(4)] [tx_id(8)] [op_type(1)] [path_len(4)] [path] [offset(8)] [data_len(4)] [data]
Response: [Length(4)] [tx_id(8)] [status(4)] [lsn(8)] [data_len(4)] [data]
```

**示例输出**：
```
========================================
🚀 Mock Kernel TCP Server
========================================
Port: 12345
Mode: Mock DBFS operations
Protocol: Length-prefixed binary
========================================
✅ Server listening on 0.0.0.0:12345

Ready to accept Elle test clients from Host

========================================
Connection #1 from ('127.0.0.1', 54321)
========================================
📨 New connection from ('127.0.0.1', 54321)
📦 Receiving 45 bytes
📨 TX-1: BeginTx
  TX-1: BEGIN -> LSN=1
📤 Sent 24 bytes
📦 Receiving 32 bytes
📨 TX-1: CreateFile
  TX-1: CREATE /test.txt
📤 Sent 24 bytes
📦 Receiving 24 bytes
📨 TX-1: CommitTx
  TX-1: COMMIT -> LSN=1
📤 Sent 24 bytes
✅ Connection closed
```

### 6. test_simple_server.sh

简单的 TCP echo 服务器 (使用 netcat)。

**用途**：基础网络测试

**使用**：
```bash
./test_simple_server.sh
```

## 🔧 配置

### 端口配置

默认使用端口 **12345**。如果需要修改：

在脚本中修改：
```bash
PORT=12345  # 改为你想要的端口
```

### 路径配置

脚本会自动检测 Alien 目录：

```bash
# 自动检测当前目录
ALIEN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
```

## 📊 测试结果解读

### 成功的测试

```
TX-1: begin -> LSN=1
TX-1: CREATE /test.txt
TX-1: commit -> LSN=1
✅ Transaction completed
```

### 失败的测试

```
❌ Failed to connect to 127.0.0.1:12345
```

**可能原因**：
- 内核服务器未启动
- 端口被占用
- 防火墙阻止

## 🐛 调试技巧

### 1. 检查连接

```bash
# 检查端口是否监听
netstat -tlnp | grep 12345

# 测试 TCP 连接
telnet 127.0.0.1 12345
```

### 2. 查看详细日志

```bash
# 使用 verbose 模式
python3 mock_kernel_server.py 2>&1 | tee server.log
```

### 3. 抓包分析

```bash
# 抓取 TCP 包
sudo tcpdump -i lo port 12345 -w debug.pcap

# 使用 Wireshark 分析
wireshark debug.pcap
```

## 📚 相关文档

- **[COMPLETE_TEST_GUIDE.md](../../COMPLETE_TEST_GUIDE.md)** - 完整测试系统指南
- **[FINAL_TEST_GUIDE.md](../../FINAL_TEST_GUIDE.md)** - final_test 使用指南
- **[ELLE_USAGE.md](../../ELLE_USAGE.md)** - Elle 框架详细文档
- **[subsystems/dbfs/src/elle_handler_real.rs](../../subsystems/dbfs/src/elle_handler_real.rs)** - 内核端 Elle 处理器

## 🔗 架构

```
┌─────────────────────────────────────────────┐
│         Host Linux                          │
│  ┌───────────────────────────────────────┐  │
│  │  Elle Client (elle_dbfs_client)      │  │
│  └───────────────┬───────────────────────┘  │
│                  │ TCP (port 12345)         │
└──────────────────┼──────────────────────────┘
                   │
        ┌──────────┴──────────┐
        │                     │
┌───────▼─────────┐  ┌────────▼──────────────┐
│  Real Kernel    │  │  Mock Kernel Server   │
│  (Alien OS)     │  │  (Python)             │
│  - QEMU         │  │  - Development        │
│  - virtio       │  │  - Fast testing       │
└─────────────────┘  └───────────────────────┘
```

## 🎯 典型工作流

### 开发阶段

1. 使用 `mock_kernel_server.py` 快速迭代
2. 在本地测试客户端逻辑
3. 验证协议正确性

### 集成测试

1. 启动真实 Alien 内核 (`make elle`)
2. 运行 `test_tcp_communication.sh` 检查连接
3. 运行 `test_single_transaction.sh` 验证基本功能
4. 运行 `run_elle_test.sh` 进行完整测试

### 回归测试

```bash
# 快速测试
cd subsystems/dbfs/elle_tests
./test_single_transaction.sh

# 完整测试
./run_elle_test.sh
```

---

**版本**: 2026-01-06
**状态**: ✅ 完整的 Elle 测试脚本集
**作者**: Alien OS Development Team

**开始测试 Elle + Jepsen 吧！** 🚀

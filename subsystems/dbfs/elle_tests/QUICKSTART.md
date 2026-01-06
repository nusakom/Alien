# 🚀 Elle 测试快速开始

## 一键运行所有测试

```bash
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh
```

## 快速测试选项

### 1. 使用 Mock 内核快速测试 (推荐新手)

**终端 1** - 启动 Mock 服务器：
```bash
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py
```

**终端 2** - 运行测试客户端：
```bash
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

### 2. 使用真实 Alien 内核测试

**终端 1** - 启动 Alien：
```bash
cd /home/ubuntu2204/Desktop/Alien
make elle
```

**终端 2** - 运行 Elle 客户端：
```bash
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

### 3. 交互式菜单

```bash
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh
```

你会看到：
```
========================================
🔬 Elle 测试套件
========================================

请选择测试模式:

  1) Mock 内核测试 (快速开发测试)
  2) 真实内核测试 (完整集成测试)
  3) 通信检查 (TCP 连接测试)
  4) 单事务测试 (快速验证)
  5) 小规模测试 (2 并发)
  6) 完整 Elle 测试 (50000 ops)
  7) 运行所有测试
  8) 退出

请输入选项 [1-8]:
```

## 测试脚本说明

| 脚本 | 用途 | 运行方式 |
|------|------|---------|
| `run_all_elle_tests.sh` | **主测试脚本** | `./run_all_elle_tests.sh` |
| `mock_kernel_server.py` | Mock DBFS 服务器 | `python3 mock_kernel_server.py` |
| `run_elle_test.sh` | 完整自动化测试 | `./run_elle_test.sh` |
| `test_tcp_communication.sh` | 检查 TCP 连接 | `./test_tcp_communication.sh` |
| `test_single_transaction.sh` | 单事务测试 | `./test_single_transaction.sh` |
| `test_small.sh` | 小规模测试 | `./test_small.sh` |

## 预期输出

### Mock 内核服务器

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

### Elle 客户端

```
Connecting to 127.0.0.1:12345...
✅ Connected
TX-1: begin -> LSN=1
TX-1: create /test.txt
TX-1: commit -> LSN=1
✅ Transaction completed
```

## 常见问题

### Q: 端口已被占用

```bash
# 查看占用端口的进程
netstat -tlnp | grep 12345

# 杀死进程
kill -9 <PID>

# 或者修改 mock_kernel_server.py 中的端口
python3 mock_kernel_server.py 9999
```

### Q: 连接失败

```bash
# 检查防火墙
sudo ufw status

# 临时关闭防火墙 (测试用)
sudo ufw disable
```

### Q: Python 依赖缺失

```bash
# Mock 服务器使用标准库，无需额外安装
python3 --version  # 应该 >= 3.6
```

## 下一步

1. ✅ 运行 Mock 内核测试
2. ✅ 运行真实内核测试
3. ✅ 查看完整文档：[README.md](README.md)
4. ✅ 集成到 final_test

---

**快速开始！** 🎉

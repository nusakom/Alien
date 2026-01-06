# 🧪 Alien OS Testing Guide / 测试指南

<div align="center">

  [English](#english-version) | [中文](#中文版本)

</div>

---

## English Version

### Overview

Alien OS has a comprehensive 3-tier testing architecture designed to validate system correctness, performance, and distributed system properties.

### Testing Architecture

```
┌──────────────────────────────────────────────┐
│         Alien OS Testing System             │
└──────────────────────────────────────────────┘
                    │
    ┌───────────────┼───────────────┐
    │               │               │
┌───▼────┐   ┌─────▼─────┐   ┌─────▼──────────┐
│ Core   │   │ Elle      │   │ POSIX/Performance│
│ Tests  │   │ Tests      │   │ Tests            │
├─────────┤   ├───────────┤   ├────────────────┤
│final_  │   │elle_tests │   │testbin-second- │
│test    │   │           │   │  stage         │
│- DBFS  │   │- Mock     │   │- UnixBench     │
│- Perf  │   │  Kernel   │   │- lmbench       │
│- Func  │   │- TCP      │   │- iozone        │
└─────────┘   └───────────┘   └────────────────┘
```

### Tier 1: Core Functionality Tests

**Location**: `user/apps/final_test/`

**Purpose**: Validate core system functionality and DBFS correctness.

**How to Run**:

```bash
# Start Alien OS
make f_test

# In QEMU, run:
/ # ./final_test
```

**Test Suite**:

| Test | Description | Pass Criteria |
|------|-------------|---------------|
| **DBFS Correctness** | WAL and transaction integrity | All 5 subtests pass |
| **Dhrystone Benchmark** | CPU performance | ~1500 DMIPS |
| **Arithmetic Benchmark** | Integer operations | All operations correct |
| **System Call Benchmark** | Syscall overhead | < 1000ns per call |
| **Hackbench Concurrency** | Scheduler and concurrency | Completes without deadlock |

**Expected Output**:

```
========================================
✅ DBFS Correctness Test
========================================
✅ WAL Create Test: PASSED
✅ Transaction Begin: PASSED
✅ Transaction Commit: PASSED
✅ File Write Test: PASSED
✅ File Read Test: PASSED

========================================
✅ Dhrystone Benchmark
========================================
DMIPS: 1500.5

========================================
✅ All Tests PASSED
========================================
```

### Tier 2: Elle + Jepsen Distributed Tests

**Location**: `subsystems/dbfs/elle_tests/`

**Purpose**: Validate transaction isolation, concurrency control, and distributed system properties.

#### Option A: Mock Kernel Testing (Recommended for Development)

**Best for**: Fast development iteration and protocol testing.

```bash
# Terminal 1: Start Mock Server
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py

# Terminal 2: Run Elle Client
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

**Advantages**:
- ✅ Fast startup (no QEMU)
- ✅ Easy to debug
- ✅ Perfect for protocol validation

#### Option B: Real Kernel Testing

**Best for**: Complete integration testing.

```bash
# Terminal 1: Start Real Kernel
cd /home/ubuntu2204/Desktop/Alien
make f_test

# Terminal 2: Run Elle Client
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

#### Option C: Interactive Menu

```bash
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh
```

**Menu Options**:
1. Mock kernel test (fast dev test)
2. Real kernel test (complete integration)
3. Communication check (TCP connection)
4. Single transaction test (quick validation)
5. Small scale test (2 concurrent)
6. Complete Elle test (50000 ops)
7. Run all tests
8. Exit

**What Elle Tests**:

- ✅ **Transaction Isolation** - Verifies serializable isolation
- ✅ **Concurrency Control** - Tests lock contention handling
- ✅ **Crash Recovery** - Validates WAL replay
- ✅ **Protocol Verification** - TCP communication correctness

### Tier 3: POSIX & Performance Tests

**Location**: `tests/testbin-second-stage/`

**Purpose**: Validate POSIX compliance and measure system performance.

#### UnixBench - Comprehensive Performance

```bash
make f_test
/ # cd /tests
/tests # ./unixbench_testcode.sh
```

**Tests Included**:
- File copy, pipe, context switch
- Arithmetic, function calls
- Process creation, shell scripts

#### lmbench - System Latency

```bash
/tests # ./lmbench_testcode.sh
```

**Measures**:
- Context switch overhead
- Pipe latency
- TCP connection overhead
- File system operations

#### iozone - I/O Performance

```bash
/tests # ./iozone_testcode.sh
```

**Tests**:
- Sequential read/write
- Random read/write
- Different file sizes
- Different record sizes

#### Network Performance

```bash
/tests # ./iperf_testcode.sh    # TCP throughput
/tests # ./netperf_testcode.sh  # Network latency
```

#### Database Performance

```bash
/tests # redis-server
/tests # redis-benchmark

/tests # sqlite3
```

### Interpreting Test Results

#### Success Indicators

✅ **Core Tests**:
- All 5 DBFS tests pass
- No crashes or panics
- Performance metrics within expected range

✅ **Elle Tests**:
- No "Resource temporarily unavailable" errors
- Transaction retry logs visible (if contention occurs)
- High completion rate (> 95%)

✅ **Performance Tests**:
- Stable scores across multiple runs
- No significant regressions

#### Troubleshooting

**Problem**: "Resource temporarily unavailable (os error 11)"

**Solution**: This is the lock contention issue we fixed. Check logs for:
```
⚠ DBFS: begin_tx lock contention (attempt 1/5), retrying...
✓ DBFS: Transaction X started (retry N)
```

If you see these logs, the retry mechanism is working correctly.

**Problem**: Test crashes or hangs

**Solution**:
1. Check if kernel is properly built: `make kernel`
2. Verify initramfs is generated: `make initramfs`
3. Check QEMU version: `qemu-system-riscv64 --version`

**Problem**: Elle client can't connect

**Solution**:
1. Verify server is running: `ps aux | grep mock_kernel_server`
2. Check port: `netstat -tlnp | grep 12345`
3. Test TCP: `telnet localhost 12345`

### Running All Tests

For comprehensive testing, run tests in this order:

```bash
# 1. Core functionality (5 minutes)
make f_test
/ # ./final_test

# 2. Elle distributed tests (10 minutes)
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh

# 3. Performance tests (30 minutes)
/ # cd /tests
/tests # ./unixbench_testcode.sh
/tests # ./lmbench_testcode.sh
/tests # ./iozone_testcode.sh
```

---

## 中文版本

### 概述

Alien OS 具有完整的三层测试架构，旨在验证系统正确性、性能和分布式系统属性。

### 测试架构

```
┌──────────────────────────────────────────────┐
│         Alien OS 测试系统                   │
└──────────────────────────────────────────────┘
                    │
    ┌───────────────┼───────────────┐
    │               │               │
┌───▼────┐   ┌─────▼─────┐   ┌─────▼──────────┐
│ 核心   │   │ Elle      │   │ POSIX/性能    │
│ 测试   │   │ 测试       │   │ 测试           │
├─────────┤   ├───────────┤   ├────────────────┤
│final_  │   │elle_tests │   │testbin-second- │
│test    │   │           │   │  stage         │
│- DBFS  │   │- Mock内核 │   │- UnixBench     │
│- 性能  │   │- TCP测试  │   │- lmbench       │
│- 功能  │   │- 事务测试 │   │- iozone        │
└─────────┘   └───────────┘   └────────────────┘
```

### 第一层：核心功能测试

**位置**: `user/apps/final_test/`

**目的**: 验证核心系统功能和 DBFS 正确性。

**如何运行**:

```bash
# 启动 Alien OS
make f_test

# 在 QEMU 中运行:
/ # ./final_test
```

**测试套件**:

| 测试 | 说明 | 通过标准 |
|------|------|----------|
| **DBFS 正确性** | WAL 和事务完整性 | 所有 5 个子测试通过 |
| **Dhrystone 基准** | CPU 性能 | ~1500 DMIPS |
| **算术基准** | 整数运算 | 所有操作正确 |
| **系统调用基准** | 系统调用开销 | < 1000ns/次 |
| **Hackbench 并发** | 调度器和并发 | 无死锁完成 |

**预期输出**:

```
========================================
✅ DBFS 正确性测试
========================================
✅ WAL 创建测试: 通过
✅ 事务开始: 通过
✅ 事务提交: 通过
✅ 文件写入测试: 通过
✅ 文件读取测试: 通过

========================================
✅ Dhrystone 基准测试
========================================
DMIPS: 1500.5

========================================
✅ 所有测试通过
========================================
```

### 第二层：Elle + Jepsen 分布式测试

**位置**: `subsystems/dbfs/elle_tests/`

**目的**: 验证事务隔离、并发控制和分布式系统属性。

#### 选项 A: Mock 内核测试（推荐开发）

**最适合**: 快速开发迭代和协议验证。

```bash
# 终端 1: 启动 Mock 服务器
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py

# 终端 2: 运行 Elle 客户端
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

**优势**:
- ✅ 快速启动（无需 QEMU）
- ✅ 易于调试
- ✅ 适合协议验证

#### 选项 B: 真实内核测试

**最适合**: 完整的集成测试。

```bash
# 终端 1: 启动真实内核
cd /home/ubuntu2204/Desktop/Alien
make f_test

# 终端 2: 运行 Elle 客户端
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

#### 选项 C: 交互式菜单

```bash
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh
```

**菜单选项**:
1. Mock 内核测试（快速开发）
2. 真实内核测试（完整集成）
3. 通信检查（TCP 连接）
4. 单事务测试（快速验证）
5. 小规模测试（2 并发）
6. 完整 Elle 测试（50000 操作）
7. 运行所有测试
8. 退出

**Elle 测试内容**:

- ✅ **事务隔离** - 验证可串行化隔离
- ✅ **并发控制** - 测试锁竞争处理
- ✅ **崩溃恢复** - 验证 WAL 重放
- ✅ **协议验证** - TCP 通信正确性

### 第三层：POSIX & 性能测试

**位置**: `tests/testbin-second-stage/`

**目的**: 验证 POSIX 合规性和测量系统性能。

#### UnixBench - 综合性能

```bash
make f_test
/ # cd /tests
/tests # ./unixbench_testcode.sh
```

**包含的测试**:
- 文件复制、管道、上下文切换
- 算术、函数调用
- 进程创建、Shell 脚本

#### lmbench - 系统延迟

```bash
/tests # ./lmbench_testcode.sh
```

**测量内容**:
- 上下文切换开销
- 管道延迟
- TCP 连接开销
- 文件系统操作

#### iozone - I/O 性能

```bash
/tests # ./iozone_testcode.sh
```

**测试内容**:
- 顺序读/写
- 随机读/写
- 不同文件大小
- 不同记录大小

#### 网络性能

```bash
/tests # ./iperf_testcode.sh    # TCP 吞吐量
/tests # ./netperf_testcode.sh  # 网络延迟
```

#### 数据库性能

```bash
/tests # redis-server
/tests # redis-benchmark

/tests # sqlite3
```

### 解读测试结果

#### 成功指标

✅ **核心测试**:
- 所有 5 个 DBFS 测试通过
- 无崩溃或 panic
- 性能指标在预期范围内

✅ **Elle 测试**:
- 无 "Resource temporarily unavailable" 错误
- 可见事务重试日志（如有竞争）
- 高完成率（> 95%）

✅ **性能测试**:
- 多次运行得分稳定
- 无显著回退

#### 故障排除

**问题**: "Resource temporarily unavailable (os error 11)"

**解决方案**: 这是我们修复的锁竞争问题。检查日志中是否有：
```
⚠ DBFS: begin_tx lock contention (attempt 1/5), retrying...
✓ DBFS: Transaction X started (retry N)
```

如果看到这些日志，说明重试机制正常工作。

**问题**: 测试崩溃或挂起

**解决方案**:
1. 检查内核是否正确编译: `make kernel`
2. 验证 initramfs 是否生成: `make initramfs`
3. 检查 QEMU 版本: `qemu-system-riscv64 --version`

**问题**: Elle 客户端无法连接

**解决方案**:
1. 验证服务器运行中: `ps aux | grep mock_kernel_server`
2. 检查端口: `netstat -tlnp | grep 12345`
3. 测试 TCP: `telnet localhost 12345`

### 运行所有测试

综合测试，按以下顺序运行：

```bash
# 1. 核心功能（5 分钟）
make f_test
/ # ./final_test

# 2. Elle 分布式测试（10 分钟）
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh

# 3. 性能测试（30 分钟）
/ # cd /tests
/tests # ./unixbench_testcode.sh
/tests # ./lmbench_testcode.sh
/tests # ./iozone_testcode.sh
```

---

## Quick Reference / 快速参考

### Test Commands / 测试命令

| Test / 测试 | Command / 命令 |
|-------------|---------------|
| Core / 核心 | `./final_test` |
| Elle (Mock) | `python3 mock_kernel_server.py` |
| Elle (Menu) | `./run_all_elle_tests.sh` |
| UnixBench | `./unixbench_testcode.sh` |
| lmbench | `./lmbench_testcode.sh` |
| iozone | `./iozone_testcode.sh` |

### Test Locations / 测试位置

| Test / 测试 | Location / 位置 |
|-------------|-------------------|
| final_test | `user/apps/final_test/` |
| elle_tests | `subsystems/dbfs/elle_tests/` |
| performance | `tests/testbin-second-stage/` |

---

**For more information, see**: / 更多信息请参阅：
- [README_EN.md](README_EN.md)
- [README_CN.md](README_CN.md)
- [PROJECT_HIGHLIGHTS.md](PROJECT_HIGHLIGHTS.md)

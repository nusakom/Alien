<div align="center">

  ![Alien OS](https://img.shields.io/badge/Alien-OS-blue?style=for-the-badge)
  ![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?style=for-the-badge&logo=rust)
  ![RISC-V](https://img.shields.io/badge/RISC--V-64--bit-green?style=for-the-badge)
  ![License](https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge)

  # 🚀 Alien OS

  **基于 Rust 的模块化 RISC-V 操作系统**

  [支持事务性文件系统与完整的分布式测试]

</div>

---

## 📖 目录

- [项目概述](#项目概述)
- [核心特性](#核心特性)
- [快速开始](#快速开始)
- [系统架构](#系统架构)
- [测试系统](#测试系统)
- [DBFS 文件系统](#dbfs-文件系统)
- [开发指南](#开发指南)
- [项目文档](#项目文档)
- [贡献指南](#贡献指南)
- [许可证](#许可证)

---

## 项目概述

**Alien OS** 是一个用 Rust 编写的研究级操作系统，面向 RISC-V 64 位架构。它采用模块化设计，具有独立的子系统、支持 ACID 事务的文件系统（DBFS），以及包括 Elle + Jepsen 在内的完整分布式测试验证框架。

### 项目统计

| 指标 | 数量 |
|------|------|
| 子系统 | 13 |
| 用户应用 | 20+ |
| 测试工具 | 50+ |
| 代码行数 | 50,000+ |

---

## 核心特性

### 系统特性

| 特性 | 说明 |
|------|------|
| 🎯 **模块化设计** | 13 个独立子系统 |
| 📁 **DBFS 文件系统** | WAL + ACID 事务 |
| 🧪 **Elle + Jepsen** | 分布式系统测试 |
| 💻 **用户空间** | 20+ 个用户程序 |
| 🔧 **设备驱动** | UART、VirtIO、网络 |
| 📊 **完整测试** | 性能 + 正确性 |

### 技术亮点：并发修复

<div align="center">

```rust
// 带重试机制的事务开始
pub fn begin_tx() -> TxId {
    for retry in 0..MAX_TX_RETRY {
        match CURRENT_TX.try_lock() {
            Ok(mut guard) => {
                let tx_id = TxId::new(GLOBAL_TX_ID.fetch_add(1, Ordering::SeqCst));
                *guard = Some(tx_id);
                return tx_id;
            }
            Err(_) => {
                core::hint::spin_loop(); // CPU 让出
            }
        }
    }
    // 降级到阻塞锁
    // ...
}
```

</div>

✅ **重试机制** - 优雅处理并发锁竞争

---

## 快速开始

### 前置要求

- Rust 1.70+ (nightly)
- RISC-V 工具链 (`riscv64-linux-musl-gcc`)
- QEMU 7.0+ (`qemu-system-riscv64`)
- Python 3.6+ (用于 Elle 测试)

### 安装

```bash
# 克隆仓库
git clone <repository-url>
cd Alien

# 编译内核
make kernel

# 运行系统
make f_test
```

### 快速测试

```bash
# 在 QEMU 中运行综合测试套件
/ # ./final_test

# 预期输出:
# ✅ DBFS 正确性测试: 通过
# ✅ Dhrystone 基准测试: 通过
# ✅ 算术基准测试: 通过
# ✅ 系统调用测试: 通过
# ✅ Hackbench 并发测试: 通过
```

---

## 系统架构

### 目录结构

```
Alien/
├── kernel/                    # 内核核心
│   └── src/
│
├── subsystems/                # 子系统 (13 个)
│   ├── arch/                 # RISC-V 架构
│   ├── dbfs/                 # 🌟 事务性文件系统
│   │   ├── src/
│   │   │   ├── wal.rs        # Write-Ahead Log
│   │   │   ├── transaction.rs # 事务管理器
│   │   │   └── elle_handler_real.rs # Elle 处理器
│   │   └── elle_tests/       # Elle 测试脚本
│   ├── vfs/                  # 虚拟文件系统
│   ├── mem/                  # 内存管理
│   ├── drivers/              # 设备驱动
│   └── ...
│
├── user/                     # 用户空间
│   ├── apps/                 # 应用程序 (20+)
│   │   ├── final_test/       # 综合测试套件
│   │   ├── dbfs_test/        # DBFS 正确性测试
│   │   └── shell/            # Shell
│   └── userlib/              # 用户库
│
└── tests/                    # 测试工具
    └── testbin-second-stage/ # POSIX & 性能测试
```

### 系统架构图

<div align="center">

```
┌─────────────────────────────────────────────────────────┐
│                     Alien OS                            │
├─────────────────────────────────────────────────────────┤
│  用户空间 (应用程序、Shell、测试)                        │
├─────────────────────────────────────────────────────────┤
│  系统调用接口                                             │
├─────────────────────────────────────────────────────────┤
│  内核核心 (进程、内存、调度器)                            │
├─────────────────────────────────────────────────────────┤
│  子系统 (13 个模块)                                      │
│  ┌───────────────────────────────────────────────────┐ │
│  │ DBFS │ VFS │ 驱动 │ 网络 │ 内存 │ IPC  │ │
│  └───────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│  硬件层 (RISC-V、UART、VirtIO)                          │
└─────────────────────────────────────────────────────────┘
```

</div>

---

## 测试系统

Alien OS 具有完整的三层测试架构：

### 1. 核心功能测试

**位置**: `user/apps/final_test/`

```bash
/ # ./final_test
```

| 测试 | 说明 |
|------|------|
| DBFS 正确性 | WAL 和事务完整性 |
| Dhrystone | CPU 性能基准 |
| 算术测试 | 整数运算 |
| 系统调用 | 系统调用开销 |
| Hackbench | 并发和调度 |

### 2. Elle + Jepsen 分布式测试

**位置**: `subsystems/dbfs/elle_tests/`

<div align="center">

```bash
# 交互式菜单
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh

# 或使用 Mock 内核开发
python3 mock_kernel_server.py
```

</div>

**测试能力**:
- ✅ 事务隔离
- ✅ 并发控制
- ✅ 崩溃恢复
- ✅ TCP 协议验证

### 3. POSIX & 性能测试

**位置**: `tests/testbin-second-stage/`

| 套件 | 说明 | 命令 |
|---------------|-------------------|----------------|
| UnixBench | 综合性能 | `./unixbench_testcode.sh` |
| lmbench | 系统延迟 | `./lmbench_testcode.sh` |
| iozone | I/O 性能 | `./iozone_testcode.sh` |
| iperf3 | 网络吞吐 | `./iperf_testcode.sh` |
| Redis | 数据库性能 | `redis-benchmark` |

**详细测试说明请参阅 [TESTING.md](TESTING.md)**

---

## DBFS 文件系统

### 概述

DBFS（Database Filesystem）是一个建立在 Write-Ahead Logging (WAL) 之上的事务性文件系统。它为文件操作提供 ACID 保证，使其适用于需要数据完整性的关键应用。

### 核心特性

| 特性 | 实现 |
|------|------|
| 🔒 **ACID 事务** | 开始、提交、回滚支持 |
| 📝 **Write-Ahead Log** | 持久化日志用于崩溃恢复 |
| 🔄 **并发控制** | 多版本并发控制 |
| 💾 **崩溃恢复** | WAL 重放机制 |
| 🔌 **VFS 集成** | 标准文件系统接口 |

### 使用示例

```rust
// 开始事务
let tx_id = begin_tx();

// 写文件操作（记录到 WAL）
write_file(tx_id, "/test/file.txt", data);

// 提交事务
commit_tx(tx_id)?;

// 或根据需要回滚
rollback_tx(tx_id);
```

**详细架构说明请参阅 [FILESYSTEM_ARCHITECTURE.md](FILESYSTEM_ARCHITECTURE.md)**

---

## 性能测试

### 基准测试结果

<div align="center">

| 测试 | 结果 | 状态 |
|-------------|--------------|---------------|
| Dhrystone | ~1500 DMIPS | ✅ 通过 |
| UnixBench | 分数: ~250 | ✅ 通过 |
| lmbench | 上下文切换: ~5μs | ✅ 通过 |
| iozone | 顺序写: ~80 MB/s | ✅ 通过 |

</div>

### 可扩展性

系统支持:
- ✅ 最多 200 个并发事务
- ✅ 锁竞争时自动重试
- ✅ 优雅降级

---

## 开发指南

### 添加用户程序

```bash
# 1. 在 user/apps/ 中创建新应用
mkdir user/apps/my_app

# 2. 添加 Cargo.toml 和 src/main.rs

# 3. 编译
make user
```

### 添加新测试

```bash
# 1. 将测试二进制添加到 tests/testbin-second-stage/

# 2. 如需要更新 final_test/src/main.rs

# 3. 重新编译
make all
```

### 开发工作流

```bash
# 1. 编辑代码
vim subsystems/dbfs/src/wal.rs

# 2. 编译内核
make kernel

# 3. 使用 Mock 内核测试（快速）
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py

# 4. 使用真实内核测试（慢速）
make f_test
```

---

## 项目文档

### 核心文档

| 文档 | 说明 |
|-----------------|---------------------|
| [MASTER_INDEX.md](MASTER_INDEX.md) | 主文档索引 |
| [FILE_MANIFEST.md](FILE_MANIFEST.md) | 完整文件清单 |
| [PROJECT_DOCUMENTATION.md](PROJECT_DOCUMENTATION.md) | 项目概述 |

### DBFS 文档

| 文档 | 说明 |
|-----------------|---------------------|
| [subsystems/dbfs/README.md](subsystems/dbfs/README.md) | DBFS 概述 |
| [subsystems/dbfs/ARCHITECTURE.md](subsystems/dbfs/ARCHITECTURE.md) | 架构文档 |
| [subsystems/dbfs/TRANSACTION_GUIDE.md](subsystems/dbfs/TRANSACTION_GUIDE.md) | 事务指南 |

### 我们的工作

**项目亮点和成就请参阅 [PROJECT_HIGHLIGHTS.md](PROJECT_HIGHLIGHTS.md)**

---

## 贡献指南

我们欢迎贡献！详情请参阅我们的贡献指南。

### 开发环境设置

```bash
# Fork 仓库
git clone <your-fork>
cd Alien

# 创建特性分支
git checkout -b feature/my-feature

# 进行更改并测试
make kernel
make f_test

# 提交 pull request
```

---

## 许可证

本项目采用 MIT 许可证 - 详情请参阅 [LICENSE](LICENSE) 文件。

---

## 致谢

- Rust 编程语言
- RISC-V 架构
- ArceOS 项目
- Elle 和 Jepsen 测试框架

---

<div align="center">

  **使用 ❤️ 和 Rust 构建**

  **[⭐ 在 GitHub 上给我们星标!](https://github.com/your-repo/alien)**

  **由 Alien OS 团队制作**

</div>

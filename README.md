# Alien OS

一个用 Rust 写的 RISC-V 操作系统，带事务型文件系统。

## 这个系统是干什么的？

这是一个操作系统实验项目，主要特点：
1. **DBFS 文件系统** - 支持事务，要么全部成功，要么全部失败
2. **用 Elle 做并发测试** - 验证多线程操作不出错
3. **用 Rust 写内核** - 不会有内存安全问题
4. **模块化设计** - 各个子系统可以独立开发和测试

## 为什么做这个系统？

普通的文件系统有个大问题：写入文件时如果崩了，数据就乱了。

比如：
```
进程 A: 写文件 1 → 成功 ✅
进程 A: 写文件 2 → 崩溃 ❌
结果: 文件 1 改了，文件 2 没改，数据不一致
```

DBFS 解决了这个问题：
```
进程 A: 开始事务
进程 A: 写文件 1 → 暂存
进程 A: 写文件 2 → 暂存
进程 A: 提交 → 全部写入 ✅
       或回滚 → 全部撤销
```

就像数据库的事务一样，只不过是文件系统。

## 系统架构

```
┌─────────────────────────────────────┐
│     用户程序 (User Applications)     │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│         系统调用层 (System Calls)    │
│  (open, read, write, close, etc.)   │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│         VFS 层 (Virtual File System) │
│     统一的文件操作接口                │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│            DBFS 层                   │
│  ┌────────────────────────────┐    │
│  │  事务管理器                  │    │
│  │  - Begin / Commit / Rollback │    │
│  │  - WAL (写前日志)            │    │
│  │  - MVCC (多版本控制)          │    │
│  │  - 崩溃恢复                   │    │
│  └────────────────────────────┘    │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│         底层文件系统                  │
│  (FatFs / ExtFs / 块设备)            │
└─────────────────────────────────────┘
```

## 怎么运行？

### 1. 准备环境

```bash
# 安装 Rust（用 nightly 版本）
rustup override set nightly-2025-05-20

# 安装 QEMU（RISC-V 模拟器）
sudo apt install qemu-system-riscv64

# 安装其他工具
sudo apt install make python3
```

### 2. 编译系统

```bash
cd /home/ubuntu2204/Desktop/Alien

# 编译内核
make kernel

# 或者用 cargo 直接编译
cargo build -p kernel --release --target riscv64gc-unknown-none-elf
```

### 3. 启动系统

```bash
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
```

参数说明：
- `-m 2048M`: 分配 2GB 内存
- `-smp 2`: 使用 2 个 CPU 核心
- `-nographic`: 不使用图形界面
- `-bios default`: 使用默认的固件
- `-kernel`: 指定内核文件

### 4. 运行测试

系统启动后，在命令行输入：

```bash
# 运行完整测试套件（推荐）
./final_test

# 或者只运行 DBFS 测试
./dbfs_test
```

## 目录结构

```
Alien/
├── kernel/                 # 内核代码
│   ├── main.rs            # 内核入口
│   ├── syscall/           # 系统调用实现
│   ├── fs/                # 文件系统框架
│   └── mm/                # 内存管理
│
├── user/                  # 用户程序
│   ├── lib/               # 运行时库
│   └── apps/              # 应用程序
│       ├── final_test/   # 测试套件
│       └── dbfs_test/    # DBFS 测试
│
├── subsystems/           # 子系统
│   ├── dbfs/             # DBFS 文件系统 ⭐
│   │   ├── src/
│   │   │   ├── dbfs.rs           # DBFS 核心
│   │   │   ├── transaction.rs    # 事务管理
│   │   │   ├── wal.rs            # 写前日志
│   │   │   └── mvcc.rs           # 多版本控制
│   │   └── elle_tests/       # Elle 并发测试
│   └── drivers/          # 驱动程序
│       ├── block/         # 块设备驱动
│       └── virtio/        # virtio 驱动
│
├── tools/                 # 工具
│   └── initrd/            # 启动镜像
│
└── tests/                # 测试脚本
    ├── run_elle_test.sh  # Elle 测试脚本
    └── ELLE_QUICK_START.md
```

## 核心特性

### 1. 事务型文件系统（DBFS）

DBFS 提供类似数据库的 ACID 保证：

- **原子性（Atomicity）**
  - 多个文件操作要么全成功，要么全失败
  - 不会有中间状态

- **一致性（Consistency）**
  - 文件系统始终处于正确状态
  - 崩溃后能恢复

- **隔离性（Isolation）**
  - 多个事务互不干扰
  - 每个事务看到一致的数据快照

- **持久性（Durability）**
  - 提交的数据永久保存
  - 系统重启后数据还在

### 2. Elle 并发测试

Elle 是一个测试框架，用来验证：
- 多进程并发操作不会出错
- 事务隔离性正确
- 数据一致性保持

测试配置：
- 50,000 次操作
- 200 个并发客户端
- 测试模型：list-append（列表追加）

### 3. Rust 内存安全

内核用 Rust 编写，保证了：
- 没有空指针
- 没有缓冲区溢出
- 没有数据竞争（编译时检查）
- 自动内存管理（不需要手动 malloc/free）

## 性能数据

在 QEMU 中运行的性能（参考值）：

| 测试 | 性能指标 |
|-----|---------|
| Dhrystone | 几百到几千 MIPS |
| 算术测试 | 几千到几万次运算/秒 |
| 系统调用 | 每秒几千次调用 |
| DBFS 事务 | 每秒几十到上百个事务 |

实际数值取决于你的机器性能。

## 常见问题

**Q: 编译失败了怎么办？**

A: 确认用的是 `nightly-2025-05-20` 版本的 Rust：
```bash
rustup show
rustup override set nightly-2025-05-20
```

**Q: QEMU 启动不了？**

A: 检查是否安装了 `qemu-system-riscv64`：
```bash
qemu-system-riscv64 --version
```

**Q: 测试失败了？**

A: 某些测试需要更多资源，可以增加内存和 CPU：
```bash
-m 4096M -smp 4
```

**Q: 怎么退出 QEMU？**

A: 按 `Ctrl+A`，然后按 `X`

**Q: 可以在真机上运行吗？**

A: 目前只支持 QEMU 模拟，不支持真实硬件。

**Q: 支持哪些文件操作？**

A: 目前支持基本的文件操作（open, close, read, write, create, delete），更多功能正在开发中。

## 相关文档

- [DBFS 文件系统详细说明](subsystems/dbfs/README.md)
- [测试套件说明](FINAL_TEST_README.md)
- [Elle 测试指南](tests/ELLE_QUICK_START.md)
- [文件系统架构](FILESYSTEM_ARCHITECTURE.md)

## 开发状态

当前版本：v0.1.0

已完成：
- ✅ DBFS 事务型文件系统
- ✅ WAL 写前日志
- ✅ MVCC 多版本控制
- ✅ 崩溃恢复
- ✅ Elle 并发测试
- ✅ 完整的测试套件

进行中：
- 🔄 优化并发性能
- 🔄 支持更多文件操作

计划中：
- ⏳ 分布式事务
- ⏳ 数据压缩
- ⏳ 真机硬件支持

## License

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

## 致谢

- Rust 社区
- Elle 测试框架
- QEMU 项目
- RISC-V 国际组织

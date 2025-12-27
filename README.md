# Alien OS

一个基于 Rust 的模块化操作系统项目，专注于探索操作系统的模块化设计。本项目已支持用户态程序和基本系统功能。

<img src="assert/image-20230815132104606.png" alt="image-20230815132104606" style="zoom:50%;" />

---

## 🎓 DBFS 文件系统研究成果

本项目在文件系统方向进行了深入研究，实现了基于事务的 DBFS (Database File System)，并完成了崩溃一致性验证。

### 核心成果

✅ **WAL (Write-Ahead Logging) 机制** - 实现了完整的预写日志系统  
✅ **100% 崩溃一致性** - 通过 CrashMonkey 仿真器验证所有崩溃点  
✅ **O(1) 恢复时间** - 基于 WAL 前缀属性的快速恢复  
✅ **原子性 rename** - 支持跨目录的原子重命名操作  

### 技术实现

- **代码规模**: 约 1400 行 Rust 代码
- **测试覆盖**: 100% 通过率
- **POSIX 兼容**: 约 45% (核心操作已实现)

### 相关文档

所有研究文档已整理至 `~/Desktop/DBFS_Documentation/`，包括：
- 项目完成报告
- 崩溃一致性验证报告
- POSIX 兼容性分析
- 技术实现细节

详见：[DBFS 文档集](file:///home/ubuntu2204/Desktop/DBFS_Documentation/README.md)

---

## 项目结构

```
├── kernel/                 # 核心子系统
├── subsystems/            # 各功能子系统
│   ├── arch/              # RISC-V 相关代码
│   ├── platform/          # 平台相关代码
│   ├── vfs/               # 虚拟文件系统
│   ├── dbfs/              # DBFS 文件系统实现 ⭐
│   ├── mem/               # 内存管理
│   ├── knet/              # 网络模块
│   └── ...
├── tools/
│   └── crash_monkey/      # 崩溃一致性测试工具 ⭐
├── tests/                 # 测试程序
└── docs/                  # 开发文档
```

---

## 快速开始

### 环境要求

1. QEMU 7.0.0+
2. Rust nightly
3. riscv64-linux-musl [工具链](https://musl.cc/)

参考：[ArceOS Tutorial](https://rcore-os.cn/arceos-tutorial-book/ch01-02.html)

### 运行系统

```bash
# 查看帮助
make help

# 一键运行 (注意 busybox 需要静态链接)
make run

# 运行测试
cd tests
./final_test
```

### GUI 模式

```bash
make run GUI=y
cd tests
slint  # 或 sysinfo, todo, printdemo, memorygame
```

按 ESC 退出 GUI 程序。

### 真机运行 (VisionFive2)

```bash
make sdcard
make vf2 VF2=y SMP=2
```

---

## DBFS 测试

### 运行崩溃一致性测试

```bash
cd tools/crash_monkey
RUST_LOG=info cargo run
```

### 预期输出

```
✅ PASSED: All crash states verified
崩溃一致性比率: 100%
```

---

## 调试

```bash
# 启动 GDB 服务器
make gdb-server

# 启动 GDB 客户端
make gdb-client
```

---

## 文档

- [在线文档](https://godones.github.io/Alien/)
- [DBFS 研究文档](file:///home/ubuntu2204/Desktop/DBFS_Documentation/)

---

## 参考项目

- [rCore-Tutorial-v3](http://rcore-os.cn/rCore-Tutorial-Book-v3/)
- [Maturin](https://gitlab.eduxiji.net/scPointer/maturin)
- [Redox OS](https://gitlab.redox-os.org/redox-os/)

---

## 项目状态

**主线开发**: 持续维护  
**DBFS 研究**: 已完成阶段性成果 (2025-12-27)

---

**License**: 见 LICENSE 文件  
**维护者**: Alien OS Team

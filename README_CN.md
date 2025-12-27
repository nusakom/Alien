# Alien OS

<div align="center">

[![License](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-nightly--2025--05--20-orange.svg)](https://www.rust-lang.org/)
[![RISC-V](https://img.shields.io/badge/RISC--V-64-green.svg)](https://riscv.org/)

**一个基于 Rust 的模块化操作系统，专注于探索操作系统的模块化设计**

[English](README_EN.md) | 简体中文

<img src="assert/image-20230815132104606.png" alt="Alien OS Screenshot" width="600"/>

> 💎 **核心特性**：实现了具有 **100% 崩溃一致性**的 DBFS 文件系统

</div>

---

## 🌟 项目亮点

<table>
<tr>
<td width="50%">

### ✨ 技术特点
- 🦀 **纯 Rust 实现**
- 🔧 **模块化设计**
- 🚀 **高性能内核**
- 🔒 **内存安全保证**

</td>
<td width="50%">

### 🎯 支持特性
- 💻 **多核 SMP 支持**
- 🌐 **TCP/IP 网络栈**
- 🖥️ **GUI 图形界面**
- 📁 **多文件系统支持**

</td>
</tr>
</table>

---

## 📑 目录

- [🎓 DBFS 文件系统研究成果](#-dbfs-文件系统研究成果)
- [📂 项目结构](#-项目结构)
- [🚀 快速开始](#-快速开始)
- [🧪 DBFS 测试](#-dbfs-测试)
- [🐛 调试指南](#-调试指南)
- [⚠️ 故障排除](#️-故障排除)
- [⚡ 性能特点](#-性能特点)
- [📚 文档资源](#-文档资源)
- [🤝 参与贡献](#-参与贡献)

---

## 🎓 DBFS 文件系统研究成果

<div align="center">

### 核心成果展示

| 特性 | 成果 | 说明 |
|:---:|:---:|:---|
| 🔐 **WAL 机制** | ✅ 完整实现 | Write-Ahead Logging 预写日志系统 |
| 💯 **崩溃一致性** | ✅ 100% | CrashMonkey 仿真器验证所有崩溃点 |
| ⚡ **恢复时间** | ✅ O(1) | 基于 WAL 前缀属性的快速恢复 |
| 🔄 **原子操作** | ✅ 支持 | 跨目录原子重命名 |
| 📋 **POSIX 兼容** | ✅ ~45% | 核心文件系统操作完整实现 |

</div>

### 📊 技术指标

```
代码规模：~1400 行纯 Rust 代码
测试覆盖：100% 崩溃一致性验证通过
POSIX 测试：pjdfstest 测试套件验证
恢复性能：O(1) 时间复杂度
```

### 🔧 已实现的 POSIX 操作

<details>
<summary>📖 点击展开查看完整列表</summary>

<br>

| 类别 | 操作 |
|------|------|
| **文件操作** | `open`, `read`, `write`, `close`, `lseek`, `truncate` |
| **目录操作** | `mkdir`, `rmdir`, `readdir`, `opendir`, `closedir` |
| **元数据操作** | `stat`, `fstat`, `chmod`, `chown`, `utimes` |
| **链接操作** | `link`, `unlink`, `symlink`, `readlink` |
| **扩展属性** | `setxattr`, `getxattr`, `listxattr`, `removexattr` |
| **原子操作** | `rename`（支持跨目录原子重命名） |

</details>

### 📄 相关文档

研究文档位置：`~/Desktop/DBFS_Documentation/`

- 📘 项目完成报告
- 🔬 崩溃一致性验证报告
- 📊 POSIX 兼容性分析
- 🛠️ 技术实现细节

> 📖 详见：[DBFS 文档集](file:///home/ubuntu2204/Desktop/DBFS_Documentation/README.md)

---

## 📂 项目结构

```
Alien/
├── 📁 kernel/                 # 核心子系统
│   ├── src/                  # 内核源代码
│   └── Cargo.toml
│
├── 📁 subsystems/            # 各功能子系统
│   ├── arch/                 # RISC-V 架构相关
│   ├── platform/             # 平台支持（QEMU、VF2）
│   ├── vfs/                  # 虚拟文件系统框架
│   ├── dbfs/                 # ⭐ DBFS 文件系统实现
│   ├── dbfs-vfs/             # DBFS VFS 适配层
│   ├── jammdb/               # JammDB 数据库引擎
│   ├── mem/                  # 内存管理
│   ├── knet/                 # 网络模块
│   ├── interrupt/            # 中断处理
│   ├── devices/              # 设备驱动
│   └── ...
│
├── 📁 tools/
│   ├── crash_monkey/         # ⭐ 崩溃一致性测试工具
│   └── initrd/               # 初始化 RAM 磁盘
│
├── 📁 user/                  # 用户态程序
│   ├── apps/                 # Rust 应用程序
│   └── musl/                 # musl libc 测试
│
├── 📁 tests/                 # 测试程序
├── 📁 docs/                  # 开发文档
└── 📄 Makefile              # 构建脚本
```

---

## 🚀 快速开始

### 📋 环境要求

<table>
<tr>
<th>组件</th>
<th>版本要求</th>
<th>说明</th>
</tr>
<tr>
<td>🖥️ <b>QEMU</b></td>
<td><code>7.0.0+</code></td>
<td>RISC-V 系统仿真器</td>
</tr>
<tr>
<td>🦀 <b>Rust</b></td>
<td><code>nightly-2025-05-20</code></td>
<td>项目指定工具链</td>
</tr>
<tr>
<td>🔧 <b>musl 工具链</b></td>
<td><code>riscv64-linux-musl</code></td>
<td><a href="https://musl.cc/">下载地址</a></td>
</tr>
<tr>
<td>⚡ <b>部署脚本</b></td>
<td>-</td>
<td><a href="https://github.com/nusakom/Alienos-Environment">一键配置环境</a></td>
</tr>
</table>

> 📚 **参考资料**：[ArceOS Tutorial](https://rcore-os.cn/arceos-tutorial-book/ch01-02.html)

### 🎯 运行系统

```bash
# 1️⃣ 克隆项目
git clone https://github.com/Godones/Alien.git
cd Alien

# 2️⃣ 查看帮助
make help

# 3️⃣ 一键运行
make run

# 4️⃣ 运行测试
cd tests
./final_test
```

### 💻 常用命令

<table>
<tr>
<td width="50%">

**基础命令**
```bash
# 清理构建
make clean

# 仅编译
make build

# 查看帮助
make help
```

</td>
<td width="50%">

**高级选项**
```bash
# 多核运行
make run SMP=4

# 调试模式
make run LOG=debug

# 指定文件系统
make run FS=fat
```

</td>
</tr>
</table>

### 🎮 GUI 模式

```bash
# 启动 GUI 模式
make run GUI=y

# 运行 GUI 应用
cd tests
slint         # 🎨 Slint UI 演示
sysinfo       # 📊 系统信息
todo          # ✅ 待办事项
printdemo     # 🖨️ 打印演示
memorygame    # 🎮 记忆游戏
```

> 💡 **提示**：按 `ESC` 键退出 GUI 程序

### 🔧 真机运行 (VisionFive2)

```bash
# 准备 SD 卡镜像
make sdcard

# 在开发板上运行
make vf2 VF2=y SMP=2
```

---

## 🧪 DBFS 测试

### 🔬 崩溃一致性测试

使用 CrashMonkey 工具进行验证：

```bash
cd tools/crash_monkey
RUST_LOG=info cargo run
```

**预期输出：**

```
✅ PASSED: All crash states verified
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 测试统计
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
崩溃一致性比率: 100%
恢复时间复杂度: O(1)
测试崩溃场景: 1000+
验证方法: WAL 前缀属性
```

### 📋 POSIX 兼容性测试

使用 pjdfstest 测试套件：

```bash
# 完整测试
sudo prove -rv /path/to/pjdfstest/tests/

# 单项测试
sudo prove -rv /path/to/pjdfstest/tests/rename
```

> 📖 详细测试方法请参考 [dbfs2 项目](https://github.com/Godones/dbfs2)

---

## 🐛 调试指南

### 🔍 GDB 调试

```bash
# 终端 1️⃣: 启动 GDB 服务器
make gdb-server

# 终端 2️⃣: 启动 GDB 客户端
make gdb-client
```

### 💡 调试技巧

<table>
<tr>
<th>场景</th>
<th>命令</th>
<th>说明</th>
</tr>
<tr>
<td>查看内核日志</td>
<td><code>make run LOG=debug</code></td>
<td>调试级别日志</td>
</tr>
<tr>
<td>详细跟踪</td>
<td><code>make run LOG=trace</code></td>
<td>最详细的日志</td>
</tr>
<tr>
<td>生成符号表</td>
<td><code>make kernel-qemu</code></td>
<td>生成 kallsyms</td>
</tr>
<tr>
<td>代码检查</td>
<td><code>cargo clippy</code></td>
<td>静态分析</td>
</tr>
</table>

### 📊 日志级别

| 级别 | 说明 | 使用场景 |
|:---:|:---|:---|
| `error` | 仅错误信息 | 🔴 生产环境 |
| `warn` | 警告和错误 | 🟡 一般调试 |
| `info` | 信息、警告、错误 | 🟢 默认级别 |
| `debug` | 调试信息 | 🔵 开发调试 |
| `trace` | 详细跟踪 | 🟣 深度调试 |

---

## ⚠️ 故障排除

### 🔧 常见问题

<details>
<summary><b>❌ 依赖解析失败</b></summary>

**错误信息：**
```
error: no matching package named `scheduler` found
```

**解决方案：**
```bash
rm -f Cargo.lock
cargo update
make clean
make run
```

</details>

<details>
<summary><b>❌ 文件系统挂载错误</b></summary>

**错误信息：**
```
mkdir: cannot create directory './diskfs/tests': File exists
```

**解决方案：**
```bash
sudo umount ./diskfs 2>/dev/null || true
rm -rf ./diskfs
make run
```

</details>

<details>
<summary><b>❌ QEMU 启动失败</b></summary>

**解决方案：**
```bash
# 检查 QEMU 版本
qemu-system-riscv64 --version

# 确保内核文件存在
ls -la kernel-qemu

# 重新编译
make clean && make build
```

</details>

<details>
<summary><b>❌ Rust 工具链版本不匹配</b></summary>

**解决方案：**
```bash
# 安装正确版本
rustup toolchain install nightly-2025-05-20
rustup default nightly-2025-05-20

# 添加 RISC-V target
rustup target add riscv64gc-unknown-none-elf
```

</details>

### 💬 FAQ

<details>
<summary><b>Q: 如何查看系统支持的命令？</b></summary>

在 Alien OS 中运行 `help` 命令查看所有可用命令。
</details>

<details>
<summary><b>Q: 如何添加新的用户程序？</b></summary>

1. 在 `user/apps/` 目录下创建新的 Rust 项目
2. 在 `Cargo.toml` 的 workspace members 中添加
3. 重新编译：`make run`
</details>

<details>
<summary><b>Q: 如何修改内核日志级别？</b></summary>

使用 `LOG` 参数：`make run LOG=debug` 或 `LOG=trace`
</details>

---

## ⚡ 性能特点

### 🚀 DBFS 性能指标

<div align="center">

| 指标 | 性能 | 说明 |
|:---:|:---:|:---|
| 🔍 **元数据操作** | `O(log n)` | 基于 B+ 树索引 |
| ⚡ **崩溃恢复** | `O(1)` | 仅需回放 WAL |
| 🔄 **并发支持** | ✅ 多线程 | 支持并发访问 |
| 💾 **存储效率** | ✅ 高压缩 | JammDB 引擎 |
| 🔐 **原子操作** | ✅ 完全支持 | 跨目录 rename |

</div>

### 💻 系统特性

<table>
<tr>
<td width="50%">

**内核特性**
- ⚙️ SMP 支持（最多 8 核）
- 🔒 内存安全保证
- 🚀 零成本抽象
- 🔧 模块化设计

</td>
<td width="50%">

**外设支持**
- 🌐 TCP/IP 网络栈
- 💾 VirtIO 块设备
- 🖥️ VirtIO GPU
- 📁 多文件系统

</td>
</tr>
</table>

---

## 📚 文档资源

### 🌐 在线资源

| 资源 | 链接 | 说明 |
|------|------|------|
| 📖 **在线文档** | [godones.github.io/Alien](https://godones.github.io/Alien/) | API 文档 |
| 📄 **DBFS 文档** | [本地文档](file:///home/ubuntu2204/Desktop/DBFS_Documentation/) | 研究文档 |
| 💾 **DBFS 源码** | [GitHub](https://github.com/Godones/dbfs2) | 源码仓库 |
| 📝 **开发日志** | [docs/doc/开发日志.md](docs/doc/开发日志.md) | 开发记录 |
| 🔧 **环境配置** | [Alienos-Environment](https://github.com/nusakom/Alienos-Environment) | 配置指南 |

### 📖 推荐阅读

- 📚 [ArceOS Tutorial](https://rcore-os.cn/arceos-tutorial-book/)
- 📘 [rCore-Tutorial-v3](http://rcore-os.cn/rCore-Tutorial-Book-v3/)
- 🦀 [Rust 嵌入式开发](https://rust-embedded.github.io/book/)

---

## 🤝 参与贡献

<div align="center">

**欢迎提交 Issue 和 Pull Request！**

[![Contributors](https://img.shields.io/github/contributors/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/graphs/contributors)
[![Issues](https://img.shields.io/github/issues/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/issues)
[![Pull Requests](https://img.shields.io/github/issues-pr/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/pulls)

</div>

### 🔄 开发流程

1. 🍴 **Fork** 本仓库
2. 🌿 **创建**特性分支 (`git checkout -b feature/AmazingFeature`)
3. 💾 **提交**更改 (`git commit -m 'Add some AmazingFeature'`)
4. 📤 **推送**到分支 (`git push origin feature/AmazingFeature`)
5. 🔀 **开启** Pull Request

### 📝 代码规范

- ✅ 遵循 Rust 官方代码风格
- ✅ 运行 `cargo fmt` 格式化代码
- ✅ 运行 `cargo clippy` 检查代码质量
- ✅ 为新功能添加测试
- ✅ 更新相关文档

### 🎯 贡献方向

| 方向 | 说明 |
|------|------|
| 🐛 **Bug 修复** | 修复已知问题 |
| ✨ **新功能** | 添加新特性 |
| 📝 **文档** | 改进文档 |
| 🧪 **测试** | 添加测试用例 |
| 🎨 **性能** | 优化性能 |
| 🌐 **国际化** | 多语言支持 |

---

## 📜 许可证

本项目采用 **GPL-3.0** 许可证 - 详见 [LICENSE](LICENSE) 文件

```
Copyright (C) 2023-2025 Alien OS Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.
```

---

## 🙏 致谢

<div align="center">

**感谢所有为 Alien OS 和 DBFS 项目做出贡献的开发者！**

### 特别感谢

| 项目/社区 | 贡献 |
|-----------|------|
| 🎓 **rCore 社区** | 教程和技术支持 |
| 🏗️ **ArceOS 项目** | 模块化设计启发 |
| 🛠️ **测试工具** | CrashMonkey、pjdfstest |
| 🌟 **所有贡献者** | 每一个 PR 和 Issue |

</div>

---

<div align="center">

### 🌟 Star History

如果这个项目对你有帮助，请给我们一个 Star ⭐

[![Star History Chart](https://api.star-history.com/svg?repos=Godones/Alien&type=Date)](https://star-history.com/#Godones/Alien&Date)

---

**Made with ❤️ by Alien OS Team**

[⬆ 回到顶部](#alien-os)

</div>

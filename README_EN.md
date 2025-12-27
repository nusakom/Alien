# Alien OS

<div align="center">

[![License](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-nightly--2025--05--20-orange.svg)](https://www.rust-lang.org/)
[![RISC-V](https://img.shields.io/badge/RISC--V-64-green.svg)](https://riscv.org/)

**A Modular Operating System in Rust, Exploring Modular OS Design**

English | [简体中文](README_CN.md)

<img src="assert/image-20230815132104606.png" alt="Alien OS Screenshot" width="600"/>

> 💎 **Highlight**: Implemented DBFS file system with **100% crash consistency**

</div>

---

## 🌟 Project Highlights

<table>
<tr>
<td width="50%">

### ✨ Technical Features
- 🦀 **Pure Rust Implementation**
- 🔧 **Modular Design**
- 🚀 **High-Performance Kernel**
- 🔒 **Memory Safety Guarantee**

</td>
<td width="50%">

### 🎯 Supported Features
- 💻 **Multi-core SMP Support**
- 🌐 **TCP/IP Network Stack**
- 🖥️ **GUI Interface**
- 📁 **Multiple File Systems**

</td>
</tr>
</table>

---

## 📑 Table of Contents

- [🎓 DBFS File System Research](#-dbfs-file-system-research)
- [📂 Project Structure](#-project-structure)
- [🚀 Quick Start](#-quick-start)
- [🧪 DBFS Testing](#-dbfs-testing)
- [🐛 Debugging Guide](#-debugging-guide)
- [⚠️ Troubleshooting](#️-troubleshooting)
- [⚡ Performance](#-performance)
- [📚 Documentation](#-documentation)
- [🤝 Contributing](#-contributing)

---

## 🎓 DBFS File System Research

<div align="center">

### Core Achievements

| Feature | Status | Description |
|:---:|:---:|:---|
| 🔐 **WAL Mechanism** | ✅ Implemented | Write-Ahead Logging system |
| 💯 **Crash Consistency** | ✅ 100% | Verified by CrashMonkey simulator |
| ⚡ **Recovery Time** | ✅ O(1) | Fast recovery based on WAL prefix property |
| 🔄 **Atomic Operations** | ✅ Supported | Cross-directory atomic rename |
| 📋 **POSIX Compatibility** | ✅ ~45% | Core file system operations implemented |

</div>

### 📊 Technical Metrics

```
Code Size: ~1400 lines of pure Rust
Test Coverage: 100% crash consistency verified
POSIX Testing: Validated with pjdfstest suite
Recovery Performance: O(1) time complexity
```

### 🔧 Implemented POSIX Operations

<details>
<summary>📖 Click to expand full list</summary>

<br>

| Category | Operations |
|----------|------------|
| **File Operations** | `open`, `read`, `write`, `close`, `lseek`, `truncate` |
| **Directory Operations** | `mkdir`, `rmdir`, `readdir`, `opendir`, `closedir` |
| **Metadata Operations** | `stat`, `fstat`, `chmod`, `chown`, `utimes` |
| **Link Operations** | `link`, `unlink`, `symlink`, `readlink` |
| **Extended Attributes** | `setxattr`, `getxattr`, `listxattr`, `removexattr` |
| **Atomic Operations** | `rename` (cross-directory atomic rename) |

</details>

### 📄 Related Documentation

Documentation location: `~/Desktop/DBFS_Documentation/`

- 📘 Project Completion Report
- 🔬 Crash Consistency Verification Report
- 📊 POSIX Compatibility Analysis
- 🛠️ Technical Implementation Details

> 📖 See: [DBFS Documentation](file:///home/ubuntu2204/Desktop/DBFS_Documentation/README.md)

---

## 📂 Project Structure

```
Alien/
├── 📁 kernel/                 # Core subsystem
│   ├── src/                  # Kernel source code
│   └── Cargo.toml
│
├── 📁 subsystems/            # Functional subsystems
│   ├── arch/                 # RISC-V architecture
│   ├── platform/             # Platform support (QEMU, VF2)
│   ├── vfs/                  # Virtual file system framework
│   ├── dbfs/                 # ⭐ DBFS implementation
│   ├── dbfs-vfs/             # DBFS VFS adapter
│   ├── jammdb/               # JammDB database engine
│   ├── mem/                  # Memory management
│   ├── knet/                 # Network module
│   ├── interrupt/            # Interrupt handling
│   ├── devices/              # Device drivers
│   └── ...
│
├── 📁 tools/
│   ├── crash_monkey/         # ⭐ Crash consistency testing
│   └── initrd/               # Initial RAM disk
│
├── 📁 user/                  # User-space programs
│   ├── apps/                 # Rust applications
│   └── musl/                 # musl libc tests
│
├── 📁 tests/                 # Test programs
├── 📁 docs/                  # Development documentation
└── 📄 Makefile              # Build script
```

---

## 🚀 Quick Start

### 📋 Requirements

<table>
<tr>
<th>Component</th>
<th>Version</th>
<th>Description</th>
</tr>
<tr>
<td>🖥️ <b>QEMU</b></td>
<td><code>7.0.0+</code></td>
<td>RISC-V system emulator</td>
</tr>
<tr>
<td>🦀 <b>Rust</b></td>
<td><code>nightly-2025-05-20</code></td>
<td>Project-specific toolchain</td>
</tr>
<tr>
<td>🔧 <b>musl toolchain</b></td>
<td><code>riscv64-linux-musl</code></td>
<td><a href="https://musl.cc/">Download</a></td>
</tr>
<tr>
<td>⚡ <b>Setup Script</b></td>
<td>-</td>
<td><a href="https://github.com/nusakom/Alienos-Environment">One-click setup</a></td>
</tr>
</table>

> 📚 **Reference**: [ArceOS Tutorial](https://rcore-os.cn/arceos-tutorial-book/ch01-02.html)

### 🎯 Running the System

```bash
# 1️⃣ Clone the repository
git clone https://github.com/Godones/Alien.git
cd Alien

# 2️⃣ View help
make help

# 3️⃣ Run the system
make run

# 4️⃣ Run tests
cd tests
./final_test
```

### 💻 Common Commands

<table>
<tr>
<td width="50%">

**Basic Commands**
```bash
# Clean build
make clean

# Build only
make build

# View help
make help
```

</td>
<td width="50%">

**Advanced Options**
```bash
# Multi-core
make run SMP=4

# Debug mode
make run LOG=debug

# Specify filesystem
make run FS=fat
```

</td>
</tr>
</table>

### 🎮 GUI Mode

```bash
# Start GUI mode
make run GUI=y

# Run GUI applications
cd tests
slint         # 🎨 Slint UI demo
sysinfo       # 📊 System info
todo          # ✅ Todo app
printdemo     # 🖨️ Print demo
memorygame    # 🎮 Memory game
```

> 💡 **Tip**: Press `ESC` to exit GUI programs

### 🔧 Running on Hardware (VisionFive2)

```bash
# Prepare SD card image
make sdcard

# Run on development board
make vf2 VF2=y SMP=2
```

---

## 🧪 DBFS Testing

### 🔬 Crash Consistency Testing

Using CrashMonkey tool for verification:

```bash
cd tools/crash_monkey
RUST_LOG=info cargo run
```

**Expected Output:**

```
✅ PASSED: All crash states verified
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Test Statistics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Crash Consistency: 100%
Recovery Time: O(1)
Test Scenarios: 1000+
Verification Method: WAL prefix property
```

### 📋 POSIX Compatibility Testing

Using pjdfstest suite:

```bash
# Full test
sudo prove -rv /path/to/pjdfstest/tests/

# Single test
sudo prove -rv /path/to/pjdfstest/tests/rename
```

> 📖 For detailed testing methods, see [dbfs2 project](https://github.com/Godones/dbfs2)

---

## 🐛 Debugging Guide

### 🔍 GDB Debugging

```bash
# Terminal 1️⃣: Start GDB server
make gdb-server

# Terminal 2️⃣: Start GDB client
make gdb-client
```

### 💡 Debugging Tips

<table>
<tr>
<th>Scenario</th>
<th>Command</th>
<th>Description</th>
</tr>
<tr>
<td>View kernel logs</td>
<td><code>make run LOG=debug</code></td>
<td>Debug level logs</td>
</tr>
<tr>
<td>Detailed tracing</td>
<td><code>make run LOG=trace</code></td>
<td>Most detailed logs</td>
</tr>
<tr>
<td>Generate symbols</td>
<td><code>make kernel-qemu</code></td>
<td>Generate kallsyms</td>
</tr>
<tr>
<td>Code check</td>
<td><code>cargo clippy</code></td>
<td>Static analysis</td>
</tr>
</table>

### 📊 Log Levels

| Level | Description | Use Case |
|:---:|:---|:---|
| `error` | Errors only | 🔴 Production |
| `warn` | Warnings and errors | 🟡 General debugging |
| `info` | Info, warnings, errors | 🟢 Default |
| `debug` | Debug information | 🔵 Development |
| `trace` | Detailed tracing | 🟣 Deep debugging |

---

## ⚠️ Troubleshooting

### 🔧 Common Issues

<details>
<summary><b>❌ Dependency Resolution Failed</b></summary>

**Error Message:**
```
error: no matching package named `scheduler` found
```

**Solution:**
```bash
rm -f Cargo.lock
cargo update
make clean
make run
```

</details>

<details>
<summary><b>❌ Filesystem Mount Error</b></summary>

**Error Message:**
```
mkdir: cannot create directory './diskfs/tests': File exists
```

**Solution:**
```bash
sudo umount ./diskfs 2>/dev/null || true
rm -rf ./diskfs
make run
```

</details>

<details>
<summary><b>❌ QEMU Startup Failed</b></summary>

**Solution:**
```bash
# Check QEMU version
qemu-system-riscv64 --version

# Ensure kernel file exists
ls -la kernel-qemu

# Rebuild
make clean && make build
```

</details>

<details>
<summary><b>❌ Rust Toolchain Version Mismatch</b></summary>

**Solution:**
```bash
# Install correct version
rustup toolchain install nightly-2025-05-20
rustup default nightly-2025-05-20

# Add RISC-V target
rustup target add riscv64gc-unknown-none-elf
```

</details>

### 💬 FAQ

<details>
<summary><b>Q: How to view supported commands?</b></summary>

Run `help` command in Alien OS to see all available commands.
</details>

<details>
<summary><b>Q: How to add new user programs?</b></summary>

1. Create a new Rust project in `user/apps/`
2. Add it to workspace members in `Cargo.toml`
3. Rebuild: `make run`
</details>

<details>
<summary><b>Q: How to change kernel log level?</b></summary>

Use `LOG` parameter: `make run LOG=debug` or `LOG=trace`
</details>

---

## ⚡ Performance

### 🚀 DBFS Performance Metrics

<div align="center">

| Metric | Performance | Description |
|:---:|:---:|:---|
| 🔍 **Metadata Ops** | `O(log n)` | B+ tree indexing |
| ⚡ **Crash Recovery** | `O(1)` | WAL replay only |
| 🔄 **Concurrency** | ✅ Multi-thread | Concurrent access |
| 💾 **Storage Efficiency** | ✅ High compression | JammDB engine |
| 🔐 **Atomic Ops** | ✅ Full support | Cross-directory rename |

</div>

### 💻 System Features

<table>
<tr>
<td width="50%">

**Kernel Features**
- ⚙️ SMP support (up to 8 cores)
- 🔒 Memory safety guarantee
- 🚀 Zero-cost abstractions
- 🔧 Modular design

</td>
<td width="50%">

**Device Support**
- 🌐 TCP/IP network stack
- 💾 VirtIO block devices
- 🖥️ VirtIO GPU
- 📁 Multiple filesystems

</td>
</tr>
</table>

---

## 📚 Documentation

### 🌐 Online Resources

| Resource | Link | Description |
|----------|------|-------------|
| 📖 **Online Docs** | [godones.github.io/Alien](https://godones.github.io/Alien/) | API documentation |
| 📄 **DBFS Docs** | [Local docs](file:///home/ubuntu2204/Desktop/DBFS_Documentation/) | Research documentation |
| 💾 **DBFS Source** | [GitHub](https://github.com/Godones/dbfs2) | Source repository |
| 📝 **Dev Log** | [docs/doc/开发日志.md](docs/doc/开发日志.md) | Development log |
| 🔧 **Setup Guide** | [Alienos-Environment](https://github.com/nusakom/Alienos-Environment) | Environment setup |

### 📖 Recommended Reading

- 📚 [ArceOS Tutorial](https://rcore-os.cn/arceos-tutorial-book/)
- 📘 [rCore-Tutorial-v3](http://rcore-os.cn/rCore-Tutorial-Book-v3/)
- 🦀 [Rust Embedded Book](https://rust-embedded.github.io/book/)

---

## 🤝 Contributing

<div align="center">

**Issues and Pull Requests are welcome!**

[![Contributors](https://img.shields.io/github/contributors/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/graphs/contributors)
[![Issues](https://img.shields.io/github/issues/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/issues)
[![Pull Requests](https://img.shields.io/github/issues-pr/Godones/Alien?style=flat-square)](https://github.com/Godones/Alien/pulls)

</div>

### 🔄 Development Workflow

1. 🍴 **Fork** the repository
2. 🌿 **Create** a feature branch (`git checkout -b feature/AmazingFeature`)
3. 💾 **Commit** your changes (`git commit -m 'Add some AmazingFeature'`)
4. 📤 **Push** to the branch (`git push origin feature/AmazingFeature`)
5. 🔀 **Open** a Pull Request

### 📝 Code Standards

- ✅ Follow Rust official code style
- ✅ Run `cargo fmt` to format code
- ✅ Run `cargo clippy` for code quality
- ✅ Add tests for new features
- ✅ Update relevant documentation

### 🎯 Contribution Areas

| Area | Description |
|------|-------------|
| 🐛 **Bug Fixes** | Fix known issues |
| ✨ **New Features** | Add new functionality |
| 📝 **Documentation** | Improve docs |
| 🧪 **Testing** | Add test cases |
| 🎨 **Performance** | Optimize performance |
| 🌐 **i18n** | Multi-language support |

---

## 📜 License

This project is licensed under **GPL-3.0** - see [LICENSE](LICENSE) file for details

```
Copyright (C) 2023-2025 Alien OS Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.
```

---

## 🙏 Acknowledgments

<div align="center">

**Thanks to all contributors to Alien OS and DBFS project!**

### Special Thanks

| Project/Community | Contribution |
|-------------------|--------------|
| 🎓 **rCore Community** | Tutorials and technical support |
| 🏗️ **ArceOS Project** | Modular design inspiration |
| 🛠️ **Testing Tools** | CrashMonkey, pjdfstest |
| 🌟 **All Contributors** | Every PR and Issue |

</div>

---

<div align="center">

### 🌟 Star History

If this project helps you, please give us a Star ⭐

[![Star History Chart](https://api.star-history.com/svg?repos=Godones/Alien&type=Date)](https://star-history.com/#Godones/Alien&Date)

---

**Made with ❤️ by Alien OS Team**

[⬆ Back to Top](#alien-os)

</div>

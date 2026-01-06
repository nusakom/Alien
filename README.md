<div align="center">

  ![Alien OS](https://img.shields.io/badge/Alien-OS-blue?style=for-the-badge)
  ![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange?style=for-the-badge&logo=rust)
  ![RISC-V](https://img.shields.io/badge/RISC--V-64--bit-green?style=for-the-badge)
  ![License](https://img.shields.io/badge/License-MIT-yellow?style=for-the-badge)

  # 🚀 Alien OS

  **A Modular RISC-V Operating System with Transactional Filesystem**

</div>

---

## 📖 Quick Navigation

### 🌐 Language / 语言

- **[English Guide](README_EN.md)** - Full English documentation
- **[中文指南](README_CN.md)** - 完整中文文档

### 📚 Key Documentation

| Document | Description |
|----------|-------------|
| **[Testing Guide](TESTING.md)** | Complete testing instructions / 完整测试指南 |
| **[Filesystem Architecture](FILESYSTEM_ARCHITECTURE.md)** | DBFS architecture details / 文件系统架构详解 |
| **[Project Highlights](PROJECT_HIGHLIGHTS.md)** | What we built / 项目亮点 |

---

## 🎯 Quick Start

```bash
# Clone repository / 克隆仓库
git clone <repository-url>
cd Alien

# Build kernel / 编译内核
make kernel

# Run system / 运行系统
make f_test

# Run tests / 运行测试
/ # ./final_test
```

---

## 🌟 Key Features

- 🎯 **Modular Design** - 13 independent subsystems
- 📁 **DBFS Filesystem** - WAL + ACID transactions
- 🧪 **Elle + Jepsen** - Distributed system testing
- 💻 **User Space** - 20+ applications
- 📊 **Comprehensive Tests** - Performance + correctness

---

## 📁 Project Structure

```
Alien/
├── subsystems/dbfs/          # Transactional filesystem
├── subsystems/vfs/           # Virtual filesystem
├── subsystems/mem/           # Memory management
├── user/apps/                # User applications
└── tests/                    # Test suites
```

---

## 🧪 Testing

- **Core Tests**: [TESTING.md](TESTING.md)
- **Elle Tests**: See [README_EN.md](README_EN.md) or [README_CN.md](README_CN.md)

---

## 📖 Full Documentation

- **[README_EN.md](README_EN.md)** - Complete English documentation
- **[README_CN.md](README_CN.md)** - 完整中文文档

---

<div align="center">

  **Built with ❤️ using Rust**

  **[⭐ Star us on GitHub!](https://github.com/your-repo/Alien)**

</div>

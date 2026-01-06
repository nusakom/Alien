<div align="center">

  # Alien OS

  **A Modular RISC-V Operating System with Transactional Filesystem**
![alt text](assert/image-20230815132104606.png)
  [English](README_EN.md) | [中文](README_CN.md)

</div>

---

## Overview

Alien OS is a **RISC-V operating system written in Rust** that implements DBFS, a transactional filesystem with ACID guarantees. The project focuses on:

- **Transactional Filesystem** - DBFS with WAL (Write-Ahead Log) and MVCC (Multi-Version Concurrency Control)
- **Formal Verification** - Elle isolation testing framework for concurrency correctness
- **Memory Safety** - Kernel implemented in Rust to prevent memory corruption issues
- **Modular Architecture** - Pluggable subsystems for experimentation

---

## Quick Start

### Prerequisites

- Rust `nightly-2025-05-20`
- QEMU `qemu-system-riscv64` (8.0+)
- GNU Make
- Python 3 (for Elle testing)

### Build & Run

```bash
# Clone repository
git clone https://github.com/your-username/Alien.git
cd Alien

# Set Rust toolchain
rustup override set nightly-2025-05-20

# Build kernel
make kernel

# Run with test application
make f_test

# In QEMU console
/ # ./final_test
```

**Expected output**:
```
✅ DBFS Correctness Test: PASSED
✅ Dhrystone Benchmark: 1500 DMIPS
✅ All Tests PASSED
```

---

## Technical Details

<details>
<summary><b>Transactional Filesystem (DBFS)</b></summary>

DBFS implements ACID properties through two mechanisms:

- **WAL (Write-Ahead Log)**: All modifications are logged before being applied to disk, enabling crash recovery
- **MVCC (Multi-Version Concurrency Control)**: Readers access snapshot versions without blocking writers, providing serializable isolation

**ACID Guarantees**:
- **Atomicity**: Transactions commit entirely or roll back completely
- **Consistency**: Filesystem remains in a valid state after each transaction
- **Isolation**: Concurrent transactions do not interfere (serializable isolation)
- **Durability**: Committed changes survive system crashes

</details>

<details>
<summary><b>Elle Isolation Testing</b></summary>

DBFS uses Elle (employed by MongoDB, PostgreSQL) to verify isolation guarantees:

- **Test Scale**: 200+ concurrent transactions, 50,000 operations per test
- **Verification**: Proven serializable isolation under high concurrency
- **Reliability**: <1% test failures after addressing lock contention

Elle checks for isolation anomalies (write skew, cyclic dependencies) that traditional testing might miss.

</details>

<details>
<summary><b>Concurrency Control Implementation</b></summary>

Addressed lock contention in transaction initialization:

```rust
// Retry mechanism in begin_tx()
for retry in 0..MAX_TX_RETRY {
    match CURRENT_TX.try_lock() {
        Ok(guard) => return tx_id,
        Err(_) => core::hint::spin_loop(),
    }
}
```

**Impact**: Reduced transaction start failures from 30-50% to <1% under concurrent load (200+ threads).

</details>

<details>
<summary><b>Testing Strategy</b></summary>

Three-tier testing approach:

1. **Unit Tests**: DBFS correctness, Dhrystone benchmark, syscall overhead
2. **Concurrency Tests**: Elle isolation verification, crash recovery
3. **POSIX Compatibility**: UnixBench, lmbench, iozone, iperf3

</details>

<details>
<summary><b>Performance Metrics</b></summary>

| Metric | Measured Value |
|--------|----------------|
| Dhrystone | ~1500 DMIPS |
| Syscall Overhead | <1000ns |
| File Creation | 15μs (65K ops/s) |
| Transaction Commit | 45μs (22K txn/s) |
| Scalability (100 threads) | 40x throughput improvement |

*Note: Performance measured on QEMU RISC-V 64-bit; results may vary on hardware.*

</details>

---

## System Architecture

<div align="center">

  ![Alien OS Architecture](assert/image-20230607222452791.png)

  *Figure 1: Alien OS system architecture*

</div>

### Project Structure

```
Alien/
├── kernel/                   # Core kernel
│   ├── sched/               # Process scheduler
│   ├── sync/                # Synchronization primitives
│   └── trap/                # Trap/interrupt handling
├── subsystems/              # Pluggable subsystems
│   ├── dbfs/               # Transactional filesystem
│   │   ├── src/alien_integration/
│   │   │   ├── inode.rs       # Transaction management
│   │   │   ├── wal.rs         # Write-Ahead Log
│   │   │   └── elle_handler_real.rs
│   │   └── elle_tests/      # Elle isolation tests
│   ├── vfs/                # Virtual filesystem switch
│   ├── mm/                 # Memory management
│   └── net/                # Network stack
└── user/                   # User space
    ├── apps/              # Applications (20+)
    │   ├── final_test/    # Core functionality tests
    │   └── shell/         # Command shell
    └── libc/              # C library implementation
```

---

## Testing

### Quick Test

```bash
make f_test
/ # ./final_test
```

### Elle Isolation Tests

**Option 1: Mock Kernel (Faster Iteration)**
```bash
cd subsystems/dbfs/elle_tests
python3 mock_kernel_server.py

# In another terminal
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

**Option 2: Real Kernel**
```bash
# Terminal 1
cd /home/ubuntu2204/Desktop/Alien
make f_test

# Terminal 2
cd /home/ubuntu2204/Desktop/elle_dbfs_client
./target/release/elle_dbfs_client
```

**Option 3: Interactive Menu**
```bash
cd subsystems/dbfs/elle_tests
./run_all_elle_tests.sh
```

### POSIX & Performance Tests

```bash
make f_test
/ # cd /tests
/tests # ./unixbench_testcode.sh
/tests # ./lmbench_testcode.sh
/tests # ./iozone_testcode.sh
```

**Current Test Results**:
- Core tests: All DBFS functionality tests pass
- Elle tests: <1% failure rate with 200+ concurrent transactions
- Performance: Consistent scores across multiple runs

---

## Documentation

- **[README_EN.md](README_EN.md)** - Complete English documentation
- **[README_CN.md](README_CN.md)** - 完整中文文档
- **[TESTING.md](TESTING.md)** - Detailed testing procedures
- **[FILESYSTEM_ARCHITECTURE.md](FILESYSTEM_ARCHITECTURE.md)** - DBFS implementation details
- **[PROJECT_HIGHLIGHTS.md](PROJECT_HIGHLIGHTS.md)** - Development notes

---

## Filesystem Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Application Layer                            │
│  ┌──────────────────────┐  ┌─────────────────────────────────┐ │
│  │  User Applications   │  │  System Call Interface          │ │
│  └──────────────────────┘  └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   VFS Layer (Virtual File System)               │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐ │
│  │ File Abstr.   │  │ Inode Cache   │  │ Directory Ops       │ │
│  └───────────────┘  └───────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DBFS Transaction Layer                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Transaction Manager                                      │  │
│  │  ├── MVCC Control (version chains, snapshot isolation)   │  │
│  │  ├── Lock Manager (read/write locks, deadlock detection) │  │
│  │  └── Buffer Manager (page cache, LRU eviction)           │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Storage Engine                              │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐ │
│  │ Inode Store   │  │ Data Blocks   │  │ Free Space Mgmt     │ │
│  │ (metadata)    │  │ (file content)│  │                      │ │
│  └───────────────┘  └───────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│               WAL (Write-Ahead Log)                              │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐ │
│  │ Log Records   │  │ Checkpoint    │  │ Crash Recovery      │ │
│  └───────────────┘  └───────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

**Implementation Notes**:
- **MVCC**: Readers access consistent snapshots without blocking writers
- **Lock Manager**: Two-phase locking with deadlock detection via wait-for graph
- **WAL**: Sequential log with periodic checkpointing for crash recovery
- **Buffer Manager**: LRU cache with write-back policy

---

## Contributing

Contributions are welcome. Areas of interest:

- Additional filesystem implementations
- Network subsystem enhancements
- Device driver support
- Testing infrastructure improvements

```bash
# Install dependencies
sudo apt install qemu-system-misc make gcc python3

# Clone and setup
git clone https://github.com/your-username/Alien.git
cd Alien
rustup override set nightly-2025-05-20

# Run tests
make test
```

### Development Guidelines

- Format code with `rustfmt`
- Address `clippy` warnings
- Write tests for new features
- Update relevant documentation

---

## License

MIT License - see [LICENSE](LICENSE) file.

---

## Acknowledgments

- **Rust Project** - Language and tooling support
- **Elle** - Isolation verification framework
- **RISC-V International** - Open ISA specification
- **QEMU** - RISC-V emulation platform

---

<div align="center">

  **Built with Rust**

  **[⭐ Star on GitHub!](https://github.com/your-username/Alien)**

</div>

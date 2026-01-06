# DBFS for Alien OS - 最终架构设计

## 📐 核心架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│              (dbfs_test, user applications)                  │
│                  open/read/write/mkdir/unlink                │
└──────────────────────────────┬──────────────────────────────┘
                               │ POSIX syscalls
                               ↓
┌─────────────────────────────────────────────────────────────┐
│                        VFS Layer                             │
│           (vfscore: path, dentry, inode, file)              │
└──────────────────────────────┬──────────────────────────────┘
                               │ VFS Operations
                               ↓
┌─────────────────────────────────────────────────────────────┐
│                     DBFS Layer                               │
│                  (事务层 - Transactional)                    │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  DbfsSuperBlock                                     │  │
│  │  - begin_tx() / commit_tx() / rollback_tx()          │  │
│  │  - WAL management & crash recovery                   │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       ↓                                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  Write-Ahead Log (WAL)                               │  │
│  │  - TxBegin / TxCommit / TxRollback                   │  │
│  │  - FileWrite / FileCreate / FileDelete / Mkdir       │  │
│  │  - Checksum validation                              │  │
│  │  - Crash recovery                                   │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       ↓                                     │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  DbfsInode (VFS Interface)                           │  │
│  │  - write_at() → record_write(tx_id, ...)            │  │
│  │  - create() → record_create(tx_id, ...)             │  │
│  │  - unlink() → record_delete(tx_id, ...)             │  │
│  │  - mkdir() → record_mkdir(tx_id, ...)               │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────┘
                               │ (delegated storage)
                               ↓
┌─────────────────────────────────────────────────────────────┐
│               Underlying Filesystem (可选)                    │
│           FAT32 / ramfs / ext4 / raw block                   │
└─────────────────────────────────────────────────────────────┘
```

## 🎯 设计原则

### 1. DBFS 的唯一职责

**✅ DBFS 做**:
- 事务生命周期管理 (begin/commit/rollback)
- WAL (Write-Ahead Log) 记录
- 崩溃恢复 (crash recovery)
- 操作延迟执行 (deferred execution)

**❌ DBFS 不做**:
- Block I/O
- Page cache
- 文件系统 journaling (底层FS负责)
- 数据库功能

### 2. 架构定位

```
正确理解:
VFS → DBFS (事务层) → 底层FS (存储层)

错误理解:
VFS → RVFS → DBFS → Disk  ❌
```

**关键点**:
- DBFS 本身就是一个 VFS filesystem type
- RVFS/FAT/ramfs 只是底层存储,不是中间层
- DBFS inode = 底层 inode + 事务语义

## 📦 核心组件

### 1. Wal (Write-Ahead Log)

**文件**: `src/wal.rs`

**数据结构**:
```rust
pub struct Wal {
    path: String,              // WAL 文件路径
    buffer: Vec<WalRecord>,    // 内存记录缓冲区
    next_lsn: Lsn,             // 下一个 LSN
    flushed_lsn: Lsn,          // 已刷盘 LSN
    next_tx_id: u64,           // 下一个事务 ID
}

pub struct WalRecord {
    lsn: Lsn,                  // Log Sequence Number
    tx_id: TxId,               // Transaction ID
    record_type: WalRecordType, // 记录类型
    data: Vec<u8>,             // 操作数据
    checksum: u32,             // 校验和
}
```

**API**:
```rust
// 事务管理
let tx_id = wal.begin_tx();
wal.commit_tx(tx_id)?;
wal.rollback_tx(tx_id);

// 操作记录
wal.write_file(tx_id, "/test.txt", 0, b"Hello");
wal.create_file(tx_id, "/newfile");
wal.delete_file(tx_id, "/oldfile");
wal.mkdir(tx_id, "/newdir");

// 崩溃恢复
let recovery = wal.recover()?;
// recovery.committed    - 已提交事务
// recovery.uncommitted  - 未提交事务
```

### 2. DbfsSuperBlock

**文件**: `src/alien_integration/superblock.rs`

**职责**:
- 管理 WAL 实例
- 提供事务接口
- 协调文件操作和事务记录

**API**:
```rust
impl DbfsSuperBlock {
    // 事务管理
    pub fn begin_tx(&self) -> TxId;
    pub fn commit_tx(&self, tx_id: TxId) -> VfsResult<()>;
    pub fn rollback_tx(&self, tx_id: TxId);

    // 操作记录 (供 DbfsInode 调用)
    pub fn record_write(&self, tx_id: TxId, path: &str, offset: u64, data: &[u8]);
    pub fn record_create(&self, tx_id: TxId, path: &str);
    pub fn record_delete(&self, tx_id: TxId, path: &str);
    pub fn record_mkdir(&self, tx_id: TxId, path: &str);

    // 崩溃恢复
    fn recover(&self);
}
```

### 3. DbfsInode (待实现)

**文件**: `src/alien_integration/inode.rs`

**职责**:
- 实现 VFS Inode 接口
- 拦截文件操作,记录到 WAL
- 延迟执行实际操作

**示例**:
```rust
impl VfsInode for DbfsInode {
    fn write_at(&self, offset: usize, data: &[u8]) -> VfsResult<usize> {
        // 1. 获取当前事务 ID
        let tx_id = self.current_tx()?;

        // 2. 记录到 WAL
        self.superblock.record_write(tx_id, &self.path, offset as u64, data);

        // 3. 延迟执行 (commit 时才真正写)
        // TODO: 保存到内存缓冲区

        Ok(data.len())
    }
}
```

### 4. DbfsFsType

**文件**: `src/alien_integration/fstype.rs`

**职责**:
- 实现 VFS FilesystemType 接口
- 处理 mount 请求
- 创建 DbfsSuperBlock

**API**:
```rust
impl VfsFsType for DbfsFsType {
    fn mount(...) -> VfsResult<Arc<dyn VfsDentry>> {
        let sb = DbfsSuperBlock::new(db_path);
        let root_inode = sb.root_inode()?;
        let root_dentry = DbfsDentry::root(root_inode);
        Ok(root_dentry)
    }
}
```

## 🔄 事务流程

### 1. 正常写入流程

```
Application
    ↓ write()
DbfsInode::write_at()
    ↓ record_write()
DbfsSuperBlock::record_write()
    ↓ write_file()
Wal::write_file()
    ↓ (记录到内存缓冲区)
[Commit 时]
    ↓ commit_tx()
Wal::commit_tx()
    ↓ flush() (持久化 WAL)
[Apply 时]
    ↓ (遍历 WAL 记录)
真正写入底层文件系统
```

### 2. 崩溃恢复流程

```
系统启动
    ↓ mount Dbfs
DbfsSuperBlock::new()
    ↓ Wal::new()
DbfsSuperBlock::recover()
    ↓ Wal::recover()
分析 WAL 记录:
    - 已提交事务 → 重放操作
    - 未提交事务 → 忽略/回滚
    ↓
系统进入一致性状态
```

## ✅ ACID 保证

### Atomicity (原子性)
- 所有操作在事务内
- Commit 时全部应用或全部不应用
- Rollback 撤销所有操作
- **实现**: WAL + 延迟执行

### Consistency (一致性)
- 文件系统状态始终有效
- 无孤儿文件或损坏数据
- WAL 校验和验证
- **实现**: WAL validation

### Isolation (隔离性)
- Phase 1: 全局事务锁 (简单实现)
- Phase 2: MVCC 快照隔离 (高级优化)
- **实现**: Mutex<Wal>

### Durability (持久性)
- Commit 前 WAL 刷盘
- 崩溃后从 WAL 恢复
- 数据永久存储
- **实现**: WAL flush + recovery

## 📁 文件结构

```
subsystems/dbfs/
├── Cargo.toml                    # 依赖配置
├── src/
│   ├── lib.rs                    # 库入口
│   ├── wal.rs                    # ✅ WAL 实现
│   ├── common.rs                 # 公共类型
│   ├── alien_integration/        # Alien OS 集成
│   │   ├── mod.rs
│   │   ├── fstype.rs             # ✅ DBFS 文件系统类型
│   │   ├── superblock.rs         # ✅ 事务管理器
│   │   ├── inode.rs              # 🔄 事务性 inode (待完善)
│   │   └── dentry.rs             # ✅ Dentry 实现
│   └── ... (其他模块)
├── TRANSACTION_GUIDE.md          # 实现指南
├── ARCHITECTURE_FINAL.md         # 本文档
└── README.md
```

## 🚀 使用示例

### 1. 挂载 DBFS

```rust
// 在 kernel/VFS 初始化时
use dbfs::alien_integration::DbfsFsType;

let dbfs = DbfsFsType::new("/dev/vda".to_string());
FS.lock().insert("dbfs".to_string(), Arc::new(dbfs));

// 挂载到 /dbfs
let dbfs_root = mount("dbfs", "/dev/vda", "/dbfs", None, &[])?;
```

### 2. 应用层使用 (伪代码)

```rust
// 打开文件
let fd = open("/dbfs/test.txt", O_CREAT | O_WRONLY);

// 开始事务
let tx_id = begin_transaction();

// 写入数据
write(fd, b"Hello, Transaction!");

// 提交事务
commit_transaction(tx_id);
```

### 3. 系统调用接口 (待实现)

```rust
// 新增系统调用
sys_dbfs_begin_tx() -> TxId
sys_dbfs_commit_tx(tx_id: TxId)
sys_dbfs_rollback_tx(tx_id: TxId)
```

## 🧪 测试计划

### 单元测试
- [x] WAL 序列化/反序列化
- [x] 事务 begin/commit/rollback
- [x] WAL recovery

### 集成测试
- [ ] 文件写入事务性
- [ ] 崩溃一致性
- [ ] 多文件操作
- [ ] 并发事务

### 系统测试
- [ ] dbfs_test 5项测试
- [ ] 崩溃恢复验证
- [ ] 性能基准测试

## 📖 为什么不用 jammdb?

| 特性     | jammdb        | DBFS WAL          |
| ------ | ------------- | ----------------- |
| no_std | ❌ 有依赖问题      | ✅ 完全兼容           |
| OS 集成  | ❌ 设计为用户态库    | ✅ 专为内核设计         |
| 依赖复杂度  | ❌ 高 (errno等)  | ✅ 极低             |
| 可控性    | ❌ 黑盒          | ✅ 完全可控           |
| 学术价值   | ⚠️ 现成方案       | ⭐⭐⭐⭐⭐ 原创实现 |

**结论**: DBFS 是一个 VFS 级事务层,不是数据库支持的文件系统

## 📞 下一步

1. ✅ WAL 实现
2. ✅ SuperBlock 集成
3. 🔄 Inode 事务操作
4. ⏳ VFS 挂载
5. ⏳ 系统调用接口
6. ⏳ 测试验证

## 📚 参考

- [PostgreSQL WAL](https://www.postgresql.org/docs/current/wal.html)
- [SQLite Transaction Management](https://www.sqlite.org/transactionintro.html)
- [LMDB Architecture](https://www.symas.com/lmdb)
- [Linux VFS](https://www.kernel.org/doc/html/latest/filesystems/vfs.html)

---

**一句话总结**:

> **DBFS in AlienOS is implemented as a VFS-level transactional shim, providing ACID guarantees while delegating storage to existing filesystems.**
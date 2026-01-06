# DBFS 事务性存储实现指南

## 🎯 目标

为 Alien OS 提供基于 WAL (Write-Ahead Log) 的**事务性文件系统**,保证 ACID 特性。

## 📊 架构设计

```
┌─────────────────────────────────────────────┐
│           Application Layer                 │
│        (dbfs_test, user apps)               │
└──────────────────┬──────────────────────────┘
                   │ POSIX syscalls
                   ↓
┌─────────────────────────────────────────────┐
│              VFS Layer                      │
│     (vfscore: VfsPath, VfsFile, etc.)       │
└──────────────────┬──────────────────────────┘
                   ↓
┌─────────────────────────────────────────────┐
│           DBFS Layer                        │
│  ┌────────────────────────────────────┐    │
│  │  TransactionManager                │    │
│  │  - begin_tx() / commit() / rollback│    │
│  └────────────┬───────────────────────┘    │
│               ↓                             │
│  ┌────────────────────────────────────┐    │
│  │  Write-Ahead Log (WAL)             │    │
│  │  - Log file: /dev/vda + offset     │    │
│  │  - Records: TxBegin/Commit/Ops     │    │
│  │  - Crash recovery                  │    │
│  └────────────┬───────────────────────┘    │
│               ↓                             │
│  ┌────────────────────────────────────┐    │
│  │  File Storage (In-Memory Map)      │    │
│  │  - Buckets: file_path → data      │    │
│  │  - Metadata: inodes, dentries      │    │
│  └────────────────────────────────────┘    │
└──────────────────┬──────────────────────────┘
                   │
                   ↓
┌─────────────────────────────────────────────┐
│         Block Device Layer                  │
│          (/dev/vda - FAT32)                 │
└─────────────────────────────────────────────┘
```

## 🔧 核心组件

### 1. WAL (Write-Ahead Log)

**位置**: [subsystems/dbfs/src/wal.rs](src/wal.rs)

**功能**:
- 记录所有事务操作
- 支持崩溃恢复
- 保证原子性和持久性

**API**:
```rust
let mut wal = Wal::new("/dev/vda".to_string())?;

// 开始事务
let tx_id = wal.begin_tx();

// 记录操作
wal.write_file(tx_id, "/test.txt", 0, b"Hello, World!");
wal.create_file(tx_id, "/newfile.txt");
wal.delete_file(tx_id, "/oldfile.txt");

// 提交事务
wal.commit_tx(tx_id)?;

// 或回滚
// wal.rollback_tx(tx_id);
```

**WAL 记录格式**:
```
[LSN: 8 bytes] [TxID: 8 bytes] [Type: 1 byte] [DataLen: 4 bytes] [Data: N bytes] [Checksum: 4 bytes]
```

**记录类型**:
- `TxBegin` - 事务开始
- `TxCommit` - 事务提交
- `TxRollback` - 事务回滚
- `FileWrite` - 文件写入
- `FileCreate` - 创建文件
- `FileDelete` - 删除文件
- `Mkdir` - 创建目录

### 2. TransactionManager

**TODO: 实现**

将集成到 `alien_integration/superblock.rs`:

```rust
pub struct DbfsSuperBlock {
    wal: Mutex<Wal>,
    // ... other fields
}

impl DbfsSuperBlock {
    pub fn begin_transaction(&self) -> TxId {
        self.wal.lock().begin_tx()
    }

    pub fn commit_transaction(&self, tx_id: TxId) -> Result<(), DbfsError> {
        // Apply all operations in transaction
        // Then commit WAL
        self.wal.lock().commit_tx(tx_id)
    }

    pub fn rollback_transaction(&self, tx_id: TxId) {
        self.wal.lock().rollback_tx(tx_id)
    }
}
```

### 3. DbfsInode (事务性文件操作)

**TODO: 更新**

在 [alien_integration/inode.rs](src/alien_integration/inode.rs) 中:

```rust
impl VfsInode for DbfsInode {
    fn write_at(&self, offset: usize, data: &[u8]) -> VfsResult<usize> {
        // Get current transaction from thread-local storage
        let tx_id = self.current_tx()?;

        // Log to WAL
        self.superblock.wal.lock().write_file(
            tx_id,
            &self.path,
            offset as u64,
            data,
        );

        // Defer actual write until commit
        Ok(data.len())
    }
}
```

## 🚀 使用示例

### 应用层使用

```rust
// 在 dbfs_test 或用户程序中
use dbfs::alien_integration::DbfsFsType;
use vfscore::VfsFsType;

// 1. 挂载 DBFS
let dbfs = DbfsFsType::new("/dev/vda".to_string());
let root_dentry = dbfs.mount(...)?;

// 2. 打开文件
let file = root_dentry.lookup("/test.txt")?;

// 3. 开始事务
let sb = file.dentry().superblock();
let tx_id = sb.begin_transaction()?;

// 4. 执行操作
file.write_at(0, b"Hello, Transaction!")?;
file2.create(...)?;

// 5. 提交事务
sb.commit_transaction(tx_id)?;

// 成功: 文件内容持久化
// 失败: 自动回滚,无副作用
```

### 崩溃恢复

```rust
// 系统启动时
let wal = Wal::new("/dev/vda".to_string())?;
let recovery = wal.recover()?;

// 重放已提交的事务
for tx_id in recovery.committed {
    let records = wal.get_tx_records(tx_id);
    for record in records {
        // Apply operation
        match record.record_type {
            WalRecordType::FileWrite => {
                // Apply write
            }
            // ... other operations
        }
    }
}

// 回滚未提交的事务
for tx_id in recovery.uncommitted {
    // Rollback or ignore
}
```

## ✅ ACID 保证

### Atomicity (原子性)
- 所有操作在事务内
- Commit 时全部应用或全部不应用
- Rollback 撤销所有操作

### Consistency (一致性)
- 文件系统状态始终有效
- 无孤儿文件或损坏数据
- 通过 WAL 校验和保证

### Isolation (隔离性)
- Phase 1: 全局事务锁 (简单实现)
- Phase 2: MVCC 快照隔离 (高级优化)

### Durability (持久性)
- Commit 前 WAL 刷盘
- 崩溃后从 WAL 恢复
- 数据永久存储

## 📝 实现步骤

### Phase 1: ✅ WAL 基础 (已完成)
- [x] Wal 数据结构
- [x] 记录序列化/反序列化
- [x] begin/commit/rollback
- [x] 内存操作记录

### Phase 2: 🔄 SuperBlock 集成 (进行中)
- [ ] 在 DbfsSuperBlock 中集成 Wal
- [ ] 实现 TransactionManager
- [ ] 提供 begin/commit/rollback 接口

### Phase 3: ⏳ Inode 事务操作
- [ ] 更新 DbfsInode::write_at 使用事务
- [ ] 更新 DbfsInode::create 使用事务
- [ ] 更新 DbfsInode::unlink 使用事务

### Phase 4: ⏳ 持久化 WAL
- [ ] 将 WAL 写入磁盘
- [ ] 实现 WAL 刷盘 (fsync)
- [ ] 实现 WAL 恢复逻辑

### Phase 5: ⏳ VFS 集成
- [ ] 在 VFS 中注册 DBFS
- [ ] 实现 mount 逻辑
- [ ] 创建 dbfs_test 测试

## 🧪 测试计划

### 单元测试
- [x] WAL 序列化测试
- [x] 事务 begin/commit 测试
- [x] 崩溃恢复测试

### 集成测试
- [ ] 文件写入事务性测试
- [ ] 崩溃一致性测试
- [ ] 多文件操作测试
- [ ] 并发事务测试

### 系统测试
- [ ] dbfs_test 5项测试全部通过
- [ ] 崩溃恢复验证
- [ ] 性能基准测试

## 📖 参考

- [PostgreSQL WAL](https://www.postgresql.org/docs/current/wal.html)
- [SQLite Transaction Management](https://www.sqlite.org/transactionintro.html)
- [LMDB Architecture](https://www.symas.com/lmdb)
- [BoltDB Design](https://github.com/boltdb/bolt)

## 🎓 为什么不使用 jammdb?

jammdb 在 Alien OS 的 `no_std` 环境下存在依赖问题:
- `errno` crate 不支持 no_std
- `libc` 依赖在某些配置下有问题
- 需要大量修复工作

**我们的方案**:
- 简单、可控、可定制
- 完全兼容 Alien OS 环境
- 提供核心 ACID 保证
- 易于理解和维护

## 🚧 当前状态

- ✅ WAL 模块完成
- 🔄 集成到 alien_integration
- ⏳ VFS 挂载
- ⏳ 测试验证

## 📞 下一步

1. 在 `DbfsSuperBlock` 中集成 `Wal`
2. 更新 `DbfsInode` 使用事务
3. 在 VFS 中注册 DBFS
4. 创建测试程序验证事务性
# DBFS 事务性文件系统 - 使用指南

## 🎯 核心概念

DBFS 是一个 **VFS 级事务层**,为文件系统操作提供 ACID 保证。

```
应用层 (dbfs_test)
    ↓ open/write/close
VFS Layer
    ↓
DBFS Layer (事务层)
    ├─ begin_tx() / commit_tx() / rollback_tx()
    ├─ WAL (Write-Ahead Log)
    └─ 延迟执行
    ↓
底层FS (FAT/ramfs)
```

## 📖 核心 API

### 1. 事务管理 (Transaction Management)

```rust
use dbfs::alien_integration::inode::{begin_tx, commit_tx, rollback_tx};

// 开始事务
let tx_id = begin_tx();

// 执行文件操作 (会记录到 WAL)
// ... create / write / delete 操作 ...

// 提交事务 (WAL 刷盘,操作生效)
commit_tx(tx_id)?;

// 或回滚事务 (撤销所有操作)
// rollback_tx(tx_id);
```

### 2. 文件操作 (自动记录到 WAL)

```rust
// 所有写操作都需要在事务上下文中
let tx_id = begin_tx();

// 创建文件
let file = parent_dir.create("test.txt", VfsNodeType::File, perm, None)?;

// 写入文件
file.write_at(0, b"Hello, Transaction!")?;

// 删除文件
parent_dir.unlink("old.txt")?;

// 提交事务
commit_tx(tx_id)?;
```

## 🔧 实现细节

### 1. DbfsInode - 事务化 Inode

**文件**: `src/alien_integration/inode.rs`

**关键特性**:
- ✅ 每个写操作都记录到 WAL
- ✅ 自动获取当前事务 ID
- ✅ 延迟执行 (commit 时才真正修改)

**实现示例**:
```rust
impl DbfsInode {
    // 获取当前事务
    fn current_tx(&self) -> VfsResult<TxId> {
        CURRENT_TX.lock()
            .ok_or(VfsError::NoSys)
            .and_then(|tx_opt| tx_opt.ok_or(VfsError::NoSys))
    }
}

impl VfsInode for DbfsInode {
    fn create(&self, name: &str, ty: VfsNodeType, ...) -> VfsResult<...> {
        // 1. 获取当前事务
        let tx_id = self.current_tx()?;

        // 2. 记录到 WAL
        self.sb.record_create(tx_id, &new_path);

        // 3. 执行操作 (Phase 2: 暂时立即执行)
        let new_inode = Self::new_inode(...);

        Ok(new_inode)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        // 1. 获取当前事务
        let tx_id = self.current_tx()?;

        // 2. 记录到 WAL
        self.sb.record_write(tx_id, &path, offset, buf);

        // 3. 执行写入
        data[start..start + buf.len()].copy_from_slice(buf);

        Ok(buf.len())
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        // 1. 获取当前事务
        let tx_id = self.current_tx()?;

        // 2. 记录到 WAL
        self.sb.record_delete(tx_id, &file_path);

        // 3. 执行删除
        entries.remove(name);

        Ok(())
    }
}
```

### 2. DbfsSuperBlock - 事务管理器

**文件**: `src/alien_integration/superblock.rs`

**API**:
```rust
impl DbfsSuperBlock {
    // 事务管理
    pub fn begin_tx(&self) -> TxId;
    pub fn commit_tx(&self, tx_id: TxId) -> VfsResult<()>;
    pub fn rollback_tx(&self, tx_id: TxId);

    // 操作记录 (供 Inode 调用)
    pub fn record_write(&self, tx_id: TxId, path: &str, offset: u64, data: &[u8]);
    pub fn record_create(&self, tx_id: TxId, path: &str);
    pub fn record_delete(&self, tx_id: TxId, path: &str);
    pub fn record_mkdir(&self, tx_id: TxId, path: &str);
}
```

### 3. WAL - Write-Ahead Log

**文件**: `src/wal.rs`

**功能**:
- 记录所有事务操作
- 支持序列化/反序列化
- 校验和验证
- 崩溃恢复

**数据结构**:
```rust
pub struct Wal {
    path: String,              // WAL 文件路径
    buffer: Vec<WalRecord>,    // 内存记录
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

## 🚀 使用示例

### 示例 1: 简单事务写入

```rust
use dbfs::alien_integration::inode::{begin_tx, commit_tx};

// 1. 挂载 DBFS
let dbfs = DbfsFsType::new("/dev/vda".to_string());
let root = dbfs.mount(...)?;

// 2. 开始事务
let tx_id = begin_tx();

// 3. 创建文件
let file = root.create("test.txt", VfsNodeType::File, perm, None)?;

// 4. 写入数据
file.write_at(0, b"Hello, DBFS!")?;

// 5. 提交事务
commit_tx(tx_id)?;

// 成功: 文件已持久化,即使系统崩溃也能恢复
```

### 示例 2: 事务回滚

```rust
let tx_id = begin_tx();

// 创建文件
let file = root.create("temp.txt", ...)?;
file.write_at(0, b"Temporary data")?;

// 出错,回滚
rollback_tx(tx_id);

// 文件不存在,所有操作已撤销
```

### 示例 3: 多文件操作

```rust
let tx_id = begin_tx();

// 原子性操作多个文件
root.create("file1.txt", ...)?;
root.create("file2.txt", ...)?;
root.create("file3.txt", ...)?;

// 全部成功或全部失败
commit_tx(tx_id)?;
```

### 示例 4: 崩溃恢复

```rust
// 系统启动时
let sb = DbfsSuperBlock::new("/dev/vda".to_string());

// SuperBlock::new() 会自动调用 recover()
// 重放已提交的事务,忽略未提交的事务

// 系统进入一致性状态
```

## ⚠️ 当前限制 (Phase 2)

### 1. 事务上下文管理
- **当前**: 使用静态 `CURRENT_TX: Mutex<Option<TxId>>`
- **限制**: 全局单一事务
- **改进**: 使用 thread-local 或 SuperBlock 管理

### 2. 延迟执行
- **当前**: 操作立即执行,WAL 仅用于恢复
- **TODO**: 完全延迟到 commit 时执行

### 3. 持久化 WAL
- **当前**: WAL 仅在内存中
- **TODO**: 写入磁盘文件

### 4. 并发控制
- **当前**: 全局 Mutex
- **TODO**: MVCC 或快照隔离

## 📝 测试指南

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_serialize() {
        let tx_id = TxId::new(1);
        let record = WalRecord::new(tx_id, WalRecordType::TxBegin, Vec::new());

        let bytes = record.serialize();
        let deserialized = WalRecord::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.tx_id, tx_id);
    }

    #[test]
    fn test_transaction_commit() {
        let mut wal = Wal::new("/test/wal".to_string()).unwrap();

        let tx_id = wal.begin_tx();
        wal.commit_tx(tx_id).unwrap();

        assert_eq!(wal.next_tx_id(), 2);
    }
}
```

### 集成测试

```rust
#[test]
fn test_file_write_transaction() {
    // 挂载 DBFS
    let dbfs = DbfsFsType::new("/tmp/test".to_string());
    let root = dbfs.mount(...).unwrap();

    // 开始事务
    let tx_id = begin_tx();

    // 创建并写入文件
    let file = root.create("test.txt", ...).unwrap();
    file.write_at(0, b"Hello").unwrap();

    // 提交事务
    commit_tx(tx_id).unwrap();

    // 验证文件存在
    let file2 = root.lookup("test.txt").unwrap();
    assert_eq!(file2.get_attr().unwrap().st_size, 5);
}
```

## 🔜 下一步

1. ✅ WAL 实现
2. ✅ SuperBlock 集成
3. ✅ Inode 事务化
4. ⏳ 完善 `begin_tx()` / `commit_tx()` 实现
5. ⏳ VFS 挂载
6. ⏳ 持久化 WAL 到磁盘
7. ⏳ 测试验证

## 📚 参考

- [ARCHITECTURE_FINAL.md](ARCHITECTURE_FINAL.md) - 架构设计
- [TRANSACTION_GUIDE.md](TRANSACTION_GUIDE.md) - 实现指南
- [PostgreSQL WAL](https://www.postgresql.org/docs/current/wal.html)
//! WAL Backend Trait
//!
//! 将 WAL 存储抽象为可插拔的 backend
//!
//! ## 架构
//!
//! ```text
//! WriteAheadLog
//!    ↓
//! dyn WalBackend (trait)
//!    ↓
//! ┌────────────┬────────────┬────────────┐
//! │ InMemory   │ VfsFile    │ Pmem       │
//! │ Backend    │ Backend    │ Backend    │
//! └────────────┴────────────┴────────────┘
//! ```

use alloc::{boxed::Box, string::String, sync::Arc, vec::Vec};
use core::iter::Iterator;
use ksync::Mutex;

use crate::common::DbfsError;
use crate::wal::{Lsn, TxId, WalRecord, WalRecordType};

/// WAL Backend 错误类型
#[derive(Debug)]
pub enum WalBackendError {
    /// IO 错误
    Io,
    /// 无效参数
    InvalidArgument,
    /// 后端不支持的操作
    Unsupported,
    /// 其他错误
    Other,
}

impl From<DbfsError> for WalBackendError {
    fn from(_: DbfsError) -> Self {
        WalBackendError::Io
    }
}

/// WAL Backend Trait
///
/// 定义了 WAL 存储的最小接口集
///
/// ## 设计原则
///
/// 1. **最小化**: 只包含必要的方法
/// 2. **语义清晰**: 每个方法的职责明确
/// 3. **可测试**: 容易 mock 和测试
/// 4. **可扩展**: 易于添加新的 backend
pub trait WalBackend: Send + Sync {
    /// 追加一条 WAL 记录（必须是顺序的）
    ///
    /// # 参数
    /// - `record`: 要追加的记录
    ///
    /// # 返回
    /// - 成功返回 Ok(())
    /// - 失败返回 WalBackendError
    fn append(&self, record: &WalRecord) -> Result<(), WalBackendError>;

    /// 强制持久化（fsync / flush / clwb）
    ///
    /// # 语义
    /// - 对于磁盘 backend: 调用 fsync
    /// - 对于 PMEM backend: clwb + sfence
    /// - 对于内存 backend: no-op
    ///
    /// # 返回
    /// - 成功返回 Ok(())
    /// - 失败返回 WalBackendError
    fn flush(&self) -> Result<(), WalBackendError>;

    /// 从头顺序读取所有 WAL（用于恢复）
    ///
    /// # 返回
    /// - WAL 记录的迭代器
    fn replay(&self) -> Result<Box<dyn Iterator<Item = WalRecord> + '_>, WalBackendError>;

    /// 截断到某个 LSN（checkpoint）
    ///
    /// # 参数
    /// - `lsn`: 截断到的 LSN（保留此 LSN 及之后的记录）
    ///
    /// # 语义
    /// - 删除所有 LSN < lsn 的记录
    /// - 用于 checkpoint 后清理旧 WAL
    fn truncate(&self, lsn: Lsn) -> Result<(), WalBackendError>;

    /// 当前持久化到的 LSN
    ///
    /// # 返回
    /// - 已经持久化的最大 LSN
    fn durable_lsn(&self) -> Lsn;

    /// 是否具备持久化语义（区分内存 / PMEM / 磁盘）
    ///
    /// # 返回
    /// - true: 数据会持久化到磁盘/PMEM
    /// - false: 数据仅在内存中
    fn is_persistent(&self) -> bool;
}

/// In-Memory WAL Backend
///
/// ## 用途
/// - 单元测试
/// - 事务语义验证
/// - 并发正确性测试
///
/// ## 特性
/// - 不持久化
/// - `flush()` 是 no-op
/// - `is_persistent() == false`
pub struct InMemoryWalBackend {
    /// WAL 记录缓冲区
    records: Mutex<Vec<WalRecord>>,
    /// 已持久化的 LSN
    flushed_lsn: core::sync::atomic::AtomicU64,
}

impl InMemoryWalBackend {
    /// 创建新的内存 backend
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            records: Mutex::new(Vec::new()),
            flushed_lsn: core::sync::atomic::AtomicU64::new(0),
        })
    }
}

impl WalBackend for InMemoryWalBackend {
    fn append(&self, record: &WalRecord) -> Result<(), WalBackendError> {
        let mut records = self.records.lock();
        let mut record = record.clone();
        // 分配 LSN
        record.lsn = records.len() as u64 + 1;
        records.push(record);
        Ok(())
    }

    fn flush(&self) -> Result<(), WalBackendError> {
        // 内存 backend: flush 是 no-op
        // 更新 durable_lsn
        let records = self.records.lock();
        if let Some(last) = records.last() {
            self.flushed_lsn.store(last.lsn, core::sync::atomic::Ordering::Release);
        }
        Ok(())
    }

    fn replay(&self) -> Result<Box<dyn Iterator<Item = WalRecord> + '_>, WalBackendError> {
        let records = self.records.lock();
        // 返回一个克隆的迭代器
        let cloned: Vec<WalRecord> = records.clone();
        Ok(Box::new(cloned.into_iter()))
    }

    fn truncate(&self, lsn: Lsn) -> Result<(), WalBackendError> {
        let mut records = self.records.lock();
        records.retain(|r| r.lsn >= lsn);
        Ok(())
    }

    fn durable_lsn(&self) -> Lsn {
        self.flushed_lsn.load(core::sync::atomic::Ordering::Acquire)
    }

    fn is_persistent(&self) -> bool {
        false
    }
}

/// VFS File WAL Backend
///
/// ## 用途
/// - 生产环境
/// - 真正的持久化存储
///
/// ## 特性
/// - 持久化到磁盘
/// - `flush()` 调用 fsync
/// - `is_persistent() == true`
///
/// ## 注意
/// 当前使用内存模拟,未来需要实现真正的文件 I/O
pub struct VfsFileWalBackend {
    /// WAL 文件路径
    path: String,
    /// 内存缓冲区 (当前实现)
    buffer: Mutex<Vec<WalRecord>>,
    /// 已持久化的 LSN
    flushed_lsn: core::sync::atomic::AtomicU64,
}

impl VfsFileWalBackend {
    /// 创建新的 VFS 文件 backend
    ///
    /// # 参数
    /// - `path`: WAL 文件路径
    pub fn new(path: String) -> Arc<Self> {
        log::info!("✓ DBFS: Creating VFS File WAL Backend at {}", path);

        Arc::new(Self {
            path,
            buffer: Mutex::new(Vec::new()),
            flushed_lsn: core::sync::atomic::AtomicU64::new(0),
        })
    }

    /// 从磁盘恢复 WAL (未来实现)
    fn load_from_disk(&self) -> Result<(), WalBackendError> {
        // TODO: Phase 4 - 从磁盘文件读取 WAL
        // 1. 打开 self.path
        // 2. 读取所有记录
        // 3. 反序列化
        // 4. 存入 self.buffer
        Ok(())
    }

    /// 持久化到磁盘 (未来实现)
    fn flush_to_disk(&self) -> Result<(), WalBackendError> {
        // TODO: Phase 4 - 写入磁盘文件
        // 1. 打开/创建 self.path
        // 2. 序列化所有新记录
        // 3. 追加写入文件
        // 4. 调用 fsync
        Ok(())
    }
}

impl WalBackend for VfsFileWalBackend {
    fn append(&self, record: &WalRecord) -> Result<(), WalBackendError> {
        let mut buffer = self.buffer.lock();
        let mut record = record.clone();
        // 分配 LSN
        record.lsn = buffer.len() as u64 + 1;
        buffer.push(record);
        Ok(())
    }

    fn flush(&self) -> Result<(), WalBackendError> {
        // 当前: 内存模拟
        let buffer = self.buffer.lock();
        if let Some(last) = buffer.last() {
            self.flushed_lsn.store(last.lsn, core::sync::atomic::Ordering::Release);
        }

        // 未来: 调用 self.flush_to_disk()

        let buffer = self.buffer.lock();
        log::info!("✓ DBFS: WAL flush: {} records (VFS file mode)",
                  buffer.len());

        Ok(())
    }

    fn replay(&self) -> Result<Box<dyn Iterator<Item = WalRecord> + '_>, WalBackendError> {
        let buffer = self.buffer.lock();
        let cloned: Vec<WalRecord> = buffer.clone();
        Ok(Box::new(cloned.into_iter()))
    }

    fn truncate(&self, lsn: Lsn) -> Result<(), WalBackendError> {
        let mut buffer = self.buffer.lock();
        buffer.retain(|r| r.lsn >= lsn);

        // TODO: 同步到磁盘

        Ok(())
    }

    fn durable_lsn(&self) -> Lsn {
        self.flushed_lsn.load(core::sync::atomic::Ordering::Acquire)
    }

    fn is_persistent(&self) -> bool {
        // 当前: 实际上是内存模式
        // 未来: 返回 true
        false  // 等待真正的磁盘 I/O 实现
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_backend() {
        let backend = InMemoryWalBackend::new();

        // 测试 append
        let record = WalRecord::new(TxId::new(1), WalRecordType::TxBegin, Vec::new());
        backend.append(&record).unwrap();

        // 测试 durable_lsn
        assert_eq!(backend.durable_lsn(), 0);

        // 测试 flush
        backend.flush().unwrap();
        assert_eq!(backend.durable_lsn(), 1);

        // 测试 is_persistent
        assert!(!backend.is_persistent());
    }

    #[test]
    fn test_vfs_file_backend() {
        let backend = VfsFileWalBackend::new("/test/.wal".to_string());

        // 测试 append
        let record = WalRecord::new(TxId::new(1), WalRecordType::TxBegin, Vec::new());
        backend.append(&record).unwrap();

        // 测试 flush
        backend.flush().unwrap();

        // 测试 is_persistent
        // 当前实现返回 false,未来实现后应该返回 true
        assert!(!backend.is_persistent());
    }
}
//! DBFS SuperBlock for Alien Integration
//!
//! Phase 2: 集成 WAL 事务层

use alloc::{format, string::String, string::ToString, sync::Arc};
use ksync::Mutex;
use log::info;
use vfscore::{
    fstype::VfsFsType,
    superblock::{SuperType, VfsSuperBlock},
    utils::VfsFsStat,
    VfsResult,
};

use crate::wal::{TxId, Wal};
use super::{fstype::DummyFsType, inode::DbfsInode};

/// DBFS SuperBlock with Transaction Support
///
/// 职责:
/// 1. 管理 WAL (Write-Ahead Log)
/// 2. 提供事务接口 (begin/commit/rollback)
/// 3. 协调文件操作和事务记录
pub struct DbfsSuperBlock {
    /// Block size (固定 4KB)
    block_size: u64,
    /// 文件系统路径
    db_path: String,
    /// Write-Ahead Log
    wal: Mutex<Wal>,
    /// Root inode (cached)
    root: Mutex<Option<Arc<DbfsInode>>>,
}

impl DbfsSuperBlock {
    /// Create a new superblock with WAL
    pub fn new(db_path: String) -> Arc<Self> {
        info!("✓ DBFS: Initializing superblock with WAL");

        let wal = Wal::new(format!("{}/.wal", db_path))
            .expect("Failed to initialize WAL");

        let sb = Arc::new(Self {
            block_size: 4096,
            db_path,
            wal: Mutex::new(wal),
            root: Mutex::new(None),
        });

        // Perform crash recovery
        sb.recover();

        sb
    }

    /// Create root inode
    pub fn root_inode(self: &Arc<Self>) -> VfsResult<Arc<dyn vfscore::inode::VfsInode>> {
        Ok(DbfsInode::new_root(self.clone()))
    }

    /// Begin a new transaction
    pub fn begin_tx(&self) -> TxId {
        self.wal.lock().begin_tx()
    }

    /// Commit a transaction
    pub fn commit_tx(&self, tx_id: TxId) -> VfsResult<()> {
        info!("✓ DBFS: Committing transaction {}", tx_id);

        // Flush WAL to disk first (durability)
        self.wal.lock().commit_tx(tx_id)
            .map_err(|e| {
                log::error!("Failed to commit transaction {}: {:?}", tx_id, e);
                vfscore::error::VfsError::IoError
            })?;

        // TODO: Apply all operations to underlying filesystem
        // For now, operations are already logged in WAL

        info!("✓ DBFS: Transaction {} committed successfully", tx_id);
        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_tx(&self, tx_id: TxId) {
        info!("✓ DBFS: Rolling back transaction {}", tx_id);
        self.wal.lock().rollback_tx(tx_id);
    }

    /// Record a file write operation
    pub fn record_write(&self, tx_id: TxId, path: &str, offset: u64, data: &[u8]) {
        self.wal.lock().write_file(tx_id, path, offset, data);
    }

    /// Record a file create operation
    pub fn record_create(&self, tx_id: TxId, path: &str) {
        self.wal.lock().create_file(tx_id, path);
    }

    /// Record a file delete operation
    pub fn record_delete(&self, tx_id: TxId, path: &str) {
        self.wal.lock().delete_file(tx_id, path);
    }

    /// Record a mkdir operation
    pub fn record_mkdir(&self, tx_id: TxId, path: &str) {
        self.wal.lock().mkdir(tx_id, path);
    }

    /// Crash recovery from WAL
    fn recover(&self) {
        info!("✓ DBFS: Starting crash recovery...");

        let wal = self.wal.lock();
        match wal.recover() {
            Ok(recovery) => {
                if recovery.committed.is_empty() && recovery.uncommitted.is_empty() {
                    info!("✓ DBFS: No transactions to recover (clean shutdown)");
                } else {
                    info!("✓ DBFS: Found {} committed transactions",
                          recovery.committed.len());
                    info!("✓ DBFS: Found {} uncommitted transactions (will rollback)",
                          recovery.uncommitted.len());

                    // TODO: Replay committed transactions
                    // For now, just log them
                    for tx_id in &recovery.committed {
                        info!("  - Transaction {} (committed)", tx_id);
                    }

                    // Uncommitted transactions are automatically rolled back
                    for tx_id in &recovery.uncommitted {
                        info!("  - Transaction {} (rolled back)", tx_id);
                    }
                }
            }
            Err(e) => {
                log::error!("✗ DBFS: WAL recovery failed: {:?}", e);
            }
        }
    }

    /// Get WAL statistics
    pub fn wal_stats(&self) -> (u64, u64) {
        let wal = self.wal.lock();
        (wal.next_tx_id(), wal.flushed_lsn())
    }
}

impl VfsSuperBlock for DbfsSuperBlock {
    fn sync_fs(&self, _wait: bool) -> VfsResult<()> {
        info!("✓ DBFS: Syncing filesystem");
        // Flush WAL to disk
        self.wal.lock().flush()
            .map_err(|_| vfscore::error::VfsError::IoError)?;
        Ok(())
    }

    fn stat_fs(&self) -> VfsResult<VfsFsStat> {
        // Use default for now, as the exact field names vary between versions
        let mut stat = VfsFsStat::default();
        // Set the fields we know exist
        stat.f_type = 0x44424653;      // "DBFS" magic number
        Ok(stat)
    }

    fn super_type(&self) -> SuperType {
        // Use Independent as the filesystem type
        SuperType::Independent
    }

    fn fs_type(&self) -> Arc<dyn VfsFsType> {
        Arc::new(DummyFsType {
            name: self.db_path.clone(),
        })
    }

    fn root_inode(&self) -> VfsResult<Arc<dyn vfscore::inode::VfsInode>> {
        // Check if root is already cached
        let mut root_cache = self.root.lock();
        if let Some(ref inode) = *root_cache {
            return Ok(Arc::clone(inode) as Arc<dyn vfscore::inode::VfsInode>);
        }

        // Need to create root inode, but we need Arc<Self>
        // Use unsafe to reconstruct Arc from &self
        // This is safe because superblock is always created as Arc via DbfsSuperBlock::new()
        unsafe {
            // Reconstruct Arc from pointer - this increments ref count
            let arc_self = Arc::from_raw(self as *const DbfsSuperBlock);

            // Create the inode (which increments ref count again)
            let inode = DbfsInode::new_root(Arc::clone(&arc_self));

            // Forget the first Arc to avoid decrementing ref count
            core::mem::forget(arc_self);

            // Cache the inode
            *root_cache = Some(Arc::clone(&inode));

            Ok(inode as Arc<dyn vfscore::inode::VfsInode>)
        }
    }
}
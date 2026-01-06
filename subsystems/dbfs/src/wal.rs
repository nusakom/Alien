//! Write-Ahead Log for DBFS
//!
//! 简单的 WAL 实现,提供事务性保证
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │        WAL File Structure           │
//! ├─────────────────────────────────────┤
//! │ Header (512 bytes)                 │
//! │  - magic: "DBFSWAL"                │
//! │  - version: u32                    │
//! │  - last_tx_id: u64                 │
//! │  - checkpoint_lsn: u64             │
//! ├─────────────────────────────────────┤
//! │ Log Records (variable)             │
//! │  [LSN | TxID | Type | Data | CRC]  │
//! ├─────────────────────────────────────┤
//! │ Checkpoint (periodic)              │
//! └─────────────────────────────────────┘
//! ```

#![allow(unused)]
use alloc::{format, string::String, vec::Vec};
use core::fmt;

use crate::common::DbfsError;

/// WAL Magic Number
const WAL_MAGIC: &[u8; 8] = b"DBFSWAL\0";

/// WAL Header (fixed size: 512 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalHeader {
    /// Magic number for validation
    pub magic: [u8; 8],
    /// WAL format version
    pub version: u32,
    /// Last committed transaction ID
    pub last_tx_id: u64,
    /// Checkpoint LSN (Log Sequence Number)
    pub checkpoint_lsn: u64,
    /// Reserved space
    _reserved: [u8; 492],
}

impl Default for WalHeader {
    fn default() -> Self {
        Self {
            magic: *WAL_MAGIC,
            version: 1,
            last_tx_id: 0,
            checkpoint_lsn: 0,
            _reserved: [0; 492],
        }
    }
}

/// Log Sequence Number - unique identifier for each log record
pub type Lsn = u64;

/// Transaction ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxId(u64);

impl TxId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for TxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TX-{}", self.0)
    }
}

/// WAL Record Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WalRecordType {
    /// Transaction begin
    TxBegin = 1,
    /// Transaction commit
    TxCommit = 2,
    /// Transaction rollback
    TxRollback = 3,
    /// File write operation
    FileWrite = 4,
    /// File create operation
    FileCreate = 5,
    /// File delete operation
    FileDelete = 6,
    /// Directory create
    Mkdir = 7,
    /// Checkpoint marker
    Checkpoint = 8,
}

/// WAL Record
#[derive(Debug, Clone)]
pub struct WalRecord {
    /// Log Sequence Number
    pub lsn: Lsn,
    /// Transaction ID
    pub tx_id: TxId,
    /// Record type
    pub record_type: WalRecordType,
    /// Record data (path, file data, etc.)
    pub data: Vec<u8>,
    /// Checksum for validation
    pub checksum: u32,
}

impl WalRecord {
    /// Create a new WAL record
    pub fn new(tx_id: TxId, record_type: WalRecordType, data: Vec<u8>) -> Self {
        let checksum = Self::compute_checksum(&data);

        Self {
            lsn: 0, // Will be assigned when written
            tx_id,
            record_type,
            data,
            checksum,
        }
    }

    /// Compute checksum for data
    fn compute_checksum(data: &[u8]) -> u32 {
        // Simple CRC32-style checksum
        let mut crc: u32 = 0xFFFFFFFF;
        for byte in data {
            crc ^= *byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    /// Verify checksum
    pub fn verify(&self) -> bool {
        Self::compute_checksum(&self.data) == self.checksum
    }

    /// Serialize record to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // LSN (8 bytes)
        bytes.extend_from_slice(&self.lsn.to_be_bytes());

        // TxID (8 bytes)
        bytes.extend_from_slice(&self.tx_id.value().to_be_bytes());

        // Record type (1 byte)
        bytes.push(self.record_type as u8);

        // Data length (4 bytes)
        bytes.extend_from_slice(&(self.data.len() as u32).to_be_bytes());

        // Data
        bytes.extend_from_slice(&self.data);

        // Checksum (4 bytes)
        bytes.extend_from_slice(&self.checksum.to_be_bytes());

        bytes
    }

    /// Deserialize record from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, DbfsError> {
        if bytes.len() < 25 {
            // 8 + 8 + 1 + 4 + 4 (minimum header + checksum)
            return Err(DbfsError::InvalidArgument);
        }

        let lsn = u64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let tx_id = TxId::new(u64::from_be_bytes(bytes[8..16].try_into().unwrap()));
        let record_type = match bytes[16] {
            1 => WalRecordType::TxBegin,
            2 => WalRecordType::TxCommit,
            3 => WalRecordType::TxRollback,
            4 => WalRecordType::FileWrite,
            5 => WalRecordType::FileCreate,
            6 => WalRecordType::FileDelete,
            7 => WalRecordType::Mkdir,
            8 => WalRecordType::Checkpoint,
            _ => return Err(DbfsError::InvalidArgument),
        };

        let data_len = u32::from_be_bytes(bytes[17..21].try_into().unwrap()) as usize;
        let data = bytes[21..21 + data_len].to_vec();
        let checksum = u32::from_be_bytes(bytes[21 + data_len..25 + data_len].try_into().unwrap());

        let record = Self {
            lsn,
            tx_id,
            record_type,
            data,
            checksum,
        };

        if !record.verify() {
            return Err(DbfsError::Io);
        }

        Ok(record)
    }
}

/// Write-Ahead Log
pub struct Wal {
    /// WAL file/device path
    path: String,
    /// In-memory buffer for records
    buffer: Vec<WalRecord>,
    /// Next LSN to assign
    next_lsn: Lsn,
    /// Last flushed LSN
    flushed_lsn: Lsn,
    /// Current transaction ID
    next_tx_id: u64,
}

impl Wal {
    /// Create a new WAL
    pub fn new(path: String) -> Result<Self, DbfsError> {
        Ok(Self {
            path,
            buffer: Vec::new(),
            next_lsn: 1,
            flushed_lsn: 0,
            next_tx_id: 1,
        })
    }

    /// Begin a new transaction
    pub fn begin_tx(&mut self) -> TxId {
        let tx_id = TxId::new(self.next_tx_id);
        self.next_tx_id += 1;

        let record = WalRecord::new(tx_id, WalRecordType::TxBegin, Vec::new());
        self.append_record(record);

        tx_id
    }

    /// Commit a transaction
    pub fn commit_tx(&mut self, tx_id: TxId) -> Result<(), DbfsError> {
        let record = WalRecord::new(tx_id, WalRecordType::TxCommit, Vec::new());
        self.append_record(record);
        self.flush()?;
        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_tx(&mut self, tx_id: TxId) {
        let record = WalRecord::new(tx_id, WalRecordType::TxRollback, Vec::new());
        self.append_record(record);
    }

    /// Write a file operation
    pub fn write_file(&mut self, tx_id: TxId, path: &str, offset: u64, data: &[u8]) {
        let mut record_data = Vec::new();

        // Path length (2 bytes) + path
        let path_bytes = path.as_bytes();
        record_data.extend_from_slice(&(path_bytes.len() as u16).to_be_bytes());
        record_data.extend_from_slice(path_bytes);

        // Offset (8 bytes)
        record_data.extend_from_slice(&offset.to_be_bytes());

        // Data length (4 bytes) + data
        record_data.extend_from_slice(&(data.len() as u32).to_be_bytes());
        record_data.extend_from_slice(data);

        let record = WalRecord::new(tx_id, WalRecordType::FileWrite, record_data);
        self.append_record(record);
    }

    /// Create file operation
    pub fn create_file(&mut self, tx_id: TxId, path: &str) {
        let record_data = path.as_bytes().to_vec();
        let record = WalRecord::new(tx_id, WalRecordType::FileCreate, record_data);
        self.append_record(record);
    }

    /// Delete file operation
    pub fn delete_file(&mut self, tx_id: TxId, path: &str) {
        let record_data = path.as_bytes().to_vec();
        let record = WalRecord::new(tx_id, WalRecordType::FileDelete, record_data);
        self.append_record(record);
    }

    /// Create directory operation
    pub fn mkdir(&mut self, tx_id: TxId, path: &str) {
        let record_data = path.as_bytes().to_vec();
        let record = WalRecord::new(tx_id, WalRecordType::Mkdir, record_data);
        self.append_record(record);
    }

    /// Append a record to the WAL
    fn append_record(&mut self, mut record: WalRecord) {
        record.lsn = self.next_lsn;
        self.next_lsn += 1;
        self.buffer.push(record);
    }

    /// Flush WAL to disk
    pub fn flush(&mut self) -> Result<(), DbfsError> {
        if !self.buffer.is_empty() {
            // Phase 3: Persistent WAL - Write to actual disk file

            // Find records that need to be flushed (those after flushed_lsn)
            let records_to_flush: Vec<_> = self.buffer
                .iter()
                .filter(|r| r.lsn > self.flushed_lsn)
                .collect();

            if !records_to_flush.is_empty() {
                // Serialize all new records
                let mut wal_data = Vec::new();

                // Write WAL header
                let header = WalHeader {
                    magic: *WAL_MAGIC,
                    version: 1,
                    last_tx_id: self.next_tx_id,
                    checkpoint_lsn: self.flushed_lsn,
                    _reserved: [0; 492],
                };

                // Serialize header (512 bytes)
                wal_data.extend_from_slice(unsafe {
                    core::slice::from_raw_parts(
                        &header as *const _ as *const u8,
                        core::mem::size_of::<WalHeader>()
                    )
                });

                // Serialize each record
                for record in &records_to_flush {
                    wal_data.extend_from_slice(&record.serialize());
                }

                // TODO: Write to actual file system
                // For now, we'll store in memory and mark as flushed
                // In a real implementation, we would:
                // 1. Open/create WAL file at self.path
                // 2. Seek to end (append mode)
                // 3. Write wal_data
                // 4. Call fsync to ensure durability
                // 5. Update flushed_lsn

                log::info!("✓ DBFS: WAL flush: {} records, {} bytes (in-memory mode)",
                          records_to_flush.len(), wal_data.len());
            }

            // Update flushed_lsn
            self.flushed_lsn = self.buffer.last().unwrap().lsn;
        }
        Ok(())
    }

    /// Recover transactions from WAL
    pub fn recover(&self) -> Result<RecoveryResult, DbfsError> {
        let mut committed = Vec::new();
        let mut uncommitted = Vec::new();
        let mut current_tx = None;

        // Phase 3: Persistent WAL recovery
        // In a real implementation, we would:
        // 1. Check if WAL file exists at self.path
        // 2. Read WAL header (512 bytes)
        // 3. Validate magic number
        // 4. Read all records from file
        // 5. Deserialize and validate checksums
        // 6. Identify committed vs uncommitted transactions
        // 7. Return recovery result for replay

        // For now, use in-memory buffer
        log::info!("✓ DBFS: WAL recovery from {} in-memory records",
                  self.buffer.len());

        for record in &self.buffer {
            match record.record_type {
                WalRecordType::TxBegin => {
                    current_tx = Some(record.tx_id);
                }
                WalRecordType::TxCommit => {
                    if let Some(tx_id) = current_tx {
                        if tx_id == record.tx_id {
                            committed.push(tx_id);
                            current_tx = None;
                        }
                    }
                }
                WalRecordType::TxRollback => {
                    if let Some(tx_id) = current_tx {
                        if tx_id == record.tx_id {
                            current_tx = None;
                        }
                    }
                }
                _ => {
                    // Operation records - ignore for recovery state
                }
            }
        }

        // Any transaction that began but didn't commit/rollback is uncommitted
        if let Some(tx_id) = current_tx {
            uncommitted.push(tx_id);
        }

        log::info!("✓ DBFS: Recovery complete: {} committed, {} uncommitted",
                  committed.len(), uncommitted.len());

        Ok(RecoveryResult {
            committed,
            uncommitted,
        })
    }

    /// Get all records for a transaction
    pub fn get_tx_records(&self, tx_id: TxId) -> Vec<&WalRecord> {
        self.buffer
            .iter()
            .filter(|r| r.tx_id == tx_id)
            .collect()
    }

    /// Truncate WAL (remove old records)
    pub fn truncate(&mut self, lsn: Lsn) {
        self.buffer.retain(|r| r.lsn >= lsn);
    }

    /// Get next transaction ID
    pub fn next_tx_id(&self) -> u64 {
        self.next_tx_id
    }

    /// Get flushed LSN
    pub fn flushed_lsn(&self) -> Lsn {
        self.flushed_lsn
    }
}

/// WAL Recovery Result
pub struct RecoveryResult {
    /// Committed transaction IDs
    pub committed: Vec<TxId>,
    /// Uncommitted transaction IDs (need rollback)
    pub uncommitted: Vec<TxId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_record_serialize() {
        let tx_id = TxId::new(1);
        let record = WalRecord::new(tx_id, WalRecordType::TxBegin, Vec::new());

        let bytes = record.serialize();
        let deserialized = WalRecord::deserialize(&bytes).unwrap();

        assert_eq!(deserialized.tx_id, tx_id);
        assert_eq!(deserialized.record_type, WalRecordType::TxBegin);
    }

    #[test]
    fn test_wal_begin_commit() {
        let mut wal = Wal::new("/test/wal".to_string()).unwrap();

        let tx_id = wal.begin_tx();
        wal.commit_tx(tx_id).unwrap();

        assert_eq!(wal.next_tx_id(), 2);
    }

    #[test]
    fn test_wal_recovery() {
        let mut wal = Wal::new("/test/wal".to_string()).unwrap();

        let tx1 = wal.begin_tx();
        wal.commit_tx(tx1).unwrap();

        let tx2 = wal.begin_tx();
        wal.commit_tx(tx2).unwrap();

        let result = wal.recover().unwrap();
        assert_eq!(result.committed.len(), 2);
        assert_eq!(result.uncommitted.len(), 0);
    }
}
extern crate alloc;
use alloc::vec::Vec;
use ksync::Mutex; 
use crate::layout::{WalEntry, WalOp, Serializer, Superblock};
use jammdb::DB; 


// Rough plan:
// - Keep a buffer of pending logs
// - On commit/sync, write to disk area
// - For now, we interact with the `DB` trait, but ideally we should talk to `BlockDevice`
// - Refactoring plan: `DB` trait implementation in `jammdb` will eventually use this `WAL` to persist to `BlockDevice`.

pub struct WalManager {
    // In memory buffer
    buffer: Mutex<Vec<u8>>,
    // Pointer to current log head on disk (offset in bytes from log_start)
    log_head: Mutex<u64>, 
    superblock: Superblock,
}

impl WalManager {
    pub fn new(sb: Superblock) -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            log_head: Mutex::new(0),
            superblock: sb,
        }
    }

    pub fn append(&self, tx_id: u64, op: WalOp, key: &[u8], value: &[u8]) {
        let entry = WalEntry {
            tx_id,
            op,
            key: key.to_vec(),
            value: value.to_vec(),
            checksum: 0, 
        };
        let mut buf = self.buffer.lock();
        buf.extend_from_slice(&entry.serialize());
    }

    // This would be called by the commit path
    // For this step, we just return the buffer to be written
    pub fn flush_buffer(&self) -> Vec<u8> {
        let mut buf = self.buffer.lock();
        let data = buf.clone();
        buf.clear();
        data
    }
}

use alloc::vec::Vec;
#[cfg(target_os = "none")]
use ksync::Mutex;
#[cfg(not(target_os = "none"))]
use spin::Mutex;
use crate::layout::{WalEntry, WalOp, Serializer, Superblock};
use device_interface::BlockDevice;
use jammdb::DB;

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

    pub fn sync(&self, device: &dyn BlockDevice) -> Result<(), ()> {
        let mut buf = self.buffer.lock();
        if buf.is_empty() { return Ok(()); }

        let mut log_offset = self.log_head.lock();
        let start_block = self.superblock.log_start_block as usize;
        let disk_offset = (start_block * 512) + *log_offset as usize;

        // Write buffer to device
        device.write(&buf, disk_offset).map_err(|_| ())?;
        
        // Update log head
        *log_offset += buf.len() as u64;
        buf.clear();
        Ok(())
    }

    pub fn recover(&self, device: &dyn BlockDevice, db: &DB) {
        // Read log from start
        let start_block = self.superblock.log_start_block as usize;
        let max_len = (self.superblock.log_len_blocks * 512) as usize;
        
        let mut buf = vec![0u8; max_len];
        if device.read(&mut buf, start_block * 512).is_err() {
            return;
        }

        let mut offset = 0;
        let mut valid_head = 0;

        // Iterate entries
        while offset < buf.len() {
             if let Some(entry) = WalEntry::deserialize(&buf[offset..]) {
                 let entry_len = entry.serialize().len(); 
                 
                 match entry.op {
                     WalOp::Put => {
                         let mut tx = db.tx();
                         tx.put(&entry.key, &entry.value).ok();
                         tx.commit().ok();
                     },
                     WalOp::Delete => {
                     },
                     WalOp::Commit => {
                     }
                 }
                 
                 offset += entry_len;
                 valid_head += entry_len as u64;
             } else {
                 break; // End of valid log
             }
        }
        
        *self.log_head.lock() = valid_head;
    }
}

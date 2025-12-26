#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
#[cfg(target_os = "none")]
use ksync::Mutex;
#[cfg(not(target_os = "none"))]
use spin::Mutex;

use core::task::{Context, Poll};

pub struct DB {
    data: Arc<Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

pub struct Transaction<'a> {
    db: &'a DB,
    pending_writes: BTreeMap<Vec<u8>, Vec<u8>>,
    pending_deletes: Vec<Vec<u8>>,
}

impl DB {
    pub fn open<O, P>(_mmap: O, _path: P) -> Result<Self, ()> {
        Ok(DB {
            data: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let lock = self.data.lock();
        lock.get(key).cloned()
    }

    pub fn tx(&self) -> Transaction {
        Transaction {
            db: self,
            pending_writes: BTreeMap::new(),
            pending_deletes: Vec::new(),
        }
    }
}

impl<'a> Transaction<'a> {
    pub fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), ()> {
        self.pending_writes.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    pub fn delete(&mut self, key: &[u8]) -> Result<(), ()> {
        self.pending_deletes.push(key.to_vec());
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(val) = self.pending_writes.get(key) {
            return Some(val.clone());
        }
        if self.pending_deletes.iter().any(|k| k == key) {
            return None;
        }
        self.db.get(key)
    }

    pub fn commit(self) -> Result<(), ()> {
        let mut lock = self.db.data.lock();
        for (key, value) in self.pending_writes {
            lock.insert(key, value);
        }
        for key in self.pending_deletes {
            lock.remove(&key);
        }
        Ok(())
    }

    pub async fn commit_async(self) -> Result<(), ()> {
        // In this simple implementation, sync commit is fine, 
        // but we wrap it in async for the API completion.
        self.commit()
    }
}

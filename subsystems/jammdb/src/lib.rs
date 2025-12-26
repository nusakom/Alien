#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use ksync::Mutex;

pub struct DB {
    data: Arc<Mutex<BTreeMap<Vec<u8>, Vec<u8>>>>,
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

    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), ()> {
        let mut lock = self.data.lock();
        lock.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    pub fn delete(&self, key: &[u8]) -> Result<(), ()> {
        let mut lock = self.data.lock();
        lock.remove(key);
        Ok(())
    }
}

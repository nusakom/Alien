#[cfg(test)]
mod tests {
    use crate::Dbfs;
    use crate::common::*;
    use jammdb::DB;
    
    #[cfg(not(target_os = "none"))]
    use spin::Mutex as TestMutex;
    #[cfg(target_os = "none")]
    use ksync::Mutex as TestMutex;

    use alloc::sync::Arc;
    use core::sync::atomic::Ordering;

    struct MockBlockDevice {
        data: TestMutex<alloc::vec::Vec<u8>>,
    }

    impl MockBlockDevice {
        fn new() -> Self {
            Self {
                data: TestMutex::new(alloc::vec![0u8; 10 * 1024 * 1024]), // 10MB mock disk
            }
        }
    }

    impl device_interface::DeviceBase for MockBlockDevice {
        fn handle_irq(&self) {}
    }

    impl device_interface::BlockDevice for MockBlockDevice {
        fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, constants::AlienError> {
            let data = self.data.lock();
            let end = offset + buf.len();
            if end > data.len() {
                return Err(constants::AlienError::EINVAL);
            }
            buf.copy_from_slice(&data[offset..end]);
            Ok(buf.len())
        }
        fn write(&self, buf: &[u8], offset: usize) -> Result<usize, constants::AlienError> {
            let mut data = self.data.lock();
            let end = offset + buf.len();
            if end > data.len() {
                return Err(constants::AlienError::EINVAL);
            }
            data[offset..end].copy_from_slice(buf);
            Ok(buf.len())
        }
        fn size(&self) -> usize {
            10 * 1024 * 1024
        }
        fn flush(&self) -> Result<(), constants::AlienError> {
            Ok(())
        }
    }

    #[test]
    fn test_dbfs_initialization() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        // Root inode should exist
        let attr = dbfs.get_attr(1).expect("Root inode should exist");
        assert_eq!(attr.ino, 1);
        assert_eq!(attr.kind, DbfsFileType::Directory);
    }

    #[test]
    fn test_dbfs_create_and_lookup() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        let time = DbfsTimeSpec::default();
        let perm = DbfsPermission::S_IFREG | DbfsPermission::from_bits_truncate(0o644);
        
        let attr = dbfs.create(1, "test.txt", 0, 0, time, perm).expect("Failed to create file");
        assert_eq!(attr.ino, 2);
        
        let lookup_attr = dbfs.lookup(1, "test.txt").expect("Failed to lookup file");
        assert_eq!(lookup_attr.ino, 2);
    }

    #[test]
    fn test_dbfs_read_write() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        let time = DbfsTimeSpec::default();
        let perm = DbfsPermission::S_IFREG | DbfsPermission::from_bits_truncate(0o644);
        let attr = dbfs.create(1, "hello.txt", 0, 0, time, perm).expect("Failed to create file");
        
        let content = b"Hello, Modern DBFS!";
        dbfs.write(attr.ino, content, 0).expect("Failed to write to file");
        
        let mut buf = [0u8; 20];
        let read_len = dbfs.read(attr.ino, &mut buf, 0).expect("Failed to read from file");
        assert_eq!(read_len, content.len());
        assert_eq!(&buf[..read_len], content);
    }

    #[tokio::test]
    async fn test_dbfs_async_ops() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        let time = DbfsTimeSpec::default();
        let perm = DbfsPermission::S_IFREG | DbfsPermission::from_bits_truncate(0o644);
        let attr = dbfs.create_async(1, "async.txt", 0, 0, time, perm).await.expect("Failed to create file async");
        
        let content = b"Async content";
        dbfs.write_async(attr.ino, content, 0).await.expect("Failed to write async");
        
        let mut buf = [0u8; 20];
        let read_len = dbfs.read_async(attr.ino, &mut buf, 0).await.expect("Failed to read async");
        assert_eq!(&buf[..read_len], content);
    }

    #[test]
    fn test_transaction_isolation() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        let mut tx1 = dbfs.db.tx();
        tx1.put(b"key1", b"val1_tx1").unwrap();
        
        // Parallel transaction should not see tx1's uncommitted change
        let tx2 = dbfs.db.tx();
        assert!(tx2.get(b"key1").is_none());
        
        tx1.commit().unwrap();
        
        // Now it should be visible in new transactions
        let tx3 = dbfs.db.tx();
        assert_eq!(tx3.get(b"key1").unwrap(), b"val1_tx1");
    }

    #[test]
    fn test_transaction_atomicity() {
        let db = DB::open((), ()).unwrap();
        let device = Arc::new(MockBlockDevice::new());
        let dbfs = Dbfs::new(db, device);
        
        let mut tx = dbfs.db.tx();
        tx.put(b"key_atomic", b"will_not_persist").unwrap();
        // tx is dropped here without commit
        drop(tx);
        
        let tx2 = dbfs.db.tx();
        assert!(tx2.get(b"key_atomic").is_none());
    }
}

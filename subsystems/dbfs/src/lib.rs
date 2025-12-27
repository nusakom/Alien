#![no_std]
#[macro_use]
extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
#[cfg(target_os = "none")]
use ksync::Mutex;
#[cfg(not(target_os = "none"))]
use spin::Mutex;
use jammdb::DB;
use core::sync::atomic::{AtomicUsize, Ordering};
use device_interface::BlockDevice;
use crate::layout::{Serializer, Superblock, MAGIC, WalOp};
use crate::common::*;
use crate::wal::WalManager;

pub mod layout;
pub mod wal;

pub struct Dbfs {
    db: Arc<DB>,
    device: Arc<dyn BlockDevice>,
    inode_counter: AtomicUsize,
    block_allocator: AtomicUsize,
    wal: Arc<WalManager>,
}

impl Dbfs {
    pub fn new(db: DB, device: Arc<dyn BlockDevice>) -> Arc<Self> {
        // Default SB for new Dbfs wrapper, will be overwritten if disk exists
        let sb = Superblock::new(1, 1, 1023, 1024);
        let wal = Arc::new(WalManager::new(sb));
        
        // This is a bit of a chicken-and-egg for WAL init if we don't know SB yet.
        // Simplified: Assume fixed layout or read first.

        let mut buf = [0u8; 512];
        let exists = if device.read(&mut buf, 0).is_ok() {
            &buf[0..8] == MAGIC
        } else { false };

        let dbfs = Arc::new(Self {
            db: Arc::new(db),
            device: device.clone(),
            inode_counter: AtomicUsize::new(2),
            block_allocator: AtomicUsize::new(1024),
            wal: wal.clone(),
        });

        if !exists {
                // Format disk
                log::info!("DBFS: Formatting disk...");
                let sb = Superblock::new(1, 1, 1023, 1024);
                let data = sb.serialize();
                let mut sector = [0u8; 512];
                sector[..data.len()].copy_from_slice(&data);
                device.write(&sector, 0).expect("Failed to write superblock");
                device.flush().ok();

                // Write Root Inode
                let root_attr = DbfsAttr {
                    ino: 1,
                    size: 0,
                    perm: 0o777,
                    nlink: 1,
                    uid: 0,
                    gid: 0,
                    atime: DbfsTimeSpec::default(),
                    mtime: DbfsTimeSpec::default(),
                    ctime: DbfsTimeSpec::default(),
                    kind: DbfsFileType::Directory,
                };
                let mut tx = dbfs.db.tx();
                tx.put(alloc::format!("i:1").as_bytes(), &root_attr.serialize()).ok();
                tx.commit().ok();
                
                dbfs.persist_inode(&root_attr);
                device.flush().ok();
        } else {
                log::info!("DBFS: Mounted existing filesystem. Recovering state...");
                // Recover from WAL first to replay recent transactions
                dbfs.wal.recover(device.as_ref(), &dbfs.db);
                // Then recover filesystem metadata recursively
                dbfs.recover_fs();
        }
        dbfs
    }

    fn recover_fs(&self) {
        log::info!("DBFS: Recovery started (recursive).");
        // Start recursive recovery from root inode (1)
        if let Err(_) = self.recover_dir(1) {
            log::error!("DBFS: Recursive recovery failed");
        }
    }

    // Recursive directory recovery
    fn recover_dir(&self, ino: usize) -> Result<(), DbfsError> {
        log::info!("DBFS: Recovering directory ino={}", ino);
        
        // 1. Recover inode metadata (block 2000 + ino)
        let mut inode_sector = [0u8; 512];
        if self.device.read(&mut inode_sector, (2000 + ino) * 512).is_err() {
            return Err(DbfsError::Io);
        }
        
        let attr = common::DbfsAttr::deserialize(&inode_sector)
            .ok_or(DbfsError::Io)?;
        
        let mut tx = self.db.tx();
        let key = alloc::format!("i:{}", ino);
        tx.put(key.as_bytes(), &attr.serialize()).ok();
        
        // Update inode counter if needed
        let cur = self.inode_counter.load(Ordering::SeqCst);
        if ino + 1 > cur {
            self.inode_counter.store(ino + 1, Ordering::SeqCst);
        }

        // 2. Recover directory entries - prefer DB (WAL) over disk
        let d_key = alloc::format!("d:{}", ino);
        let children = if let Some(db_data) = self.db.get(d_key.as_bytes()) {
            // WAL has already restored this directory, use that data
            log::info!("DBFS: Using WAL-recovered data for dir ino={}", ino);
            <Vec<file::DirEntry>>::deserialize(&db_data).unwrap_or_default()
        } else {
            // Read from disk (block 10000 + ino*100)
            let data_block = 10000 + (ino * 100);
            let mut dir_sector = [0u8; 512];
            if self.device.read(&mut dir_sector, data_block * 512).is_err() {
                tx.commit().ok();
                return Ok(()); // Not a directory or no data
            }
            
            if let Some(disk_children) = <Vec<file::DirEntry>>::deserialize(&dir_sector) {
                // Store in DB for future use
                tx.put(d_key.as_bytes(), &disk_children.serialize()).ok();
                disk_children
            } else {
                tx.commit().ok();
                return Ok(());
            }
        };
        
        tx.commit().ok();

        // Recurse into subdirectories and recover files
        for child in children {
                match child.kind {
                    DbfsFileType::Directory => {
                        // Recursive call for subdirectory
                        self.recover_dir(child.ino)?;
                    }
                    _ => {
                        // Recover file inode
                        let mut file_inode_sector = [0u8; 512];
                        if self.device.read(&mut file_inode_sector, (2000 + child.ino) * 512).is_ok() {
                            if let Some(file_attr) = common::DbfsAttr::deserialize(&file_inode_sector) {
                                let mut ftx = self.db.tx();
                                let fkey = alloc::format!("i:{}", child.ino);
                                ftx.put(fkey.as_bytes(), &file_attr.serialize()).ok();
                                
                                // Recover file data (first chunk)
                                let file_block = 10000 + (child.ino * 100);
                                let mut file_sector = [0u8; 512];
                                if self.device.read(&mut file_sector, file_block * 512).is_ok() {
                                    let size = core::cmp::min(file_attr.size as usize, 512);
                                    let f_key = alloc::format!("f:{}:0", child.ino);
                                    ftx.put(f_key.as_bytes(), &file_sector[..size]).ok();
                                }
                                ftx.commit().ok();
                            }
                        }
                    }
                }
            }
        
        Ok(())
    }

    fn persist_inode(&self, attr: &common::DbfsAttr) {
        let data = attr.serialize();
        let block_id = 2000 + attr.ino;
        let mut sector = [0u8; 512];
        if data.len() <= 512 {
            sector[..data.len()].copy_from_slice(&data);
            self.device.write(&sector, block_id * 512).ok();
        }
    }

    fn persist_file_chunk(&self, ino: usize, chunk_id: usize, data: &[u8]) {
        let block_id = 10000 + (ino * 100) + chunk_id;
        let mut sector = [0u8; 512];
        let write_len = core::cmp::min(data.len(), 512);
        sector[..write_len].copy_from_slice(&data[..write_len]);
        self.device.write(&sector, block_id * 512).ok();
    }

    pub fn read(&self, ino: usize, buf: &mut [u8], offset: u64) -> Result<usize, common::DbfsError> {
        let key = alloc::format!("f:{}:{}", ino, 0);
        if let Some(data) = self.db.get(key.as_bytes()) {
            let offset = offset as usize;
            if offset >= data.len() {
                return Ok(0);
            }
            let read_len = core::cmp::min(buf.len(), data.len() - offset);
            buf[..read_len].copy_from_slice(&data[offset..offset+read_len]);
            Ok(read_len)
        } else {
            Ok(0)
        }
    }

    pub async fn read_async(&self, ino: usize, buf: &mut [u8], offset: u64) -> Result<usize, common::DbfsError> {
        self.read(ino, buf, offset)
    }

    pub fn write(&self, ino: usize, buf: &[u8], offset: u64) -> Result<usize, common::DbfsError> {
        let mut tx = self.db.tx();
        let key = alloc::format!("f:{}:{}", ino, 0);
        let mut data = tx.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        
        let offset = offset as usize;
        let required_len = offset + buf.len();
        if required_len > data.len() {
            data.resize(required_len, 0);
        }
        data[offset..required_len].copy_from_slice(buf);
        tx.put(key.as_bytes(), &data).map_err(|_| common::DbfsError::Io)?;
        
        self.persist_file_chunk(ino, 0, &data);

        if let Ok(mut attr) = self.get_attr(ino) {
            attr.size = data.len() as u64;
            let attr_key = alloc::format!("i:{}", ino);
            let encoded = attr.serialize();
            tx.put(attr_key.as_bytes(), &encoded).ok();
            self.persist_inode(&attr);
        }
        
        tx.commit().map_err(|_| common::DbfsError::Io)?;
        self.device.flush().ok();
        Ok(buf.len())
    }

    pub async fn write_async(&self, ino: usize, buf: &[u8], offset: u64) -> Result<usize, common::DbfsError> {
        let mut tx = self.db.tx();
        let key = alloc::format!("f:{}:{}", ino, 0);
        let mut data = tx.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        
        let offset = offset as usize;
        let required_len = offset + buf.len();
        if required_len > data.len() {
            data.resize(required_len, 0);
        }
        data[offset..required_len].copy_from_slice(buf);
        tx.put(key.as_bytes(), &data).map_err(|_| common::DbfsError::Io)?;
        
        self.persist_file_chunk(ino, 0, &data);

        if let Ok(mut attr) = self.get_attr(ino) {
            attr.size = data.len() as u64;
            let attr_key = alloc::format!("i:{}", ino);
            let encoded = attr.serialize();
            tx.put(attr_key.as_bytes(), &encoded).ok();
            self.persist_inode(&attr);
        }
        
        tx.commit_async().await.map_err(|_| common::DbfsError::Io)?;
        self.device.flush().ok();
        Ok(buf.len())
    }

    pub fn readdir(&self, ino: usize, entries_out: &mut Vec<file::DirEntry>) -> Result<(), common::DbfsError> {
        let key = alloc::format!("d:{}", ino);
        if let Some(data) = self.db.get(key.as_bytes()) {
             let children: Vec<file::DirEntry> = <Vec<file::DirEntry>>::deserialize(&data).ok_or(common::DbfsError::Io)?;
             entries_out.extend(children);
        }
        Ok(())
    }

    pub async fn readdir_async(&self, ino: usize, entries_out: &mut Vec<file::DirEntry>) -> Result<(), common::DbfsError> {
        self.readdir(ino, entries_out)
    }

    pub fn lookup(&self, parent: usize, name: &str) -> Result<common::DbfsAttr, common::DbfsError> {
        let key = alloc::format!("d:{}", parent);
        if let Some(data) = self.db.get(key.as_bytes()) {
             let children: Vec<file::DirEntry> = <Vec<file::DirEntry>>::deserialize(&data).ok_or(common::DbfsError::Io)?;
             if let Some(child) = children.iter().find(|c| c.name == name) {
                 return self.get_attr(child.ino);
             }
        }
        Err(common::DbfsError::NotFound)
    }

    pub async fn lookup_async(&self, parent: usize, name: &str) -> Result<common::DbfsAttr, common::DbfsError> {
        self.lookup(parent, name)
    }

    pub fn get_attr(&self, ino: usize) -> Result<common::DbfsAttr, common::DbfsError> {
        let key = alloc::format!("i:{}", ino);
        if let Some(data) = self.db.get(key.as_bytes()) {
            common::DbfsAttr::deserialize(&data).ok_or(common::DbfsError::Io)
        } else {
             Err(common::DbfsError::NotFound)
        }
    }

    pub fn create(&self, parent: usize, name: &str, uid: u32, gid: u32, time: common::DbfsTimeSpec, perm: common::DbfsPermission) -> Result<common::DbfsAttr, common::DbfsError> {
        if self.lookup(parent, name).is_ok() {
            return Err(common::DbfsError::FileExists);
        }

        let mut tx = self.db.tx();
        let ino = self.inode_counter.fetch_add(1, Ordering::SeqCst);
        let kind = if perm.contains(common::DbfsPermission::S_IFDIR) { 
            common::DbfsFileType::Directory 
        } else { 
            common::DbfsFileType::RegularFile 
        };

        let attr = common::DbfsAttr {
            ino,
            size: 0,
            perm: perm.bits(),
            nlink: 1,
            uid,
            gid,
            atime: time,
            mtime: time,
            ctime: time,
            kind: kind.clone(),
        };

        let attr_key = alloc::format!("i:{}", ino);
        let encoded = attr.serialize();
        tx.put(attr_key.as_bytes(), &encoded).map_err(|_| common::DbfsError::Io)?;
        self.persist_inode(&attr);

        let parent_key = alloc::format!("d:{}", parent);
        let mut children: Vec<file::DirEntry> = if let Some(data) = tx.get(parent_key.as_bytes()) {
             <Vec<file::DirEntry>>::deserialize(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        children.push(file::DirEntry { ino, kind, name: String::from(name) });
        let encoded_children = children.serialize();
        tx.put(parent_key.as_bytes(), &encoded_children).map_err(|_| common::DbfsError::Io)?;
        
        self.persist_file_chunk(parent, 0, &encoded_children); 
        
        tx.commit().map_err(|_| common::DbfsError::Io)?;
        self.device.flush().ok();

        Ok(attr)
    }

    pub async fn create_async(&self, parent: usize, name: &str, uid: u32, gid: u32, time: common::DbfsTimeSpec, perm: common::DbfsPermission) -> Result<common::DbfsAttr, common::DbfsError> {
        if self.lookup(parent, name).is_ok() {
            return Err(common::DbfsError::FileExists);
        }

        let mut tx = self.db.tx();
        let ino = self.inode_counter.fetch_add(1, Ordering::SeqCst);
        let kind = if perm.contains(common::DbfsPermission::S_IFDIR) { 
            common::DbfsFileType::Directory 
        } else { 
            common::DbfsFileType::RegularFile 
        };

        let attr = common::DbfsAttr {
            ino,
            size: 0,
            perm: perm.bits(),
            nlink: 1,
            uid,
            gid,
            atime: time,
            mtime: time,
            ctime: time,
            kind: kind.clone(),
        };

        let attr_key = alloc::format!("i:{}", ino);
        let encoded = attr.serialize();
        tx.put(attr_key.as_bytes(), &encoded).map_err(|_| common::DbfsError::Io)?;
        self.persist_inode(&attr);

        let parent_key = alloc::format!("d:{}", parent);
        let mut children: Vec<file::DirEntry> = if let Some(data) = tx.get(parent_key.as_bytes()) {
             <Vec<file::DirEntry>>::deserialize(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        children.push(file::DirEntry { ino, kind, name: String::from(name) });
        let encoded_children = children.serialize();
        tx.put(parent_key.as_bytes(), &encoded_children).map_err(|_| common::DbfsError::Io)?;
        
        self.persist_file_chunk(parent, 0, &encoded_children); 
        
        tx.commit_async().await.map_err(|_| common::DbfsError::Io)?;
        self.device.flush().ok();

        Ok(attr)
    }

    pub fn truncate(&self, ino: usize, size: usize) -> Result<(), common::DbfsError> {
        let mut tx = self.db.tx();
        let key = alloc::format!("f:{}:{}", ino, 0);
        let mut data = tx.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        if data.len() != size {
            data.resize(size, 0);
            tx.put(key.as_bytes(), &data).map_err(|_| common::DbfsError::Io)?;
            self.persist_file_chunk(ino, 0, &data);
            
            if let Ok(mut attr) = self.get_attr(ino) {
                attr.size = size as u64;
                let attr_key = alloc::format!("i:{}", ino);
                let encoded = attr.serialize();
                tx.put(attr_key.as_bytes(), &encoded).ok();
                self.persist_inode(&attr);
            }
            tx.commit().map_err(|_| common::DbfsError::Io)?;
            self.device.flush().ok();
        }
        Ok(())
    }

    pub async fn truncate_async(&self, ino: usize, size: usize) -> Result<(), common::DbfsError> {
        let mut tx = self.db.tx();
        let key = alloc::format!("f:{}:{}", ino, 0);
        let mut data = tx.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        if data.len() != size {
            data.resize(size, 0);
            tx.put(key.as_bytes(), &data).map_err(|_| common::DbfsError::Io)?;
            self.persist_file_chunk(ino, 0, &data);
            
            if let Ok(mut attr) = self.get_attr(ino) {
                attr.size = size as u64;
                let attr_key = alloc::format!("i:{}", ino);
                let encoded = attr.serialize();
                tx.put(attr_key.as_bytes(), &encoded).ok();
                self.persist_inode(&attr);
            }
            tx.commit_async().await.map_err(|_| common::DbfsError::Io)?;
            self.device.flush().ok();
        }
        Ok(())
    }
    pub fn rename(&self, old_parent: usize, old_name: &str, new_parent: usize, new_name: &str) -> Result<(), common::DbfsError> {
        let mut tx = self.db.tx();
        
        // 1. Load Old Parent
        let old_p_key = alloc::format!("d:{}", old_parent);
        let old_data = tx.get(old_p_key.as_bytes()).ok_or(common::DbfsError::NotFound)?;
        let mut old_children = <Vec<file::DirEntry>>::deserialize(&old_data).ok_or(common::DbfsError::Io)?;
        
        // 2. Find and Remove Entry
        let idx = old_children.iter().position(|e| e.name == old_name).ok_or(common::DbfsError::NotFound)?;
        let entry = old_children.remove(idx);
        
        // 3. Update entry name
        let mut new_entry = entry.clone();
        new_entry.name = String::from(new_name);

        // 4. Handle Case: Single Directory Rename (Atomic by default if 1 block)
        if old_parent == new_parent {
            if old_children.iter().any(|e| e.name == new_name) {
                return Err(common::DbfsError::FileExists);
            }
            old_children.push(new_entry);
            let encoded = old_children.serialize();
            tx.put(old_p_key.as_bytes(), &encoded).map_err(|_| common::DbfsError::Io)?;
            self.persist_file_chunk(old_parent, 0, &encoded);
        } else {
            // 5. Handle Cross-Directory Rename (Secure WAL)
            
            let tx_id = 999; // Simple Transaction ID for proof-of-concept

            // A. Update Old Parent (Remove)
            let encoded_old = old_children.serialize();
            tx.put(old_p_key.as_bytes(), &encoded_old).map_err(|_| common::DbfsError::Io)?;
            self.wal.append(tx_id, WalOp::Put, old_p_key.as_bytes(), &encoded_old);
            
            // B. Load New Parent
            let new_p_key = alloc::format!("d:{}", new_parent);
            let mut new_children = if let Some(data) = tx.get(new_p_key.as_bytes()) {
                 <Vec<file::DirEntry>>::deserialize(&data).unwrap_or_default()
            } else {
                 Vec::new() 
            };
            
            if new_children.iter().any(|e| e.name == new_name) {
                 return Err(common::DbfsError::FileExists);
            }
            
            new_children.push(new_entry);
            let encoded_new = new_children.serialize();
            tx.put(new_p_key.as_bytes(), &encoded_new).map_err(|_| common::DbfsError::Io)?;
            self.wal.append(tx_id, WalOp::Put, new_p_key.as_bytes(), &encoded_new);
            
            // C. WAL Commit & Sync (The Atomic Point)
            self.wal.append(tx_id, WalOp::Commit, &[], &[]);
            self.wal.sync(self.device.as_ref()).map_err(|_| common::DbfsError::Io)?;

            // D. Checkpoint to Data Area (Lazy / Behind WAL)
            self.persist_file_chunk(old_parent, 0, &encoded_old);
            self.persist_file_chunk(new_parent, 0, &encoded_new);
        }
        
        tx.commit().map_err(|_| common::DbfsError::Io)?;
        // self.device.flush().ok(); // handled by sync
        Ok(())
    }
}

pub mod common {
    use super::*;
    use bitflags::bitflags;

    #[derive(Debug, Clone, Copy, Default)]
    pub struct DbfsTimeSpec {
        pub sec: i64,
        pub nsec: i32,
    }

    #[derive(Debug, Clone)]
    pub struct DbfsAttr {
        pub ino: usize,
        pub size: u64,
        pub perm: u16,
        pub nlink: u32,
        pub uid: u32,
        pub gid: u32,
        pub atime: DbfsTimeSpec,
        pub mtime: DbfsTimeSpec,
        pub ctime: DbfsTimeSpec,
        pub kind: DbfsFileType,
    }

    impl Serializer for DbfsAttr {
        fn serialize(&self) -> Vec<u8> {
            let mut data = Vec::new();
            data.extend_from_slice(&(self.ino as u64).to_le_bytes());
            data.extend_from_slice(&self.size.to_le_bytes());
            data.extend_from_slice(&self.perm.to_le_bytes());
            data.extend_from_slice(&self.nlink.to_le_bytes());
            data.extend_from_slice(&self.uid.to_le_bytes());
            data.extend_from_slice(&self.gid.to_le_bytes());
            
            data.extend_from_slice(&self.atime.sec.to_le_bytes());
            data.extend_from_slice(&self.atime.nsec.to_le_bytes());
            data.extend_from_slice(&self.mtime.sec.to_le_bytes());
            data.extend_from_slice(&self.mtime.nsec.to_le_bytes());
            data.extend_from_slice(&self.ctime.sec.to_le_bytes());
            data.extend_from_slice(&self.ctime.nsec.to_le_bytes());
            
            let kind_byte = match self.kind {
                DbfsFileType::Directory => 0,
                DbfsFileType::RegularFile => 1,
                DbfsFileType::Symlink => 2,
                DbfsFileType::CharDevice => 3,
                DbfsFileType::BlockDevice => 4,
                DbfsFileType::NamedPipe => 5,
                DbfsFileType::Socket => 6,
            };
            data.push(kind_byte);
            data
        }

        fn deserialize(data: &[u8]) -> Option<Self> {
            if data.len() < 67 { return None; }
            let mut offset = 0;
            
            let ino = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?) as usize; offset += 8;
            let size = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?); offset += 8;
            let perm = u16::from_le_bytes(data[offset..offset+2].try_into().ok()?); offset += 2;
            let nlink = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?); offset += 4;
            let uid = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?); offset += 4;
            let gid = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?); offset += 4;
            
            let atime = DbfsTimeSpec {
                sec: i64::from_le_bytes(data[offset..offset+8].try_into().ok()?),
                nsec: i32::from_le_bytes(data[offset+8..offset+12].try_into().ok()?),
            }; offset += 12;

            let mtime = DbfsTimeSpec {
                sec: i64::from_le_bytes(data[offset..offset+8].try_into().ok()?),
                nsec: i32::from_le_bytes(data[offset+8..offset+12].try_into().ok()?),
            }; offset += 12;

            let ctime = DbfsTimeSpec {
                sec: i64::from_le_bytes(data[offset..offset+8].try_into().ok()?),
                nsec: i32::from_le_bytes(data[offset+8..offset+12].try_into().ok()?),
            }; offset += 12;

            let kind = match data[offset] {
                0 => DbfsFileType::Directory,
                1 => DbfsFileType::RegularFile,
                2 => DbfsFileType::Symlink,
                3 => DbfsFileType::CharDevice,
                4 => DbfsFileType::BlockDevice,
                5 => DbfsFileType::NamedPipe,
                6 => DbfsFileType::Socket,
                _ => return None,
            };

            Some(DbfsAttr { ino, size, perm, nlink, uid, gid, atime, mtime, ctime, kind })
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum DbfsFileType {
        Directory,
        RegularFile,
        Symlink,
        CharDevice,
        BlockDevice,
        NamedPipe,
        Socket,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum DbfsError {
        PermissionDenied,
        NotFound,
        AccessError,
        FileExists,
        InvalidArgument,
        NoSpace,
        RangeError,
        NameTooLong,
        NoSys,
        NotEmpty,
        Io,
        NotSupported,
        NoData,
        Other,
    }

    bitflags! {
        pub struct DbfsPermission: u16 {
            const S_IFMT   = 0o170000;
            const S_IFSOCK = 0o140000;
            const S_IFLNK  = 0o120000;
            const S_IFREG  = 0o100000;
            const S_IFBLK  = 0o060000;
            const S_IFDIR  = 0o040000;
            const S_IFCHR  = 0o020000;
            const S_IFIFO  = 0o010000;
            const S_ISUID  = 0o004000;
            const S_ISGID  = 0o002000;
            const S_ISVTX  = 0o001000;
            const S_IRWXU  = 0o000700;
            const S_IRWXG  = 0o000070;
            const S_IRWXO  = 0o000007;
        }
    }
}

pub mod file {
    use super::*;
    use super::common::DbfsFileType;

    #[derive(Clone)]
    pub struct DirEntry {
        pub ino: usize,
        pub kind: DbfsFileType,
        pub name: String,
    }

    impl Serializer for Vec<DirEntry> {
        fn serialize(&self) -> Vec<u8> {
             let mut data = Vec::new();
             data.extend_from_slice(&(self.len() as u32).to_le_bytes());
             for entry in self {
                 data.extend_from_slice(&(entry.ino as u64).to_le_bytes());
                 let kind = match entry.kind {
                    DbfsFileType::Directory => 0,
                    DbfsFileType::RegularFile => 1,
                    _ => 1, 
                 };
                 data.push(kind);
                 let name_bytes = entry.name.as_bytes();
                 data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
                 data.extend_from_slice(name_bytes);
             }
             data
        }

        fn deserialize(data: &[u8]) -> Option<Self> {
             if data.len() < 4 { return Some(Vec::new()); }
             let mut offset = 0;
             let count = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?); offset += 4;
             let mut entries = Vec::with_capacity(count as usize);

             for _ in 0..count {
                 if offset + 13 > data.len() { break; }
                 let ino = u64::from_le_bytes(data[offset..offset+8].try_into().ok()?) as usize; offset += 8;
                 let kind_byte = data[offset]; offset += 1;
                 let kind = if kind_byte == 0 { DbfsFileType::Directory } else { DbfsFileType::RegularFile };
                 
                 let name_len = u32::from_le_bytes(data[offset..offset+4].try_into().ok()?) as usize; offset += 4;
                 if offset + name_len > data.len() { break; }
                 let name = String::from_utf8(data[offset..offset+name_len].to_vec()).ok()?; offset += name_len;
                 
                 entries.push(DirEntry { ino, kind, name });
             }
             Some(entries)
        }
    }
}

pub mod link {
    use super::common::{DbfsError, DbfsTimeSpec};
    pub fn dbfs_common_readlink(_ino: usize, _buf: &mut [u8]) -> Result<usize, DbfsError> {
        Err(DbfsError::NotSupported)
    }
    pub fn dbfs_common_unlink(_uid: u32, _gid: u32, _parent: usize, _name: &str, _target: Option<&str>, _time: DbfsTimeSpec) -> Result<(), DbfsError> {
        Ok(())
    }
}

pub mod fuse {
    #[cfg(feature = "fuse")]
    pub mod mkfs {
         pub struct FakeMMap;
         pub struct FakePath;
         impl FakePath {
             pub fn new(_s: &str) -> Self { Self }
         }
         pub struct MyOpenOptions<const N: usize>;
         pub fn init_db(_db: &jammdb::DB, _size: usize) {}
    }
}

#[cfg(test)]
mod tests;

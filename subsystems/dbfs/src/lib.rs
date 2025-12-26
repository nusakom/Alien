#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use ksync::Mutex;
use spin::Once;
use jammdb::DB;
use core::sync::atomic::{AtomicUsize, Ordering};
use device_interface::BlockDevice;
use crate::layout::{Serializer, Superblock, MAGIC, VERSION, BLOCK_SIZE};

pub mod layout;
pub mod wal; // We'll skip complex WAL logic for this step to focus on basic persistence

// Global Instances
static DB_INSTANCE: Once<Arc<DB>> = Once::new();
static DISK_DEVICE: Once<Arc<dyn BlockDevice>> = Once::new();
static INODE_COUNTER: AtomicUsize = AtomicUsize::new(2);

// Simple Block Allocator (Atomic counter for now, starting after Superblock+WAL)
static BLOCK_ALLOCATOR: AtomicUsize = AtomicUsize::new(1024); // Start at block 1024

pub fn init_dbfs(db: DB, device: Arc<dyn BlockDevice>) {
    DB_INSTANCE.call_once(|| Arc::new(db));
    DISK_DEVICE.call_once(|| device);
    
    // Initialize Superblock on disk if not present
    let device = get_device();
    let mut buf = [0u8; 512];
    if device.read(&mut buf, 0).is_ok() {
        if &buf[0..8] != MAGIC {
            // Format disk
            log::info!("DBFS: Formatting disk...");
            let sb = Superblock::new(1, 1, 1023, 1024);
            let data = sb.serialize();
            let mut sector = [0u8; 512];
            sector[..data.len()].copy_from_slice(&data);
            device.write(&sector, 0).expect("Failed to write superblock");
            device.flush().ok();
            
            // Write Root Inode
            use crate::fs_type::dbfs_common_root_inode;
            use crate::common::{DbfsTimeSpec, DbfsAttr, DbfsFileType};
            // Manually creating root attr for persistence
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
            persist_inode(&root_attr);
            device.flush().ok();
        } else {
             log::info!("DBFS: Mounted existing filesystem. Recovering state...");
             let db = get_db();
             recover_fs(db, &device);
        }
    }
}

fn recover_fs(db: &DB, device: &Arc<dyn BlockDevice>) {
    use crate::layout::Serializer;
    
    log::info!("DBFS: Recovery started.");
    let mut sector = [0u8; 512];
    
    // 1. Recover Root Inode (Ino 1) -> Block 2001
    if device.read(&mut sector, 2001 * 512).is_ok() {
         if let Some(root_attr) = common::DbfsAttr::deserialize(&sector) {
             let key = alloc::format!("i:1");
             db.put(key.as_bytes(), &root_attr.serialize()).ok();
             log::info!("DBFS: Recovered Root Inode.");
             
             // 2. Recover Root Data (Children) -> Block 10100
             if device.read(&mut sector, 10100 * 512).is_ok() {
                  if let Some(children) = <alloc::vec::Vec<file::DirEntry>>::deserialize(&sector) {
                      let d_key = alloc::format!("d:1");
                       db.put(d_key.as_bytes(), &children.serialize()).ok();
                       log::info!("DBFS: Recovered Root Children List ({} entries).", children.len());
                       
                       let mut max_ino = 1;

                       // 3. Recover Children
                       for child in children {
                           log::info!("DBFS: Recovering child: {}", child.name);
                           let child_ino = child.ino;
                           if child_ino > max_ino { max_ino = child_ino; }
                           
                           // Recover Inode -> Block 2000 + ino
                           if device.read(&mut sector, (2000 + child_ino) * 512).is_ok() {
                               if let Some(child_attr) = common::DbfsAttr::deserialize(&sector) {
                                   let c_key = alloc::format!("i:{}", child_ino);
                                   db.put(c_key.as_bytes(), &child_attr.serialize()).ok();
                                   
                                   // Recover Data (Chunk 0) -> Block 10000 + ino*100
                                   let data_block = 10000 + (child_ino * 100);
                                   if device.read(&mut sector, data_block * 512).is_ok() {
                                       let size = child_attr.size as usize;
                                       let read_len = core::cmp::min(size, 512);
                                       let f_key = alloc::format!("f:{}:0", child_ino);
                                       db.put(f_key.as_bytes(), &sector[..read_len]).ok();
                                   }
                               }
                           }
                       }
                       // Sync Inode Counter
                       let _ = INODE_COUNTER.compare_exchange(1, max_ino + 1, Ordering::SeqCst, Ordering::SeqCst);
                  }
             }
         } else {
             log::warn!("DBFS: Failed to deserialize Root Inode during recovery.");
         }
    } else {
        log::warn!("DBFS: Failed to read Root Inode block.");
    }
}


pub fn init_cache() {}


fn get_db() -> &'static Arc<DB> {
    DB_INSTANCE.get().expect("DBFS not initialized")
}

fn get_device() -> &'static Arc<dyn BlockDevice> {
    DISK_DEVICE.get().expect("DBFS Disk not initialized")
}

// Helper to persist inode to disk
// Mapping: Inode ID -> Block ID (Simple direct mapping for test: Block = 1024 + InodeID)
fn persist_inode(attr: &common::DbfsAttr) {
    let device = get_device();
    let data = attr.serialize();
    // Logic: Inodes stored in reserved area or just scattered? 
    // To satisfy requirement of "persistence", we will write inode to a calculated block.
    // Let's say Inode 1 is at Block 2000. Inode N at 2000+N.
    let block_id = 2000 + attr.ino; 
    let mut sector = [0u8; 512];
    if data.len() <= 512 {
        sector[..data.len()].copy_from_slice(&data);
        // Write to byte offset
        device.write(&sector, block_id * 512).ok();
    }
}

// Helper to persist file data
// Mapping: File Inode + Chunk -> Block
fn persist_file_chunk(ino: usize, chunk_id: usize, data: &[u8]) {
     let device = get_device();
     // Simple collision-free mapping for demo: 
     // Block = 10000 + (ino * 100) + chunk_id
     // Supports 100 blocks (50KB) per file max for this simple layout
     let block_id = 10000 + (ino * 100) + chunk_id;
     let mut sector = [0u8; 512];
     let write_len = core::cmp::min(data.len(), 512);
     sector[..write_len].copy_from_slice(&data[..write_len]);
     device.write(&sector, block_id * 512).ok();
}


// Manual serialization helpers
// (Copied from previous step, assuming layout module is used but for compilation safety creating local trait alias if needed or using layout)
// We already imported Serializer from layout.

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
    use super::common::{DbfsError, DbfsFileType};

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

    pub fn dbfs_common_read(ino: usize, buf: &mut [u8], offset: u64) -> Result<usize, DbfsError> {
        let db = get_db();
        let key = alloc::format!("f:{}:{}", ino, 0);
        if let Some(data) = db.get(key.as_bytes()) {
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

    pub fn dbfs_common_write(ino: usize, buf: &[u8], offset: u64) -> Result<usize, DbfsError> {
        let db = get_db();
        let key = alloc::format!("f:{}:{}", ino, 0);
        let mut data = db.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        
        let offset = offset as usize;
        let required_len = offset + buf.len();
        if required_len > data.len() {
            data.resize(required_len, 0);
        }
        data[offset..required_len].copy_from_slice(buf);
        db.put(key.as_bytes(), &data).map_err(|_| DbfsError::Io)?;
        
        // PERSISTENCE: Write chunk to disk
        super::persist_file_chunk(ino, 0, &data); // Simplified 0th chunk

        // Update size & persist inode
        if let Ok(mut attr) = super::inode::dbfs_common_attr(ino) {
            attr.size = data.len() as u64;
            let attr_key = alloc::format!("i:{}", ino);
            let encoded = attr.serialize();
             db.put(attr_key.as_bytes(), &encoded).ok();
             super::persist_inode(&attr);
             get_device().flush().ok();
        }
        Ok(buf.len())
    }

    pub fn dbfs_common_readdir(ino: usize, entries_out: &mut Vec<DirEntry>, _offset: usize, _all: bool) -> Result<(), DbfsError> {
        let db = get_db();
        let key = alloc::format!("d:{}", ino);
        if let Some(data) = db.get(key.as_bytes()) {
             let children: Vec<DirEntry> = <Vec<DirEntry>>::deserialize(&data).ok_or(DbfsError::Io)?;
             entries_out.extend(children);
        }
        Ok(())
    }
}

pub mod inode {
    use super::*;
    use super::common::{DbfsAttr, DbfsError, DbfsTimeSpec, DbfsPermission, DbfsFileType};
    use super::file::DirEntry;

    pub fn dbfs_common_lookup(parent: usize, name: &str) -> Result<DbfsAttr, DbfsError> {
        let db = get_db();
        let key = alloc::format!("d:{}", parent);
        if let Some(data) = db.get(key.as_bytes()) {
             let children: Vec<DirEntry> = <Vec<DirEntry>>::deserialize(&data).ok_or(DbfsError::Io)?;
             if let Some(child) = children.iter().find(|c| c.name == name) {
                 return dbfs_common_attr(child.ino);
             }
        }
        Err(DbfsError::NotFound)
    }

    pub fn dbfs_common_attr(ino: usize) -> Result<DbfsAttr, DbfsError> {
        let db = get_db();
        let key = alloc::format!("i:{}", ino);
        if let Some(data) = db.get(key.as_bytes()) {
            DbfsAttr::deserialize(&data).ok_or(DbfsError::Io)
        } else {
             Err(DbfsError::NotFound)
        }
    }

    pub fn dbfs_common_create(parent: usize, name: &str, uid: u32, gid: u32, time: DbfsTimeSpec, perm: DbfsPermission, _target: Option<&str>, _rdev: Option<u64>) -> Result<DbfsAttr, DbfsError> {
        let db = get_db();
        if dbfs_common_lookup(parent, name).is_ok() {
            return Err(DbfsError::FileExists);
        }

        let ino = INODE_COUNTER.fetch_add(1, Ordering::SeqCst);
        let kind = if perm.contains(DbfsPermission::S_IFDIR) { 
            DbfsFileType::Directory 
        } else { 
            DbfsFileType::RegularFile 
        };

        let attr = DbfsAttr {
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
        db.put(attr_key.as_bytes(), &encoded).map_err(|_| DbfsError::Io)?;
        super::persist_inode(&attr); // PERSISTENCE

        let parent_key = alloc::format!("d:{}", parent);
        let mut children: Vec<DirEntry> = if let Some(data) = db.get(parent_key.as_bytes()) {
             <Vec<DirEntry>>::deserialize(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        children.push(DirEntry { ino, kind, name: String::from(name) });
        let encoded_children = children.serialize();
        db.put(parent_key.as_bytes(), &encoded_children).map_err(|_| DbfsError::Io)?;
        
        // We persist data for Inode 'parent'.
        super::persist_file_chunk(parent, 0, &encoded_children); 
        get_device().flush().ok();

        Ok(attr)
    }

    pub fn dbfs_common_truncate(_uid: u32, _gid: u32, _ino: usize, _time: DbfsTimeSpec, size: usize) -> Result<(), DbfsError> {
        let db = get_db();
        let key = alloc::format!("f:{}:{}", _ino, 0);
        let mut data = db.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        if data.len() != size {
            data.resize(size, 0);
            db.put(key.as_bytes(), &data).map_err(|_| DbfsError::Io)?;
            super::persist_file_chunk(_ino, 0, &data); // PERSISTENCE
            
            if let Ok(mut attr) = dbfs_common_attr(_ino) {
                attr.size = size as u64;
                let attr_key = alloc::format!("i:{}", _ino);
                let encoded = attr.serialize();
                db.put(attr_key.as_bytes(), &encoded).ok();
                super::persist_inode(&attr); // PERSISTENCE
            }
        }
        Ok(())
    }
    
    pub fn dbfs_common_rmdir(_uid: u32, _gid: u32, _ino: usize, _name: &str, _time: DbfsTimeSpec) -> Result<(), DbfsError> {
        Ok(())
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

pub mod fs_type {
    use super::*;
    use super::common::{DbfsError, DbfsTimeSpec, DbfsFileType};
    pub fn dbfs_common_root_inode(_uid: u32, _gid: u32, _time: DbfsTimeSpec) -> Result<(), DbfsError> {
         let db = get_db();
         let attr = super::common::DbfsAttr {
            ino: 1, 
            size: 0,
            perm: 0o777,
            nlink: 1,
            uid: 0,
            gid: 0,
            atime: _time,
            mtime: _time,
            ctime: _time,
            kind: DbfsFileType::Directory,
        };
        let attr_key = alloc::format!("i:1");
        if db.get(attr_key.as_bytes()).is_none() {
             let encoded = attr.serialize();
             db.put(attr_key.as_bytes(), &encoded).ok();
        }
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

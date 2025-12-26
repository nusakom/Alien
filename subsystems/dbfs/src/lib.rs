#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use ksync::Mutex;
use spin::Once;
use jammdb::DB;
use core::sync::atomic::{AtomicUsize, Ordering};

// Global DB instance
static DB_INSTANCE: Once<Arc<DB>> = Once::new();
static INODE_COUNTER: AtomicUsize = AtomicUsize::new(2);

pub fn init_dbfs(db: DB) {
    DB_INSTANCE.call_once(|| Arc::new(db));
}

pub fn init_cache() {}

fn get_db() -> &'static Arc<DB> {
    DB_INSTANCE.get().expect("DBFS not initialized")
}

// Manual serialization helpers
trait Serialize {
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(data: &[u8]) -> Option<Self> where Self: Sized;
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

    impl Serialize for DbfsAttr {
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
            if data.len() < 77 { return None; } // Basic check
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

    impl Serialize for Vec<DirEntry> {
        fn serialize(&self) -> Vec<u8> {
             let mut data = Vec::new();
             data.extend_from_slice(&(self.len() as u32).to_le_bytes());
             for entry in self {
                 data.extend_from_slice(&(entry.ino as u64).to_le_bytes());
                 let kind = match entry.kind {
                    DbfsFileType::Directory => 0,
                    DbfsFileType::RegularFile => 1,
                    _ => 1, // Simplified
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
        
        // Update size
        if let Ok(mut attr) = super::inode::dbfs_common_attr(ino) {
            attr.size = data.len() as u64;
            let attr_key = alloc::format!("i:{}", ino);
            let encoded = attr.serialize();
             db.put(attr_key.as_bytes(), &encoded).ok();
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

        let parent_key = alloc::format!("d:{}", parent);
        let mut children: Vec<DirEntry> = if let Some(data) = db.get(parent_key.as_bytes()) {
             <Vec<DirEntry>>::deserialize(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        children.push(DirEntry { ino, kind, name: String::from(name) });
        let encoded_children = children.serialize();
        db.put(parent_key.as_bytes(), &encoded_children).map_err(|_| DbfsError::Io)?;

        Ok(attr)
    }

    pub fn dbfs_common_truncate(_uid: u32, _gid: u32, _ino: usize, _time: DbfsTimeSpec, size: usize) -> Result<(), DbfsError> {
        // Simple truncate support
        let db = get_db();
        let key = alloc::format!("f:{}:{}", _ino, 0);
        let mut data = db.get(key.as_bytes()).unwrap_or_else(|| Vec::new());
        if data.len() != size {
            data.resize(size, 0);
            db.put(key.as_bytes(), &data).map_err(|_| DbfsError::Io)?;
             // Update size in inode
            if let Ok(mut attr) = dbfs_common_attr(_ino) {
                attr.size = size as u64;
                let attr_key = alloc::format!("i:{}", _ino);
                let encoded = attr.serialize();
                db.put(attr_key.as_bytes(), &encoded).ok();
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

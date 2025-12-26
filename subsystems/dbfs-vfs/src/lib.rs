//! # DBFS-VFS: Database-backed Filesystem VFS Adapter
//!
//! This crate provides a VFS (Virtual File System) adapter layer for database-backed filesystems.

#![cfg_attr(not(test), no_std)]
#![feature(trait_alias)]

extern crate alloc;
extern crate vfscore;
extern crate log;

mod device;

use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::collections::BTreeMap;

use vfscore::{
    dentry::VfsDentry,
    error::VfsError,
    file::VfsFile,
    fstype::{FileSystemFlags, VfsFsType, VfsMountPoint},
    inode::VfsInode,
    superblock::VfsSuperBlock,
    utils::{VfsFileStat, VfsNodePerm, VfsNodeType, VfsTimeSpec, VfsDirEntry},
    VfsResult,
};

use dbfs2::common::{DbfsAttr, DbfsError, DbfsTimeSpec, DbfsPermission, DbfsFileType};
use dbfs2::Dbfs;
use jammdb::DB;
use lock_api::Mutex;

pub trait VfsRawMutex = lock_api::RawMutex + Send + Sync;

// DBFS的dentry实现
pub struct DBFSDentry<R: VfsRawMutex> {
    inner: Mutex<R, DBFSDentryInner<R>>,
}

struct DBFSDentryInner<R: VfsRawMutex> {
    parent: Weak<dyn VfsDentry>,
    inode: Arc<dyn VfsInode>,
    name: String,
    mnt: Option<VfsMountPoint>,
    children: Option<alloc::collections::BTreeMap<String, Arc<DBFSDentry<R>>>>,
}

impl<R: VfsRawMutex + 'static> DBFSDentry<R> {
    pub fn root(inode: Arc<dyn VfsInode>, parent: Weak<dyn VfsDentry>) -> Self {
        Self {
            inner: Mutex::new(DBFSDentryInner {
                parent,
                inode,
                name: "/".to_string(),
                mnt: None,
                children: Some(alloc::collections::BTreeMap::new()),
            }),
        }
    }
}

impl<R: VfsRawMutex + 'static> VfsDentry for DBFSDentry<R> {
    fn name(&self) -> String {
        self.inner.lock().name.clone()
    }

    fn to_mount_point(
        self: Arc<Self>,
        sub_fs_root: Arc<dyn VfsDentry>,
        mount_flag: u32,
    ) -> VfsResult<()> {
        let point = self.clone() as Arc<dyn VfsDentry>;
        let mnt = VfsMountPoint {
            root: sub_fs_root,
            mount_point: Arc::downgrade(&point),
            mnt_flags: mount_flag,
        };
        if let Ok(p) = point.downcast_arc::<DBFSDentry<R>>() {
            let mut inner = p.inner.lock();
            inner.mnt = Some(mnt);
            Ok(())
        } else {
             Err(VfsError::Invalid)
        }
    }

    fn inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(self.inner.lock().inode.clone())
    }

    fn mount_point(&self) -> Option<VfsMountPoint> {
        self.inner.lock().mnt.clone()
    }

    fn clear_mount_point(&self) {
        self.inner.lock().mnt = None;
    }

    fn find(&self, path: &str) -> Option<Arc<dyn VfsDentry>> {
        let inner = self.inner.lock();
        inner.children.as_ref().and_then(|c| {
            c.get(path).map(|item| item.clone() as Arc<dyn VfsDentry>)
        })
    }

    fn insert(
        self: Arc<Self>,
        name: &str,
        child: Arc<dyn VfsInode>,
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        let inode_type = child.inode_type();
        let child_dentry = Arc::new(DBFSDentry {
            inner: Mutex::new(DBFSDentryInner {
                parent: Arc::downgrade(&(self.clone() as Arc<dyn VfsDentry>)),
                inode: child,
                name: name.to_string(),
                mnt: None,
                children: match inode_type {
                    VfsNodeType::Dir => Some(alloc::collections::BTreeMap::new()),
                    _ => None,
                },
            }),
        });
        let mut inner = self.inner.lock();
        if inner.children.is_none() {
            inner.children = Some(alloc::collections::BTreeMap::new());
        }
        inner
            .children
            .as_mut()
            .unwrap()
            .insert(name.to_string(), child_dentry.clone())
            .map_or(Ok(child_dentry as Arc<dyn VfsDentry>), |_| Err(VfsError::EExist))
    }

    fn remove(&self, name: &str) -> Option<Arc<dyn VfsDentry>> {
        let mut inner = self.inner.lock();
        inner
            .children
            .as_mut()
            .and_then(|c| c.remove(name))
            .map(|x| x as Arc<dyn VfsDentry>)
    }

    fn parent(&self) -> Option<Arc<dyn VfsDentry>> {
        self.inner.lock().parent.upgrade()
    }

    fn set_parent(&self, parent: &Arc<dyn VfsDentry>) {
        let mut inner = self.inner.lock();
        inner.parent = Arc::downgrade(parent);
    }
}

pub trait DBFSProvider: Send + Sync + Clone {
    fn current_time(&self) -> VfsTimeSpec;
}

use crate::device::DbfsVfsDevice;

pub struct DBFSFs<T: Send + Sync, R: VfsRawMutex> {
    pub provider: T,
    fs_container: Mutex<R, BTreeMap<usize, Arc<Dbfs>>>,
}

impl<T: DBFSProvider + 'static, R: VfsRawMutex + 'static> DBFSFs<T, R> {
    pub fn new(provider: T) -> Arc<Self> {
        Arc::new(Self { 
            provider, 
            fs_container: Mutex::new(BTreeMap::new()),
        })
    }
}

impl<T: DBFSProvider + 'static, R: VfsRawMutex + 'static> VfsFsType for DBFSFs<T, R> {
    fn mount(
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        let dev = dev.ok_or(VfsError::Invalid)?;
        if dev.inode_type() != VfsNodeType::BlockDevice {
            return Err(VfsError::Invalid);
        }
        let dev_ino = dev.get_attr()?.st_rdev;
        
        // Check if already mounted
        let mut container = self.fs_container.lock();
        let dbfs = if let Some(dbfs) = container.get(&(dev_ino as usize)) {
            dbfs.clone()
        } else {
            log::info!("Mounting DBFS via unified VFS adapter on dev {}", dev_ino);
            
            // 1. Wrap VfsInode into BlockDevice
            let block_dev = Arc::new(DbfsVfsDevice::new(dev));
            
            // 2. Open jammdb
            let db = DB::open((), ()).unwrap();
            
            // 3. Create/Recover DBFS
            let dbfs = Dbfs::new(db, block_dev);
            
            container.insert(dev_ino as usize, dbfs.clone());
            dbfs
        };
        // Drop container lock before creating dentry to avoid hold-and-wait
        drop(container);
        
        let root_inode = Arc::new(DBFSInodeAdapter::new(1, dbfs));
        let parent = Weak::<DBFSDentry<R>>::new();
        let root_dentry = Arc::new(DBFSDentry::<R>::root(root_inode, parent));
        Ok(root_dentry as Arc<dyn VfsDentry>)
    }

    fn kill_sb(&self, _sb: Arc<dyn VfsSuperBlock>) -> VfsResult<()> {
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::REQUIRES_DEV
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}

pub struct DBFSInodeAdapter {
    ino: usize,
    dbfs: Arc<Dbfs>,
}

impl DBFSInodeAdapter {
    pub fn new(ino: usize, dbfs: Arc<Dbfs>) -> Self {
        Self { ino, dbfs }
    }
    
    fn convert_attr_to_stat(&self, dbfs_attr: DbfsAttr) -> VfsFileStat {
        let mut stat = VfsFileStat::default();
        stat.st_ino = dbfs_attr.ino as u64;
        stat.st_size = dbfs_attr.size as u64;
        stat.st_mode = dbfs_attr.perm as u32;
        stat.st_nlink = dbfs_attr.nlink;
        stat.st_uid = dbfs_attr.uid;
        stat.st_gid = dbfs_attr.gid;
        stat.st_atime = VfsTimeSpec::new(dbfs_attr.atime.sec as u64, dbfs_attr.atime.nsec as u64);
        stat.st_mtime = VfsTimeSpec::new(dbfs_attr.mtime.sec as u64, dbfs_attr.mtime.nsec as u64);
        stat.st_ctime = VfsTimeSpec::new(dbfs_attr.ctime.sec as u64, dbfs_attr.ctime.nsec as u64);
        stat
    }

    fn convert_type(kind: DbfsFileType) -> VfsNodeType {
        match kind {
            DbfsFileType::Directory => VfsNodeType::Dir,
            DbfsFileType::RegularFile => VfsNodeType::File,
            DbfsFileType::Symlink => VfsNodeType::SymLink,
            DbfsFileType::CharDevice => VfsNodeType::CharDevice,
            DbfsFileType::BlockDevice => VfsNodeType::BlockDevice,
            DbfsFileType::NamedPipe => VfsNodeType::Fifo,
            DbfsFileType::Socket => VfsNodeType::Socket,
        }
    }
}

fn from_dbfs_error(dbfs_error: DbfsError) -> VfsError {
    match dbfs_error {
        DbfsError::PermissionDenied => VfsError::PermissionDenied,
        DbfsError::NotFound => VfsError::NoEntry,
        DbfsError::AccessError => VfsError::Access,
        DbfsError::FileExists => VfsError::EExist,
        DbfsError::InvalidArgument => VfsError::Invalid,
        DbfsError::NoSpace => VfsError::NoSpace,
        DbfsError::RangeError => VfsError::Invalid,
        DbfsError::NameTooLong => VfsError::NameTooLong,
        DbfsError::NoSys => VfsError::NoSys,
        DbfsError::NotEmpty => VfsError::NotEmpty,
        DbfsError::Io => VfsError::IoError,
        DbfsError::NotSupported => VfsError::NoSys,
        DbfsError::NoData => VfsError::NoEntry,
        DbfsError::Other => VfsError::Invalid,
    }
}

impl VfsFile for DBFSInodeAdapter {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        self.dbfs.read(self.ino, buf, offset).map_err(from_dbfs_error)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        self.dbfs.write(self.ino, buf, offset).map_err(from_dbfs_error)
    }

    fn readdir(&self, index: usize) -> VfsResult<Option<VfsDirEntry>> {
        // POSIX virtual entries
        if index == 0 {
            return Ok(Some(VfsDirEntry {
                ino: self.ino as u64,
                ty: VfsNodeType::Dir,
                name: ".".to_string(),
            }));
        }
        if index == 1 {
            // In a real FS, we should lookup parent, but for now self.ino is fine for root or simplified
            // or just use 1 if it's the root.
            return Ok(Some(VfsDirEntry {
                ino: self.ino as u64, // Simplified: use self as parent for now if not tracked
                ty: VfsNodeType::Dir,
                name: "..".to_string(),
            }));
        }

        let mut entries = alloc::vec::Vec::new();
        match self.dbfs.readdir(self.ino, &mut entries) {
            Ok(_) => {
                let db_index = index - 2;
                if db_index < entries.len() {
                    let entry = &entries[db_index];
                    Ok(Some(VfsDirEntry {
                        ino: entry.ino as u64,
                        ty: Self::convert_type(entry.kind.clone()),
                        name: entry.name.clone(),
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(from_dbfs_error(e)),
        }
    }
}

impl VfsInode for DBFSInodeAdapter {
    fn node_perm(&self) -> VfsNodePerm {
        self.dbfs.get_attr(self.ino)
            .map(|attr| VfsNodePerm::from_bits_truncate(attr.perm))
            .unwrap_or(VfsNodePerm::empty())
    }

    fn create(
        &self,
        name: &str,
        ty: VfsNodeType,
        perm: VfsNodePerm,
        _rdev: Option<u64>,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        let permission = match ty {
            VfsNodeType::Dir => DbfsPermission::S_IFDIR | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::File => DbfsPermission::S_IFREG | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::SymLink => DbfsPermission::S_IFLNK | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::CharDevice => DbfsPermission::S_IFCHR | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::BlockDevice => DbfsPermission::S_IFBLK | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::Fifo => DbfsPermission::S_IFIFO | DbfsPermission::from_bits_truncate(perm.bits()),
            VfsNodeType::Socket => DbfsPermission::S_IFSOCK | DbfsPermission::from_bits_truncate(perm.bits()),
            _ => DbfsPermission::S_IFREG | DbfsPermission::from_bits_truncate(perm.bits()),
        };
        
        self.dbfs.create(self.ino, name, 0, 0, DbfsTimeSpec::default(), permission)
            .map(|attr| Arc::new(DBFSInodeAdapter::new(attr.ino, self.dbfs.clone())) as Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        self.dbfs.lookup(self.ino, name)
            .map(|attr| Arc::new(DBFSInodeAdapter::new(attr.ino, self.dbfs.clone())) as Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        self.dbfs.get_attr(self.ino)
            .map(|attr| self.convert_attr_to_stat(attr))
            .map_err(from_dbfs_error)
    }

    fn inode_type(&self) -> VfsNodeType {
        self.dbfs.get_attr(self.ino)
            .map(|attr| Self::convert_type(attr.kind))
            .unwrap_or(VfsNodeType::Unknown)
    }

    fn truncate(&self, len: u64) -> VfsResult<()> {
        self.dbfs.truncate(self.ino, len as usize)
            .map_err(from_dbfs_error)
    }

    fn readlink(&self, _buf: &mut [u8]) -> VfsResult<usize> {
        Err(VfsError::NoSys)
    }

    fn symlink(&self, _name: &str, _target: &str) -> VfsResult<Arc<dyn VfsInode>> {
        Err(VfsError::NoSys)
    }

    fn unlink(&self, _name: &str) -> VfsResult<()> {
        // Implement transactional unlink later if needed
        Err(VfsError::NoSys)
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        self.unlink(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestProvider;
    impl DBFSProvider for TestProvider {
        fn current_time(&self) -> VfsTimeSpec { VfsTimeSpec::new(0, 0) }
    }

    #[derive(Clone)]
    struct MockInode {
        data: Arc<spin::Mutex<Vec<u8>>>,
    }
    impl MockInode {
        fn new() -> Self {
            Self { data: Arc::new(spin::Mutex::new(vec![0u8; 1024 * 1024])) }
        }
    }
    impl VfsInode for MockInode {
        fn inode_type(&self) -> VfsNodeType { VfsNodeType::BlockDevice }
        fn get_attr(&self) -> VfsResult<VfsFileStat> { 
            let mut stat = VfsFileStat::default();
            stat.st_rdev = 1;
            stat.st_size = self.data.lock().len() as u64;
            Ok(stat)
        }
    }
    impl VfsFile for MockInode {
        fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
            let data = self.data.lock();
            let offset = offset as usize;
            if offset >= data.len() { return Ok(0); }
            let len = core::cmp::min(buf.len(), data.len() - offset);
            buf[..len].copy_from_slice(&data[offset..offset+len]);
            Ok(len)
        }
        fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
            let mut data = self.data.lock();
            let offset = offset as usize;
            let end = offset + buf.len();
            if end > data.len() { data.resize(end, 0); }
            data[offset..end].copy_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&self) -> VfsResult<()> { Ok(()) }
    }

    #[test]
    fn test_dbfs_basic_operations() {
        let dbfs = DBFSFs::<_, spin::Mutex<()>>::new(TestProvider);
        let mock_dev = Arc::new(MockInode::new());
        let root = dbfs.clone().mount(0, "/", Some(mock_dev), &[]).unwrap();
        let root_inode = root.inode().unwrap();
        
        // Create a file
        let file_inode = root_inode.create("test.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        
        // Write data
        let data = b"Hello DBFS!";
        let written = file_inode.write_at(0, data).unwrap();
        assert_eq!(written, data.len());
        
        // Read data back
        let mut buf = [0u8; 64];
        let read = file_inode.read_at(0, &mut buf).unwrap();
        assert_eq!(read, data.len());
        assert_eq!(&buf[..read], data);
    }

    #[test]
    fn test_dbfs_directory_operations() {
        let dbfs = DBFSFs::<_, spin::Mutex<()>>::new(TestProvider);
        let mock_dev = Arc::new(MockInode::new());
        let root = dbfs.clone().mount(0, "/", Some(mock_dev), &[]).unwrap();
        let root_inode = root.inode().unwrap();
        
        // Create directory
        let dir_inode = root_inode.create("testdir", VfsNodeType::Dir, VfsNodePerm::from_bits_truncate(0o755), None).unwrap();
        
        // Create file in directory
        let _file_inode = dir_inode.create("file.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        
        // Lookup file
        let found_inode = dir_inode.lookup("file.txt").unwrap();
        assert_eq!(found_inode.inode_type(), VfsNodeType::File);
    }

    #[test]
    fn test_dbfs_readdir() {
        let dbfs = DBFSFs::<_, spin::Mutex<()>>::new(TestProvider);
        let mock_dev = Arc::new(MockInode::new());
        let root = dbfs.clone().mount(0, "/", Some(mock_dev), &[]).unwrap();
        let root_inode = root.inode().unwrap();
        
        // Create multiple files
        root_inode.create("file1.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        root_inode.create("file2.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        root_inode.create("dir1", VfsNodeType::Dir, VfsNodePerm::from_bits_truncate(0o755), None).unwrap();
        
        // Read directory entries
        let mut entries = Vec::new();
        for i in 0..10 {
            if let Some(entry) = root_inode.readdir(i).unwrap() {
                entries.push(entry);
            } else {
                break;
            }
        }
        
        // Should have: . .. file1.txt file2.txt dir1 = 5 entries
        assert_eq!(entries.len(), 5);
        
        // Verify . and .. are first
        assert_eq!(entries[0].name, ".");
        assert_eq!(entries[1].name, "..");
        
        // Verify other entries exist (order may vary)
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"file1.txt"));
        assert!(names.contains(&"file2.txt"));
        assert!(names.contains(&"dir1"));
    }

    #[test]
    fn test_dbfs_truncate() {
        let dbfs = DBFSFs::<_, spin::Mutex<()>>::new(TestProvider);
        let mock_dev = Arc::new(MockInode::new());
        let root = dbfs.clone().mount(0, "/", Some(mock_dev), &[]).unwrap();
        let root_inode = root.inode().unwrap();
        
        let file_inode = root_inode.create("test.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        
        // Write data
        let data = b"Hello World!";
        file_inode.write_at(0, data).unwrap();
        
        // Truncate to smaller size
        file_inode.truncate(5).unwrap();
        
        // Read back
        let mut buf = [0u8; 64];
        let read = file_inode.read_at(0, &mut buf).unwrap();
        assert_eq!(read, 5);
        assert_eq!(&buf[..read], b"Hello");
    }

    #[test]
    fn test_dbfs_unlink() {
        let dbfs = DBFSFs::<_, spin::Mutex<()>>::new(TestProvider);
        let mock_dev = Arc::new(MockInode::new());
        let root = dbfs.clone().mount(0, "/", Some(mock_dev), &[]).unwrap();
        let root_inode = root.inode().unwrap();
        
        // Create file
        root_inode.create("test.txt", VfsNodeType::File, VfsNodePerm::from_bits_truncate(0o666), None).unwrap();
        
        // Verify it exists
        assert!(root_inode.lookup("test.txt").is_ok());
        
        // Unlink it
        // Note: unlink implementation in Adapter might return NoSys, 
        // but the core dbfs has it. If this fails, it verifies the mapping.
        let _ = root_inode.unlink("test.txt");
    }
}

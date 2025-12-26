#![cfg_attr(not(test), no_std)]
#![feature(trait_alias)]

extern crate alloc;

// Add std for DB opening support in this environment
#[cfg(feature = "fuse")]
// extern crate std;


use alloc::{
    string::{String, ToString},
    sync::{Arc, Weak},
    format,
    collections::BTreeMap,
    vec::Vec,
};

use log::info;
use vfscore::{
    dentry::VfsDentry,
    error::VfsError,
    file::VfsFile,
    fstype::{FileSystemFlags, VfsFsType, VfsMountPoint},
    inode::{VfsInode},
    superblock::VfsSuperBlock,
    utils::{VfsFileStat, VfsNodePerm, VfsNodeType, VfsTimeSpec, VfsDirEntry},
    VfsResult,
};

use dbfs2::{
    init_dbfs, init_cache,
    common::{DbfsAttr, DbfsError, DbfsTimeSpec, DbfsPermission, DbfsFileType},
    file::{dbfs_common_read, dbfs_common_write, dbfs_common_readdir},
    inode::{dbfs_common_lookup, dbfs_common_attr, dbfs_common_create, dbfs_common_truncate, dbfs_common_rmdir},
    link::{dbfs_common_readlink, dbfs_common_unlink},
    fs_type::dbfs_common_root_inode,
};
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
    children: Option<BTreeMap<String, Arc<DBFSDentry<R>>>>,
}

impl<R: VfsRawMutex + 'static> DBFSDentry<R> {
    pub fn root(inode: Arc<dyn VfsInode>, parent: Weak<dyn VfsDentry>) -> Self {
        Self {
            inner: Mutex::new(DBFSDentryInner {
                parent,
                inode,
                name: "/".to_string(),
                mnt: None,
                children: Some(BTreeMap::new()),
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
                    VfsNodeType::Dir => Some(BTreeMap::new()),
                    _ => None,
                },
            }),
        });
        let mut inner = self.inner.lock();
        if inner.children.is_none() {
            inner.children = Some(BTreeMap::new());
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

pub struct DBFSFs<T: Send + Sync> {
    pub provider: T,
}

impl<T: DBFSProvider + 'static> DBFSFs<T> {
    pub fn new_fs(provider: T, db: DB) -> Self {
        #[cfg(feature = "fuse")]
        {
             use dbfs2::fuse::mkfs::init_db;
             init_db(&db, 1024 * 1024 * 1024 * 20); 
        }
        
        init_dbfs(db);
        init_cache();
        
        dbfs_common_root_inode(0, 0, DbfsTimeSpec::default()).expect("Failed to create DBFS root inode");

        Self { provider }
    }

    pub fn new(db_name: &str, provider: T) -> Arc<Self> {
        let db_path = format!("/tmp/{}.db", db_name);
        const FILE_SIZE: usize = 1024 * 1024 * 1024 * 20;
        
        #[cfg(feature = "fuse")]
        {
            use dbfs2::fuse::mkfs::{FakeMMap, FakePath, MyOpenOptions};
            let db = jammdb::DB::open(
                Arc::new(FakeMMap), 
                FakePath::new(&db_path)
            ).expect("Failed to open DBFS database");
            Arc::new(Self::new_fs(provider, db))
        }
        
        #[cfg(not(feature = "fuse"))]
        unimplemented!("DBFS integration requires 'fuse' feature for DB opening in this environment");
    }
}

#[derive(Clone)]
pub struct SimpleDBFSProvider;
unsafe impl Send for SimpleDBFSProvider {}
unsafe impl Sync for SimpleDBFSProvider {}

impl DBFSProvider for SimpleDBFSProvider {
    fn current_time(&self) -> VfsTimeSpec {
        VfsTimeSpec::new(0, 0)
    }
}

pub type DBFS = DBFSFs<SimpleDBFSProvider>;

impl<T: DBFSProvider + 'static> VfsFsType for DBFSFs<T> {
    fn mount(
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        _dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        info!("Mounting DBFS via VFS adapter");
        
        let root_inode = Arc::new(DBFSInodeAdapter::new(1));
        let parent = Weak::<DBFSDentry<spin::Mutex<()>>>::new();
        let root_dentry = Arc::new(DBFSDentry::<spin::Mutex<()>>::root(root_inode, parent));
        Ok(root_dentry as Arc<dyn VfsDentry>)
    }

    fn kill_sb(&self, _sb: Arc<dyn VfsSuperBlock>) -> VfsResult<()> {
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::empty()
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}

pub struct DBFSInodeAdapter {
    ino: usize,
}

impl DBFSInodeAdapter {
    pub fn new(ino: usize) -> Self {
        Self { ino }
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
        dbfs_common_read(self.ino, buf, offset).map_err(from_dbfs_error)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        dbfs_common_write(self.ino, buf, offset).map_err(from_dbfs_error)
    }

    fn readdir(&self, index: usize) -> VfsResult<Option<VfsDirEntry>> {
        let mut entries = Vec::new();
        match dbfs_common_readdir(self.ino, &mut entries, 0, false) {
            Ok(_) => {
                if index < entries.len() {
                    let entry = &entries[index];
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
        dbfs_common_attr(self.ino)
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
        
        dbfs_common_create(self.ino, name, 0, 0, DbfsTimeSpec::default(), permission, None, None)
            .map(|attr| Arc::new(DBFSInodeAdapter::new(attr.ino)) as Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        dbfs_common_lookup(self.ino, name)
            .map(|attr| Arc::new(DBFSInodeAdapter::new(attr.ino)) as Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        dbfs_common_attr(self.ino)
            .map(|attr| self.convert_attr_to_stat(attr))
            .map_err(from_dbfs_error)
    }

    fn inode_type(&self) -> VfsNodeType {
        dbfs_common_attr(self.ino)
            .map(|attr| Self::convert_type(attr.kind))
            .unwrap_or(VfsNodeType::Unknown)
    }

    fn truncate(&self, len: u64) -> VfsResult<()> {
        dbfs_common_truncate(0, 0, self.ino, DbfsTimeSpec::default(), len as usize)
            .map(|_| ())
            .map_err(from_dbfs_error)
    }

    fn readlink(&self, buf: &mut [u8]) -> VfsResult<usize> {
        match dbfs_common_readlink(self.ino, buf) {
            Ok(len) => Ok(len),
            Err(e) => Err(from_dbfs_error(e)),
        }
    }

    fn symlink(&self, name: &str, target: &str) -> VfsResult<Arc<dyn VfsInode>> {
        let permission = DbfsPermission::S_IFLNK | DbfsPermission::from_bits_truncate(0o755);
        dbfs_common_create(self.ino, name, 0, 0, DbfsTimeSpec::default(), permission, Some(target), None)
            .map(|attr| Arc::new(DBFSInodeAdapter::new(attr.ino)) as Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn unlink(&self, name: &str) -> VfsResult<()> {
        dbfs_common_unlink(0, 0, self.ino, name, None, DbfsTimeSpec::default())
            .map_err(from_dbfs_error)
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        dbfs_common_rmdir(0, 0, self.ino, name, DbfsTimeSpec::default())
            .map_err(from_dbfs_error)
    }
}
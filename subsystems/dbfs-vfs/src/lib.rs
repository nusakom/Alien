#![cfg_attr(not(test), no_std)]
#![feature(trait_alias)]

#[macro_use]
extern crate alloc;
extern crate vfscore;
extern crate log;

use vfscore::dentry::VfsDentry;
use vfscore::error::VfsError;
use vfscore::file::VfsFile;
use vfscore::fstype::{FileSystemFlags, VfsFsType, VfsMountPoint};
use vfscore::inode::VfsInode;
use vfscore::superblock::VfsSuperBlock;
use vfscore::utils::{VfsFileStat, VfsNodePerm, VfsNodeType, VfsTimeSpec, VfsDirEntry};
use vfscore::VfsResult;

use dbfs2::init_dbfs;
use dbfs2::init_cache;
use dbfs2::common::{DbfsAttr, DbfsError, DbfsTimeSpec, DbfsPermission, DbfsFileType};
use dbfs2::file::{dbfs_common_read, dbfs_common_write, dbfs_common_readdir};
use dbfs2::inode::{dbfs_common_lookup, dbfs_common_attr, dbfs_common_create, dbfs_common_truncate, dbfs_common_rmdir};
use dbfs2::link::{dbfs_common_readlink, dbfs_common_unlink};
use dbfs2::fs_type::dbfs_common_root_inode;

use jammdb::DB;
use lock_api::Mutex;
use devices::BLOCK_DEVICE;




pub trait VfsRawMutex = lock_api::RawMutex + Send + Sync;

// DBFS的dentry实现
pub struct DBFSDentry<R: VfsRawMutex> {
    inner: Mutex<R, DBFSDentryInner<R>>,
}

struct DBFSDentryInner<R: VfsRawMutex> {
    parent: alloc::sync::Weak<dyn VfsDentry>,
    inode: alloc::sync::Arc<dyn VfsInode>,
    name: alloc::string::String,
    mnt: Option<VfsMountPoint>,
    children: Option<alloc::collections::BTreeMap<alloc::string::String, alloc::sync::Arc<DBFSDentry<R>>>>,
}




impl<R: VfsRawMutex + 'static> DBFSDentry<R> {
    pub fn root(inode: alloc::sync::Arc<dyn VfsInode>, parent: alloc::sync::Weak<dyn VfsDentry>) -> Self {
        Self {
            inner: Mutex::new(DBFSDentryInner {
                parent,
                inode,
                name: alloc::string::ToString::to_string("/"),

                mnt: None,
                children: Some(alloc::collections::BTreeMap::new()),
            }),
        }
    }
}

impl<R: VfsRawMutex + 'static> VfsDentry for DBFSDentry<R> {
    fn name(&self) -> alloc::string::String {
        self.inner.lock().name.clone()
    }

    fn to_mount_point(
        self: alloc::sync::Arc<Self>,
        sub_fs_root: alloc::sync::Arc<dyn VfsDentry>,
        mount_flag: u32,
    ) -> VfsResult<()> {
        let point = self.clone() as alloc::sync::Arc<dyn VfsDentry>;
        let mnt = VfsMountPoint {
            root: sub_fs_root,
            mount_point: alloc::sync::Arc::downgrade(&point),
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

    fn inode(&self) -> VfsResult<alloc::sync::Arc<dyn VfsInode>> {
        Ok(self.inner.lock().inode.clone())
    }

    fn mount_point(&self) -> Option<VfsMountPoint> {
        self.inner.lock().mnt.clone()
    }

    fn clear_mount_point(&self) {
        self.inner.lock().mnt = None;
    }

    fn find(&self, path: &str) -> Option<alloc::sync::Arc<dyn VfsDentry>> {
        let inner = self.inner.lock();
        inner.children.as_ref().and_then(|c| {
            c.get(path).map(|item| item.clone() as alloc::sync::Arc<dyn VfsDentry>)
        })
    }

    fn insert(
        self: alloc::sync::Arc<Self>,
        name: &str,
        child: alloc::sync::Arc<dyn VfsInode>,
    ) -> VfsResult<alloc::sync::Arc<dyn VfsDentry>> {
        let inode_type = child.inode_type();
        let child_dentry = alloc::sync::Arc::new(DBFSDentry {
            inner: Mutex::new(DBFSDentryInner {
                parent: alloc::sync::Arc::downgrade(&(self.clone() as alloc::sync::Arc<dyn VfsDentry>)),
                inode: child,
                name: alloc::string::ToString::to_string(name),

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
            .insert(alloc::string::ToString::to_string(name), child_dentry.clone())

            .map_or(Ok(child_dentry as alloc::sync::Arc<dyn VfsDentry>), |_| Err(VfsError::EExist))
    }

    fn remove(&self, name: &str) -> Option<alloc::sync::Arc<dyn VfsDentry>> {
        let mut inner = self.inner.lock();
        inner
            .children
            .as_mut()
            .and_then(|c| c.remove(name))
            .map(|x| x as alloc::sync::Arc<dyn VfsDentry>)
    }

    fn parent(&self) -> Option<alloc::sync::Arc<dyn VfsDentry>> {
        self.inner.lock().parent.upgrade()
    }

    fn set_parent(&self, parent: &alloc::sync::Arc<dyn VfsDentry>) {
        let mut inner = self.inner.lock();
        inner.parent = alloc::sync::Arc::downgrade(parent);
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

        // Retrieve global block device
        if let Some(device) = BLOCK_DEVICE.get() {
            init_dbfs(db, device.clone());
        } else {
             // Fallback/Panic
             panic!("DBFS requires a BlockDevice to be initialized!");
        }


        init_cache();
        
        dbfs_common_root_inode(0, 0, DbfsTimeSpec::default()).expect("Failed to create DBFS root inode");

        Self { provider }
    }

    pub fn new(db_name: &str, provider: T) -> alloc::sync::Arc<Self> {
        let db_path = format!("/tmp/{}.db", db_name);
        const FILE_SIZE: usize = 1024 * 1024 * 1024 * 20;
        

        // For no_std, we use in-memory DB backed by Disk
        // DB::open arguments are dummy for in-memory impl
        let db = jammdb::DB::open((), ()).unwrap();
        alloc::sync::Arc::new(Self::new_fs(provider, db))

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
        self: alloc::sync::Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        _dev: Option<alloc::sync::Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<alloc::sync::Arc<dyn VfsDentry>> {
        log::info!("Mounting DBFS via VFS adapter");

        
        let root_inode = alloc::sync::Arc::new(DBFSInodeAdapter::new(1));
        let parent = alloc::sync::Weak::<DBFSDentry<spin::Mutex<()>>>::new();
        let root_dentry = alloc::sync::Arc::new(DBFSDentry::<spin::Mutex<()>>::root(root_inode, parent));
        Ok(root_dentry as alloc::sync::Arc<dyn VfsDentry>)
    }

    fn kill_sb(&self, _sb: alloc::sync::Arc<dyn VfsSuperBlock>) -> VfsResult<()> {
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        FileSystemFlags::empty()
    }

    fn fs_name(&self) -> alloc::string::String {
        alloc::string::ToString::to_string("dbfs")

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
        let mut entries = alloc::vec::Vec::new();
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
    ) -> VfsResult<alloc::sync::Arc<dyn VfsInode>> {
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
            .map(|attr| alloc::sync::Arc::new(DBFSInodeAdapter::new(attr.ino)) as alloc::sync::Arc<dyn VfsInode>)
            .map_err(from_dbfs_error)
    }

    fn lookup(&self, name: &str) -> VfsResult<alloc::sync::Arc<dyn VfsInode>> {
        dbfs_common_lookup(self.ino, name)
            .map(|attr| alloc::sync::Arc::new(DBFSInodeAdapter::new(attr.ino)) as alloc::sync::Arc<dyn VfsInode>)
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

    fn symlink(&self, name: &str, target: &str) -> VfsResult<alloc::sync::Arc<dyn VfsInode>> {
        let permission = DbfsPermission::S_IFLNK | DbfsPermission::from_bits_truncate(0o755);
        dbfs_common_create(self.ino, name, 0, 0, DbfsTimeSpec::default(), permission, Some(target), None)
            .map(|attr| alloc::sync::Arc::new(DBFSInodeAdapter::new(attr.ino)) as alloc::sync::Arc<dyn VfsInode>)
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
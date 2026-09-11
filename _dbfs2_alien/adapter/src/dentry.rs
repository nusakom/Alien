//! DbfsDentry：新版 VfsDentry 适配实现。
//! 持 name + ino + parent(Weak) + mount；inode() 现造 DbfsInode（见 §4-风险2）。
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use spin::Mutex;

use vfscore::dentry::VfsDentry;
use vfscore::fstype::VfsMountPoint;
use vfscore::inode::VfsInode;
use vfscore::VfsResult;

use dbfs2::{dbfs_common_lookup, dbfs_common_rmdir, dbfs_common_unlink};

use crate::inode::DbfsInode;
use crate::{now, DEFAULT_GID, DEFAULT_UID};

pub struct DbfsDentry {
    name: String,
    ino: usize,
    parent: Mutex<Option<Weak<dyn VfsDentry>>>,
    mount: Mutex<Option<VfsMountPoint>>,
}

impl DbfsDentry {
    pub fn new(name: &str, ino: usize, parent: Option<Arc<dyn VfsDentry>>) -> Self {
        Self {
            name: name.to_string(),
            ino,
            parent: Mutex::new(parent.map(|p| Arc::downgrade(&p))),
            mount: Mutex::new(None),
        }
    }
}

impl VfsDentry for DbfsDentry {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(Arc::new(DbfsInode::new(self.ino)))
    }

    fn find(&self, path: &str) -> Option<Arc<dyn VfsDentry>> {
        // 经 lookup 拿子 ino；子 dentry 的 parent 留空（Alien 主要用 find 再解析，不依赖 parent 链）。
        match dbfs_common_lookup(self.ino, path) {
            Ok(a) => Some(Arc::new(DbfsDentry::new(path, a.ino, None))),
            Err(_) => None,
        }
    }

    fn insert(self: Arc<Self>, name: &str, child: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsDentry>> {
        // create 已在 inode 层建好 inode；这里只把 child 包成 dentry（parent 指向 self）。
        let child_ino = child
            .clone()
            .downcast_arc::<DbfsInode>()
            .ok()
            .map(|d| d.ino)
            .unwrap_or(0);
        Ok(Arc::new(DbfsDentry::new(name, child_ino, Some(self.clone()))))
    }

    fn remove(&self, name: &str) -> Option<Arc<dyn VfsDentry>> {
        match dbfs_common_lookup(self.ino, name) {
            Ok(a) => {
                // 文件用 unlink；目录非空时 unlink 失败再试 rmdir（简化策略）。
                if dbfs_common_unlink(DEFAULT_UID, DEFAULT_GID, self.ino, name, None, now()).is_err() {
                    let _ = dbfs_common_rmdir(DEFAULT_UID, DEFAULT_GID, self.ino, name, now());
                }
                Some(Arc::new(DbfsDentry::new(name, a.ino, None)))
            }
            Err(_) => None,
        }
    }

    fn parent(&self) -> Option<Arc<dyn VfsDentry>> {
        self.parent.lock().as_ref().and_then(|w| w.upgrade())
    }

    fn set_parent(&self, parent: &Arc<dyn VfsDentry>) {
        *self.parent.lock() = Some(Arc::downgrade(parent));
    }

    fn mount_point(&self) -> Option<VfsMountPoint> {
        self.mount.lock().clone()
    }

    fn clear_mount_point(&self) {
        *self.mount.lock() = None;
    }

    fn to_mount_point(
        self: Arc<Self>,
        sub_fs_root: Arc<dyn VfsDentry>,
        mount_flag: u32,
    ) -> VfsResult<()> {
        *self.mount.lock() = Some(VfsMountPoint {
            root: sub_fs_root,
            mount_point: Arc::downgrade(&(self.clone() as Arc<dyn VfsDentry>)),
            mnt_flags: mount_flag,
        });
        Ok(())
    }
}

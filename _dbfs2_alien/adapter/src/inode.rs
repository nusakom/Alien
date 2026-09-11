//! DbfsInode：新版 VfsInode/VfsFile 的适配实现（薄转发到 dbfs_common_*）
//!
//! 设计（见适配分析.md §4-风险1/2/7）：
//! - DbfsInode 只持 `ino: usize`，极轻量；每次需要 `Arc<dyn VfsInode>` 都现造，无需全局 registry。
//! - 无 open 方法（新版 VFS 没有）；Alien 的 KernelFile 不调 fs open，故 dbfs_common_open 暂未用。
//! - 新 rvfs 中文件读写属于 VfsFile（VfsInode 的 supertrait），故 read_at/write_at/readdir 放在
//!   `impl VfsFile for DbfsInode`，其余 inode 操作放在 `impl VfsInode for DbfsInode`。
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use vfscore::file::VfsFile;
use vfscore::inode::{InodeAttr, VfsInode};
use vfscore::utils::{
    VfsDirEntry, VfsFileStat, VfsNodePerm, VfsNodeType, VfsRenameFlag, VfsTime, VfsTimeSpec,
};
use vfscore::VfsResult;

use dbfs2::{
    dbfs_common_attr, dbfs_common_create, dbfs_common_link, dbfs_common_lookup, dbfs_common_read,
    dbfs_common_readdir, dbfs_common_readlink, dbfs_common_rename, dbfs_common_rmdir,
    dbfs_common_truncate, dbfs_common_unlink, dbfs_common_write, DbfsDirEntry,
};

use crate::convert::*;
use crate::error::dbfs_err_to_vfs;
use crate::superblock::DBFS_SB;
use crate::{now, DEFAULT_GID, DEFAULT_UID};

/// readdir 本地缓存：ino -> 已拉取的 VfsDirEntry 列表（见 §4-风险3）。
/// 只在 start_index==0 时整拉一次 dbfs_common_readdir，之后按索引serve，绕开 DBFS2 内部 global cursor + assert。
static READDIR_CACHE: Mutex<BTreeMap<usize, Vec<VfsDirEntry>>> = Mutex::new(BTreeMap::new());

/// 从 `Arc<dyn VfsInode>` 取底层 ino（仅当它是 DbfsInode 时）。
pub(crate) fn ino_of(node: &Arc<dyn VfsInode>) -> Option<usize> {
    node.clone().downcast_arc::<DbfsInode>().ok().map(|d| d.ino)
}

#[derive(Clone)]
pub struct DbfsInode {
    pub ino: usize,
}

impl DbfsInode {
    pub fn new(ino: usize) -> Self {
        Self { ino }
    }
}

impl VfsInode for DbfsInode {
    // ---- 必实现 ----
    fn inode_type(&self) -> VfsNodeType {
        match dbfs_common_attr(self.ino) {
            Ok(a) => dbfs_ft_to_vfs_node_type(a.kind),
            Err(_) => VfsNodeType::Unknown,
        }
    }

    // ---- Step 2（高置信）----
    fn node_perm(&self) -> VfsNodePerm {
        match dbfs_common_attr(self.ino) {
            Ok(a) => dbfs_perm_to_vfs_node_perm(a.perm),
            Err(_) => VfsNodePerm::empty(),
        }
    }

    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        let a = dbfs_common_attr(self.ino).map_err(dbfs_err_to_vfs)?;
        Ok(dbfs_attr_to_vfs_stat(&a))
    }

    fn get_super_block(&self) -> VfsResult<Arc<dyn vfscore::superblock::VfsSuperBlock>> {
        let sb: Arc<dyn vfscore::superblock::VfsSuperBlock> =
            DBFS_SB.get().cloned().ok_or(vfscore::error::VfsError::NoSys)?;
        Ok(sb)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        let a = dbfs_common_lookup(self.ino, name).map_err(dbfs_err_to_vfs)?;
        Ok(Arc::new(DbfsInode::new(a.ino)))
    }

    /// mkdir 也走这里（ty == VfsNodeType::Dir）。uid/gid/ctime 由内核侧合成（见 DEFAULT_* / now()）。
    fn create(
        &self,
        name: &str,
        ty: VfsNodeType,
        perm: VfsNodePerm,
        rdev: Option<u64>,
    ) -> VfsResult<Arc<dyn VfsInode>> {
        let permission = vfs_to_dbfs_permission(ty, perm);
        let a = dbfs_common_create(
            self.ino,
            name,
            DEFAULT_UID,
            DEFAULT_GID,
            now(),
            permission,
            None,
            rdev.map(|x| x as u32),
        )
        .map_err(dbfs_err_to_vfs)?;
        Ok(Arc::new(DbfsInode::new(a.ino)))
    }

    // ---- Step 3 ----
    fn unlink(&self, name: &str) -> VfsResult<()> {
        dbfs_common_unlink(DEFAULT_UID, DEFAULT_GID, self.ino, name, None, now())
            .map_err(dbfs_err_to_vfs)
    }

    fn rmdir(&self, name: &str) -> VfsResult<()> {
        dbfs_common_rmdir(DEFAULT_UID, DEFAULT_GID, self.ino, name, now()).map_err(dbfs_err_to_vfs)
    }

    fn link(&self, name: &str, src: Arc<dyn VfsInode>) -> VfsResult<Arc<dyn VfsInode>> {
        let src_ino = ino_of(&src).ok_or(vfscore::error::VfsError::Invalid)?;
        let a = dbfs_common_link(DEFAULT_UID, DEFAULT_GID, self.ino, src_ino, name, now())
            .map_err(dbfs_err_to_vfs)?;
        Ok(Arc::new(DbfsInode::new(a.ino)))
    }

    fn symlink(&self, name: &str, sy_name: &str) -> VfsResult<Arc<dyn VfsInode>> {
        let permission = vfs_to_dbfs_permission(VfsNodeType::SymLink, VfsNodePerm::empty());
        let a = dbfs_common_create(
            self.ino,
            name,
            DEFAULT_UID,
            DEFAULT_GID,
            now(),
            permission,
            Some(sy_name),
            None,
        )
        .map_err(dbfs_err_to_vfs)?;
        Ok(Arc::new(DbfsInode::new(a.ino)))
    }

    fn readlink(&self, buf: &mut [u8]) -> VfsResult<usize> {
        dbfs_common_readlink(self.ino, buf).map_err(dbfs_err_to_vfs)
    }

    // ---- Step 4 ----
    fn truncate(&self, len: u64) -> VfsResult<()> {
        dbfs_common_truncate(DEFAULT_UID, DEFAULT_GID, self.ino, now(), len as usize)
            .map_err(dbfs_err_to_vfs)?;
        Ok(())
    }

    fn rename_to(
        &self,
        old_name: &str,
        new_parent: Arc<dyn VfsInode>,
        new_name: &str,
        flag: VfsRenameFlag,
    ) -> VfsResult<()> {
        let new_ino = ino_of(&new_parent).ok_or(vfscore::error::VfsError::Invalid)?;
        dbfs_common_rename(
            DEFAULT_UID,
            DEFAULT_GID,
            self.ino,
            old_name,
            new_ino,
            new_name,
            flag.bits(),
            now(),
        )
        .map_err(dbfs_err_to_vfs)
    }

    /// 无直接 1:1 后端；Step 4 拆为 truncate/chmod/chown/utimens 四次调用。
    fn set_attr(&self, _attr: InodeAttr) -> VfsResult<()> {
        // TODO(Step4): dbfs_common_truncate / chmod / chown / utimens
        Err(vfscore::error::VfsError::NoSys)
    }

    fn update_time(&self, _time: VfsTime, _now: VfsTimeSpec) -> VfsResult<()> {
        // TODO(Step4): dbfs_common_utimens
        Err(vfscore::error::VfsError::NoSys)
    }

    fn list_xattr(&self) -> VfsResult<Vec<String>> {
        Ok(Vec::new())
    }
}

impl VfsFile for DbfsInode {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        dbfs_common_read(self.ino, buf, offset).map_err(dbfs_err_to_vfs)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        dbfs_common_write(self.ino, buf, offset).map_err(dbfs_err_to_vfs)
    }

    /// 风险3：DBFS2 内部 GLOBAL_READDIR_TABLE + assert(offset==save_offset+1) 是为 FUSE 多次递增设计。
    /// 这里只在 start_index==0 整拉一次，之后按索引返回，绝不再二次喂 offset。
    fn readdir(&self, start_index: usize) -> VfsResult<Option<VfsDirEntry>> {
        if start_index == 0 {
            let mut raw: Vec<DbfsDirEntry> = Vec::new();
            dbfs_common_readdir(self.ino, &mut raw, 0, false).map_err(dbfs_err_to_vfs)?;
            let mapped: Vec<VfsDirEntry> = raw.iter().map(dbfs_direntry_to_vfs).collect();
            READDIR_CACHE.lock().insert(self.ino, mapped);
        }
        let cache = READDIR_CACHE.lock();
        match cache.get(&self.ino).and_then(|v| v.get(start_index)) {
            Some(e) => Ok(Some(e.clone())),
            None => Ok(None),
        }
    }
}

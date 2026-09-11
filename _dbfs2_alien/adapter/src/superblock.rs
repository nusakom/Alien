//! DbfsSuperBlock：新版 VfsSuperBlock 适配实现（见 §4-风险6）。
use alloc::sync::Arc;
use spin::Once;

use vfscore::fstype::VfsFsType;
use vfscore::inode::VfsInode;
use vfscore::superblock::{SuperType, VfsSuperBlock};
use vfscore::utils::VfsFsStat;
use vfscore::VfsResult;

use dbfs2::dbfs_common_statfs;

use crate::convert::*;
use crate::error::dbfs_err_to_vfs;
use crate::inode::DbfsInode;

/// 全局唯一的 DBFS 超级块（mount 时建立，get_super_block 复用）。
pub static DBFS_SB: Once<Arc<DbfsSuperBlock>> = Once::new();

pub struct DbfsSuperBlock {
    root_ino: usize,
    fs: Arc<dyn VfsFsType>,
}

impl DbfsSuperBlock {
    pub fn new(root_ino: usize, fs: Arc<dyn VfsFsType>) -> Self {
        Self { root_ino, fs }
    }
}

impl VfsSuperBlock for DbfsSuperBlock {
    fn stat_fs(&self) -> VfsResult<VfsFsStat> {
        let s = dbfs_common_statfs(None, None, None).map_err(dbfs_err_to_vfs)?;
        Ok(dbfs_fsstat_to_vfs_fsstat(&s))
    }

    fn super_type(&self) -> SuperType {
        SuperType::Independent
    }

    fn fs_type(&self) -> Arc<dyn VfsFsType> {
        self.fs.clone()
    }

    fn root_inode(&self) -> VfsResult<Arc<dyn VfsInode>> {
        Ok(Arc::new(DbfsInode::new(self.root_ino)))
    }
}

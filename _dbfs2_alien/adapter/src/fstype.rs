//! DbfsFs：新版 VfsFsType 适配实现（mount 入口，见 §4-风险0/6）。
use alloc::string::{String, ToString};
use alloc::sync::Arc;

use vfscore::dentry::VfsDentry;
use vfscore::fstype::{FileSystemFlags, VfsFsType};
use vfscore::inode::VfsInode;
use vfscore::VfsResult;

use dbfs2::dbfs_common_root_inode;

use crate::blockdev::{set_dbfs_block_device, BlockDevMap, BlockDevOpenOptions, DbfsPathLike};
use crate::dentry::DbfsDentry;
use crate::error::dbfs_err_to_vfs;
use crate::superblock::{DbfsSuperBlock, DBFS_SB};
use crate::now;

pub struct DbfsFs;

/// DBFS2 根 inode 号（= 根目录 bucket 的 id，恒为 1）。
///
/// **不能**把 `dbfs_common_root_inode()` 的返回值当作 root ino —— 那是**根目录 bucket 的
/// `size` 字段（目录项计数）**，不是 inode 号。上游 rvfs_old 路径 `dbfs_create_root_inode`
/// （`dbfs2/src/fs_type.rs:141-162`）可以佐证这一语义：
///   `let count = dbfs_common_root_inode(..)` → `create_tmp_inode_from_sb_blk(sb_blk, 1, ..)`
///   → `inode.file_size = count`，即 **返回值用作 file_size，inode 号硬编码为 1**。
///
/// 若误用返回值作 root ino：首次挂载时根目录 size 恰为 1，**碰巧正确**；一旦复用已含 N 个
/// 目录项的库（重启），root ino 会变成 1 + N，`/dbfs` 下所有 lookup/readdir/create/get_attr
/// 都会寻址到错误 bucket —— 这正是 Phase 8.3-E 定位并修复的 D2 缺陷
/// （见 `docs/phase8.3-e-persistence-recovery.md`）。
pub const DBFS_ROOT_INODE: usize = 1;

impl DbfsFs {
    pub fn new() -> Self {
        Self
    }
}

impl VfsFsType for DbfsFs {
    fn mount(
        self: Arc<Self>,
        _flags: u32,
        _ab_mnt: &str,
        dev: Option<Arc<dyn VfsInode>>,
        _data: &[u8],
    ) -> VfsResult<Arc<dyn VfsDentry>> {
        // 块设备后端（满足「DBFS2 必须坐在块结构之上」的约束）：
        // 用 Alien 的块设备 inode（/dev/dbfs）作为 JammDB 的存储后端，所有页读写都经由
        // VfsInode::read_at / write_at 落到 Alien 块设备层（RAMDISK / virtio-blk），与
        // diskfs/fat32 同构；不再使用旁路内存映射。
        let dev = dev.ok_or_else(|| vfscore::error::VfsError::Invalid)?;

        // 1) 把块设备 inode 交给块设备后端（BlockDevOpenOptions::open 时取出）。
        set_dbfs_block_device(dev);

        // 2) 用块设备后端打开 JammDB，再交给 dbfs2 做 super_blk 格式化 + cache + 全局注册。
        //    init_dbfs_with 走 spin::Once::call_once，重复 mount 为 no-op，不会清空已建库。
        //    关键：用 DbfsPathLike 作 path，其 exists() 读块设备头部判断 jammdb 是否已初始化，
        //    从而让 DB::open 正确区分「首次格式化」与「复用已有数据」（否则每次 boot 都重格式化，
        //    导致持久化数据被覆盖——见 crash_verify 全 MISSING 的根因）。
        let db = jammdb::DB::open::<BlockDevOpenOptions, _>(Arc::new(BlockDevMap), DbfsPathLike)
            .map_err(|_| vfscore::error::VfsError::IoError)?;
        dbfs2::init_dbfs_with(db).map_err(dbfs_err_to_vfs)?;

        // 首次挂载时由 dbfs_common_root_inode 负责创建 root bucket（副作用必须保留）；
        // 其返回值是**根目录 size（目录项计数）**而非 inode 号，故显式忽略，改用常量
        // DBFS_ROOT_INODE —— 详见该常量的文档（D2 缺陷）。
        let _root_dir_size = dbfs_common_root_inode(0, 0, now()).map_err(dbfs_err_to_vfs)?;
        let root_ino = DBFS_ROOT_INODE;
        let sb = Arc::new(DbfsSuperBlock::new(root_ino, self.clone()));
        DBFS_SB.call_once(|| sb);
        Ok(Arc::new(DbfsDentry::new("/", root_ino, None)))
    }

    fn kill_sb(&self, _sb: Arc<dyn vfscore::superblock::VfsSuperBlock>) -> VfsResult<()> {
        // TODO(Step4): dbfs_common_umount()
        Ok(())
    }

    fn fs_flag(&self) -> FileSystemFlags {
        // 需要设备：挂载时由 vfs init_filesystem 传入 /dev/dbfs 块设备 inode。
        FileSystemFlags::REQUIRES_DEV
    }

    fn fs_name(&self) -> String {
        "dbfs".to_string()
    }
}

//! 类型转换：DBFS2 内部类型 <-> 新版 rvfs 类型（纯函数，零风险）
//! 全部依据 /tmp/proj_check/dbfs2-main/src/common.rs 与 /tmp/rvfs_new/vfscore/src/utils.rs 的真实定义。
use dbfs2::{
    DbfsAttr, DbfsDirEntry, DbfsFileType, DbfsFsStat, DbfsPermission, DbfsTimeSpec as DbfsTs,
};
use vfscore::utils::{
    VfsDirEntry, VfsFileStat, VfsFsStat, VfsNodePerm, VfsNodeType, VfsTimeSpec,
};

/// DbfsFileType（无 Unknown 变体）-> VfsNodeType
pub fn dbfs_ft_to_vfs_node_type(k: DbfsFileType) -> VfsNodeType {
    match k {
        DbfsFileType::NamedPipe => VfsNodeType::Fifo,
        DbfsFileType::CharDevice => VfsNodeType::CharDevice,
        DbfsFileType::BlockDevice => VfsNodeType::BlockDevice,
        DbfsFileType::Directory => VfsNodeType::Dir,
        DbfsFileType::RegularFile => VfsNodeType::File,
        DbfsFileType::Symlink => VfsNodeType::SymLink,
        DbfsFileType::Socket => VfsNodeType::Socket,
    }
}

/// DbfsAttr.perm 含 S_IFMT 类型位；这里只取低 9 位权限
pub fn dbfs_perm_to_vfs_node_perm(p: u16) -> VfsNodePerm {
    VfsNodePerm::from_bits_truncate(p & 0o777)
}

/// VfsNodeType + VfsNodePerm -> DbfsPermission（含 S_IFMT 类型位，供 dbfs_common_create 使用）
pub fn vfs_to_dbfs_permission(ty: VfsNodeType, perm: VfsNodePerm) -> DbfsPermission {
    let ty_bits = match ty {
        VfsNodeType::Dir => DbfsPermission::S_IFDIR,
        VfsNodeType::File => DbfsPermission::S_IFREG,
        VfsNodeType::SymLink => DbfsPermission::S_IFLNK,
        VfsNodeType::CharDevice => DbfsPermission::S_IFCHR,
        VfsNodeType::BlockDevice => DbfsPermission::S_IFBLK,
        VfsNodeType::Fifo => DbfsPermission::S_IFIFO,
        VfsNodeType::Socket => DbfsPermission::S_IFSOCK,
        VfsNodeType::Unknown => DbfsPermission::S_IFREG,
    };
    ty_bits | DbfsPermission::from_bits_truncate(perm.bits())
}

/// DBFS2 nsec:u32 -> Vfs nsec:u64
pub fn dbfs_ts_to_vfs(ts: &DbfsTs) -> VfsTimeSpec {
    VfsTimeSpec {
        sec: ts.sec,
        nsec: ts.nsec as u64,
    }
}

/// DbfsAttr -> VfsFileStat（逐字段，见适配分析.md §4-风险4）
pub fn dbfs_attr_to_vfs_stat(a: &DbfsAttr) -> VfsFileStat {
    VfsFileStat {
        st_dev: 0,
        st_ino: a.ino as u64,
        // DBFS2 的 perm 已含 S_IFMT 类型位（common.rs:161 From<DbfsPermission> for DbfsFileType），直赋即得正确 mode
        st_mode: a.perm as u32,
        st_nlink: a.nlink,
        st_uid: a.uid,
        st_gid: a.gid,
        st_rdev: a.rdev as u64,
        __pad: 0,
        st_size: a.size as u64,
        st_blksize: a.blksize,
        __pad2: 0,
        st_blocks: a.blocks as u64,
        st_atime: dbfs_ts_to_vfs(&a.atime),
        st_mtime: dbfs_ts_to_vfs(&a.mtime),
        st_ctime: dbfs_ts_to_vfs(&a.ctime),
        unused: 0,
    }
}

/// DbfsDirEntry -> VfsDirEntry
pub fn dbfs_direntry_to_vfs(e: &DbfsDirEntry) -> VfsDirEntry {
    VfsDirEntry {
        ino: e.ino,
        ty: dbfs_ft_to_vfs_node_type(e.kind),
        name: e.name.clone(),
    }
}

/// DbfsFsStat -> VfsFsStat（逐字段，见适配分析.md §4-风险6）
pub fn dbfs_fsstat_to_vfs_fsstat(d: &DbfsFsStat) -> VfsFsStat {
    VfsFsStat {
        f_type: d.f_fsid as i64,
        f_bsize: d.f_bsize as i64,
        f_blocks: d.f_blocks,
        f_bfree: d.f_bfree,
        f_bavail: d.f_bavail,
        f_files: d.f_files,
        f_ffree: d.f_ffree,
        f_fsid: [d.f_fsid as i32, 0],
        f_namelen: d.f_namemax as isize,
        f_frsize: d.f_frsize as isize,
        f_flags: d.f_flag as isize,
        f_spare: [0; 4],
    }
}

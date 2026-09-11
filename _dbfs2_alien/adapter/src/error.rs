//! DbfsError -> VfsError 映射（见适配分析.md §4-风险5）
use dbfs2::{DbfsError, DbfsResult};
use vfscore::error::VfsError;
use vfscore::VfsResult;

/// DBFS2 错误 -> 新版 VfsError。
/// VfsError 无 Other/NoData/RangeError/NotSupported 成员，按语义裁断（见注释）。
pub fn dbfs_err_to_vfs(e: DbfsError) -> VfsError {
    match e {
        DbfsError::PermissionDenied => VfsError::PermissionDenied,
        DbfsError::NotFound => VfsError::NoEntry,
        DbfsError::AccessError => VfsError::Access,
        DbfsError::FileExists => VfsError::EExist,
        DbfsError::InvalidArgument => VfsError::Invalid,
        DbfsError::NoSpace => VfsError::NoSpace,
        DbfsError::NameTooLong => VfsError::NameTooLong,
        DbfsError::NoSys => VfsError::NoSys,
        DbfsError::NotEmpty => VfsError::NotEmpty,
        DbfsError::Io => VfsError::IoError,
        // —— 缺口（VfsError 无对应成员，裁断）——
        DbfsError::NoData => VfsError::IoError, // 无数据：当作 IO 错
        DbfsError::RangeError => VfsError::Invalid, // 越界：当作无效参数
        DbfsError::NotSupported => VfsError::NoSys, // 不支持：当作未实现
        DbfsError::Other => VfsError::IoError,    // 兜底：当作 IO 错
    }
}

/// DbfsResult<T> -> VfsResult<T>
pub fn map_res<T>(r: DbfsResult<T>) -> VfsResult<T> {
    r.map_err(dbfs_err_to_vfs)
}

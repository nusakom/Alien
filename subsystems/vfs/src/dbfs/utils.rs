// dbfs/utils.rs

#![no_std]

use crate::db::Db;
use crate::error::VfsError;

pub fn initialize_db() -> Result<Db, VfsError> {
    // 初始化数据库，简单返回一个新的 Db 实例
    Ok(Db::new())
}

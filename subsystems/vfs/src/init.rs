// vfs/src/init.rs
use crate::dbfs2_fs::Dbfs2FS; // 引入你刚刚实现的 dbfs2 文件系统

pub fn init_vfs() {
    // 初始化 dbfs2 文件系统
    let dbfs = Dbfs2FS::init("/path/to/db/database").unwrap();
    // 进行挂载等操作
    dbfs.mount("/mnt/dbfs").unwrap();
}

//! DBFS 模块
//!
//! 本模块用于将 dbfs2 文件系统（基于键值数据库实现的文件系统）接入到你的 VFS 框架中，
//! 同时对外暴露一些 dbfs2 的通用接口。
//!
//! 使用流程大致为：
//! 1. 调用 [`init_dbfs_system`] 打开数据库、初始化超级块，并调用 dbfs2::init_dbfs 初始化全局 DBFS 状态；
//! 2. 通过传入的 VFS 文件系统实例挂载 DBFS 到指定挂载到 `/dbfs`；
//! 3. 其他模块或驱动可调用模块中提供的通用接口进行文件系统操作。
pub mod conversions;

use alloc::sync::Arc;
use vfscore::{dentry::VfsDentry, error::VfsError, fstype::VfsFsType, VfsResult};

// 引入 dbfs2 库相关类型和函数（确保 Cargo.toml 中已配置 dbfs2 的依赖）.
use dbfs2::{
    DB, FileOpenOptions, FakeMap, SLICE_SIZE, DbfsResult, DbfsTimeSpec,
    init_dbfs, dbfs_common_write, dbfs_common_removexattr,
};

/// 初始化 DBFS 系统并将其挂载到传入的 VFS 文件系统上。
///
/// # 参数
/// - `db_path`: 数据库文件的路径，例如 "my-database.db"。
/// - `fs`: 一个实现了 `VfsFsType` 的 DBFS 文件系统实例。
///
/// # 返回
/// 返回 DBFS 挂载后的根目录节点。
///
/// # 示例
/// ```rust
/// let dbfs_root = dbfs::init_dbfs_system("my-database.db", dbfs_fs_instance)?;
/// // 后续可将 dbfs_root 挂载到全局文件系统树中
/// ```
pub fn init_dbfs_system(db_path: &str, fs: Arc<dyn VfsFsType>) -> VfsResult<Arc<dyn VfsDentry>> {
    // 打开数据库
    let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), db_path)
        .map_err(|_| VfsError::Other("Failed to open DB".to_string()))?;
    
    // 初始化超级块（super_blk），设定盘的大小为 16MB（可根据实际情况调整）
    init_db(&db, 16 * 1024 * 1024);

    // 调用 dbfs2 的全局初始化，完成 DBFS 内部的初始化工作
    init_dbfs(db);

    // 挂载 DBFS 到 VFS：调用传入的 fs 实例的 i_mount 接口
    let root = fs
        .i_mount(0, "/dbfs", None, &[])
        .map_err(|e| VfsError::Other(format!("Failed to mount DBFS: {:?}", e)))?;
    println!("DBFS mounted at /dbfs");
    Ok(root)
}

/// 辅助函数：初始化数据库中的超级块结构。
///
/// 若超级块已存在，则直接返回；否则创建并写入必要的元数据。
fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).expect("Failed to create transaction");
    // 检查是否已存在名为 "super_blk" 的 bucket
    let bucket = match tx.get_bucket("super_blk") {
        Ok(_) => return, // 已初始化，无需重复初始化
        Err(_) => tx.create_bucket("super_blk").expect("Failed to create super_blk bucket"),
    };

    // 写入超级块必要的元数据
    bucket
        .put("continue_number", 1usize.to_be_bytes())
        .expect("Failed to put continue_number");
    bucket
        .put("magic", 1111u32.to_be_bytes())
        .expect("Failed to put magic");
    bucket
        .put("blk_size", (SLICE_SIZE as u32).to_be_bytes())
        .expect("Failed to put blk_size");
    bucket
        .put("disk_size", size.to_be_bytes())
        .expect("Failed to put disk_size");
    tx.commit().expect("Failed to commit transaction");
}

/// 封装 dbfs2 提供的通用写接口。
///
/// # 参数
/// - `number`: 表示操作的文件或 inode 编号。
/// - `buf`: 待写入的数据缓冲区。
/// - `offset`: 写入的起始偏移量。
///
/// # 返回
/// 成功时返回写入的字节数；否则返回错误。
pub fn dbfs_common_write(number: usize, buf: &[u8], offset: u64) -> DbfsResult<usize> {
    dbfs_common_write(number, buf, offset)
}

/// 封装 dbfs2 提供的通用移除扩展属性接口。
///
/// # 参数
/// - `r_uid`: 请求操作的用户 id。
/// - `r_gid`: 请求操作的组 id。
/// - `ino`: inode 编号。
/// - `key`: 扩展属性的键名。
/// - `ctime`: 属性修改的时间戳。
///
/// # 返回
/// 操作成功时返回 `()`，否则返回错误。
pub fn dbfs_common_removexattr(
    r_uid: u32,
    r_gid: u32,
    ino: usize,
    key: &str,
    ctime: DbfsTimeSpec,
) -> DbfsResult<()> {
    dbfs_common_removexattr(r_uid, r_gid, ino, key, ctime)
}

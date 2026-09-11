#![cfg_attr(not(test), no_std)]
#![feature(error_in_core)]
extern crate alloc;

mod dir;
mod file;
mod fs_type;
mod inode;

use alloc::{alloc::alloc, sync::Arc};
use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
};

use buddy_system_allocator::LockedHeap;
pub use fs_type::DBFS;
// §5-A: 重导出 dbfs_common_* 后端函数与公共类型，供 dbfs2-adapter 跨 crate 调用。
// 这些符号在各自子模块中以 `pub fn` / `pub struct` 定义，但模块本身为私有，
// 因此在此处统一 `pub use` 暴露，adapter 即可 `use dbfs2::dbfs_common_*`。
// (见 dbfs2-adapter/适配分析.md §5-A)
pub use attr::{
    dbfs_common_chmod, dbfs_common_chown, dbfs_common_getxattr, dbfs_common_listxattr,
    dbfs_common_removexattr, dbfs_common_setxattr, dbfs_common_utimens,
};
pub use common::{
    DbfsAttr, DbfsDirEntry, DbfsError, DbfsFileType, DbfsFsStat, DbfsPermission, DbfsResult,
    DbfsTimeSpec,
};
pub use file::{
    dbfs_common_copy_file_range, dbfs_common_open, dbfs_common_read, dbfs_common_readdir,
    dbfs_common_write,
};
pub use fs_type::{dbfs_common_root_inode, dbfs_common_statfs, dbfs_common_umount};
pub use inode::{
    dbfs_common_access, dbfs_common_attr, dbfs_common_create, dbfs_common_fallocate,
    dbfs_common_link, dbfs_common_lookup, dbfs_common_rename, dbfs_common_rmdir,
    dbfs_common_truncate,
};
pub use link::{dbfs_common_readlink, dbfs_common_unlink};
use jammdb::DB;
use log::error;
use spin::Once;
pub mod extend;
mod attr;
mod common;
mod link;

struct SafeDb(DB);

impl Deref for SafeDb {
    type Target = DB;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SafeDb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl Sync for SafeDb {}
unsafe impl Send for SafeDb {}

static DB: Once<Arc<SafeDb>> = Once::new();

/// Initialize the global DBFS database
pub fn init_dbfs(db: DB) {
    DB.call_once(|| Arc::new(SafeDb(db)));
}

/// 用已经打开好的 `DB` 完成全局初始化（super_blk 格式化 + cache + 注册全局）。
///
/// 与具体后端解耦：调用方负责用任意 JammDB 后端（内存 `memfile` 或块设备 `blockdev`）
/// 打开 `DB`，再交给本函数做 DBFS2 专属的格式化与注册。这样 adapter 既能用内存后端
/// （`init_dbfs_mem`），也能用 Alien 块设备后端（adapter 内 `blockdev` 模块）复用同一套逻辑。
///
/// 步骤（等价 fuse 路径 init_dbfs_fuse）：
/// 1. 仅当 super_blk 缺失时格式化（continue_number=1/magic/blk_size/disk_size，幂等）；
/// 2. init_cache()：dbfs_common_write 的局部写/新建 slice 分支会从私有 BUDDY_ALLOCATOR
///    分配，未初始化会 panic；adapter 不走 rvfs_old dbfs_get_super_blk（那才是原
///    init_cache 触发点），故显式 init；
/// 3. init_dbfs(db) 写全局 Once。
///
/// 重复调用安全：init_dbfs 走 spin::Once::call_once（no-op on re-call）。
pub fn init_dbfs_with(db: DB) -> DbfsResult<()> {
    // 等价 fuse::mkfs::init_db：仅当 super_blk 缺失时才格式化（幂等）。
    let tx = db.tx(true).map_err(DbfsError::from)?;
    if tx.get_bucket("super_blk".as_bytes()).is_err() {
        let bucket = tx.create_bucket("super_blk".as_bytes()).unwrap();
        bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
        bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
        bucket
            .put("blk_size", (SLICE_SIZE as u32).to_be_bytes())
            .unwrap();
        bucket
            .put("disk_size", (16u64 * 1024 * 1024).to_be_bytes())
            .unwrap(); // 16MB
        tx.commit().map_err(DbfsError::from)?;
    } else {
        drop(tx); // 无写路径，仅查询；不 commit 只读 tx

        // D2b: 复用**已存在**的库时，重建 inode 分配器的进度。
        //
        // `DBFS_INODE_NUMBER` 是 RAM-only 的 `AtomicUsize`，没有任何持久化字段，
        // 因此跨 boot 重启后会退回静态初值 1；若不处理，重启后首次 create 会
        // `create_bucket(1)` 撞上 root bucket 而失败（D2b）。
        //
        // 这里不引入新的持久化状态，而是从**已持久化的 inode bucket 名**反推：
        // `next = max(现存 inode 号) + 1`（见 `inode::dbfs_recover_inode_number`）。
        //
        // 顺序硬约束：必须在 `init_dbfs(db)` 发布全局 DB **之前**完成，这样
        // 后续 `dbfs_common_create` 的 `fetch_add` 从一个不与现存 inode 冲突的
        // 号开始；同时空库仍得到 1，满足首次创建 root 时的 `assert_eq!(old, 1)`。
        let next = inode::dbfs_recover_inode_number(&db)?;
        inode::DBFS_INODE_NUMBER.store(next, core::sync::atomic::Ordering::SeqCst);
    }

    init_cache(); // 见上文第 2 点：write 的 BUDDY_ALLOCATOR 前置
    init_dbfs(db);
    Ok(())
}

/// 在 no_std 环境下用 JammDB 自带的内存后端初始化全局 DB（adapter 的 mount 入口之一）。
///
/// 后端 = `jammdb::memfile::MemoryFile`（堆上 alloc/realloc 缓冲）+ `FakeMap`
/// （无真实 mmap，按页号裸指针索引）。不依赖宿主文件系统 / 块设备 / virtio-blk，
/// 满足「内核内只需完成 mount → root → create → write → read」的最小目标。
///
/// 仅当用户态/无块设备环境（如宿主 harness 测试）使用；接 Alien 内核时请改用
/// adapter 内的块设备后端（见 adapter::blockdev + DbfsFs::mount）。
pub fn init_dbfs_mem(name: &str) -> DbfsResult<()> {
    let db = DB::open::<jammdb::memfile::FileOpenOptions, _>(
        Arc::new(jammdb::memfile::FakeMap),
        name,
    )
    .map_err(DbfsError::from)?;

    init_dbfs_with(db)
}

/// DBFS2 事务语义自检（演示 / 验收用，可整体删除）。
///
/// DBFS2 的**每一个文件操作**（create / write / unlink / rename / truncate / chmod ...）
/// 在底层都被实现为一个 JammDB 事务：`db.tx(true)` → 操作 → `tx.commit()`。
/// 存储引擎是 MVCC + 写时复制 B+tree，因此天然具备 ACID 中的
/// A（原子性）、C（一致性）、I（隔离性）、D（持久性）。
///
/// 本函数用 DBFS2 依赖的**同一套事务原语**，在独立 bucket `tx_demo` 上完整走一遍：
///   1. 写事务内写入 `k=v_rollback`，事务内部立即可见；
///   2. **不 commit，直接丢弃该事务**（回滚）—— 之后任何事务都读不到 ⇒ 原子性 A；
///   3. 再开写事务写入 `k=v_commit` 并 **commit** —— 之后任何事务都读得到 ⇒ 持久性 D。
///
/// no_std 环境无法 `println!`，每一步结论通过 `log` 回调交给调用方打印。
pub fn dbfs_tx_selftest<F>(mut log: F) -> DbfsResult<()>
where
    F: FnMut(&str),
{
    let db = clone_db();
    let bname = "tx_demo".as_bytes();
    let key = "demo_key".as_bytes();

    // ---- step1：写事务内写入，事务内部可见 ----
    let in_tx_visible = {
        let tx = db.tx(true).map_err(DbfsError::from)?;
        let v = {
            let bucket = tx.get_or_create_bucket(bname).map_err(DbfsError::from)?;
            bucket
                .put(key, "v_rollback".as_bytes())
                .map_err(DbfsError::from)?;
            bucket
                .get(key)
                .map(|d| d.kv().value() == "v_rollback".as_bytes())
                .unwrap_or(false)
        };
        drop(tx); // 故意不 commit：丢弃即回滚
        v
    };
    log("step1: begin write-tx, put k=v_rollback, read INSIDE tx -> v_rollback");
    if !in_tx_visible {
        log("step1: ERROR - write invisible inside its own tx");
        return Err(DbfsError::Io);
    }

    // ---- step2：未提交即丢弃 ⇒ 回滚，对外不可见（原子性）----
    {
        let tx = db.tx(false).map_err(DbfsError::from)?;
        let visible = match tx.get_bucket(bname) {
            Ok(b) => b
                .get(key)
                .map(|d| d.kv().value() == "v_rollback".as_bytes())
                .unwrap_or(false),
            Err(_) => false,
        };
        drop(tx);
        if visible {
            log("step2: FAIL - rollback did not discard the write");
            return Err(DbfsError::Io);
        }
        log("step2: tx dropped WITHOUT commit -> k is NOT visible  [Atomicity OK]");
    }

    // ---- step3：写入并提交（持久化）----
    {
        let tx = db.tx(true).map_err(DbfsError::from)?;
        {
            let bucket = tx.get_or_create_bucket(bname).map_err(DbfsError::from)?;
            bucket
                .put(key, "v_commit".as_bytes())
                .map_err(DbfsError::from)?;
        }
        tx.commit().map_err(DbfsError::from)?;
        log("step3: begin write-tx, put k=v_commit, then tx.commit()");
    }

    // ---- step4：提交后新事务可见（持久性）----
    {
        let tx = db.tx(false).map_err(DbfsError::from)?;
        let committed = match tx.get_bucket(bname) {
            Ok(b) => b
                .get(key)
                .map(|d| d.kv().value() == "v_commit".as_bytes())
                .unwrap_or(false),
            Err(_) => false,
        };
        drop(tx);
        if !committed {
            log("step4: FAIL - committed value not visible to a new tx");
            return Err(DbfsError::Io);
        }
        log("step4: after commit, a NEW tx reads k=v_commit  [Durability OK]");
    }

    log("tx selftest PASS: DBFS2 tx is atomic (rollback) and durable (commit)");
    Ok(())
}

fn clone_db() -> Arc<SafeDb> {
    DB.get().unwrap().clone()
}

#[macro_export]
macro_rules! u32 {
    ($x:expr) => {
        u32::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! u16 {
    ($x:expr) => {
        u16::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! usize {
    ($x:expr) => {
        usize::from_be_bytes($x.try_into().unwrap())
    };
}
#[macro_export]
macro_rules! u64 {
    ($x:expr) => {
        u64::from_be_bytes($x.try_into().unwrap())
    };
}

#[macro_export]
macro_rules! dbfs_time_spec {
    ($x:expr) => {
        crate::common::DbfsTimeSpec::from($x)
    };
}

#[cfg(feature = "sli512")]
pub const SLICE_SIZE: usize = 512;

#[cfg(feature = "sli1k")]
pub const SLICE_SIZE: usize = 1024;

#[cfg(feature = "sli4k")]
pub const SLICE_SIZE: usize = 4096;

#[cfg(feature = "sli8k")]
pub const SLICE_SIZE: usize = 8192;

#[cfg(feature = "sli32k")]
pub const SLICE_SIZE: usize = 8192 * 2 * 2;

static BUDDY_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();
const MAX_BUF_SIZE: usize = 8 * 1024 * 1024; // 8MB

pub const BUCKET_DATA_SIZE: usize = 128 * 1024 * 1024; // 512

fn init_cache() {
    error!("alloc {}MB for cache", 8);
    unsafe {
        let ptr = alloc(Layout::from_size_align_unchecked(MAX_BUF_SIZE, 8));
        BUDDY_ALLOCATOR.lock().init(ptr as usize, MAX_BUF_SIZE);
    };
    error!("alloc ok");
}

fn copy_data(src: *const u8, dest: *mut u8, len: usize) {
    if src as usize % 16 == 0 && dest as usize % 16 == 0 && len % 16 == 0 {
        unsafe {
            (dest as *mut u128).copy_from_nonoverlapping(src as *const u128, len / 16);
        }
    } else if src as usize % 8 == 0 && dest as usize % 8 == 0 && len % 8 == 0 {
        unsafe {
            (dest as *mut u64).copy_from_nonoverlapping(src as *const u64, len / 8);
        }
    } else if src as usize % 4 == 0 && dest as usize % 4 == 0 && len % 4 == 0 {
        unsafe {
            (dest as *mut u32).copy_from_nonoverlapping(src as *const u32, len / 4);
        }
    } else if src as usize % 2 == 0 && dest as usize % 2 == 0 && len % 2 == 0 {
        unsafe {
            (dest as *mut u16).copy_from_nonoverlapping(src as *const u16, len / 2);
        }
    } else {
        unsafe {
            dest.copy_from_nonoverlapping(src, len);
        }
    }
}

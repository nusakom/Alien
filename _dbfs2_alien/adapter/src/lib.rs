//! dbfs2-adapter：DBFS2 数据库文件系统 -> Alien（os-module/rvfs 0.2.0）薄适配层。
//!
//! 设计目标：Alien 尽量零修改；全部兼容逻辑在 adapter（及 DBFS2 侧微小改动）解决。
//! 详见同目录 `适配分析.md`（含 ① 新版 trait 签名 / ② dbfs_common_* 签名 / ③ 函数级适配表 / ④ 7 风险结论 / ⑤ 构建级阻塞 / ⑥ 改动清单）。
//!
//! 构建：随 Alien workspace 用 nightly 编译（dbfs2 依赖 const_mut_refs / const_weak_new / error_in_core）。
#![no_std]
extern crate alloc;

pub mod blockdev;
pub mod convert;
pub mod dentry;
pub mod error;
pub mod fstype;
pub mod inode;
pub mod superblock;

use dbfs2::DbfsTimeSpec as DbfsTs;

/// 创建/改名等缺少 uid/gid 的 VFS 方法，这里暂以 root(0) 合成；
/// 接 Alien 时应改为取 current_task 的 uid/gid。
pub const DEFAULT_UID: u32 = 0;
pub const DEFAULT_GID: u32 = 0;

/// 时间合成：接 Alien 时应改为取内核时钟（CommonFsProviderImpl::current_time 风格）。
pub(crate) fn now() -> DbfsTs {
    DbfsTs { sec: 0, nsec: 0 }
}

pub use fstype::DbfsFs;
/// 事务语义自检：DBFS2 每个文件操作底层都是 JammDB 事务，这里把演示入口透出给
/// Alien 的 vfs 自检模块（no_std 下 dbfs2 无法 println，结论由回调打印）。
pub use dbfs2::dbfs_tx_selftest;
/// 写放大统计：透出给 Alien vfs 自检 / 性能测试，量化 COW 写放大（逻辑写 vs 物理写）。
pub use blockdev::{take_write_amplify_stats, WriteAmplifyStats};

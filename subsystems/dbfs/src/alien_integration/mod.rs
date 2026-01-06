//! DBFS Alien Integration
//!
//! Phase 2: 事务化文件系统
//!
//! 功能:
//! - ✅ 可以在 Alien OS 中注册和挂载
//! - ✅ 支持基本的 inode 操作: lookup, create, mkdir, read_at, write_at, unlink
//! - ✅ 支持基本的 dentry 操作: insert, remove, parent
//! - ✅ WAL (Write-Ahead Log) 支持
//! - ✅ 事务管理: begin_tx / commit_tx / rollback_tx
//! - ✅ 崩溃恢复

mod dentry;
mod fstype;
mod inode;
mod superblock;

// Test modules - always included for runtime testing
pub mod tests;
pub mod tests_enhanced;
pub mod tests_elle_jepsen;

pub use fstype::DbfsFsType;
pub use inode::{begin_tx, commit_tx, rollback_tx};
pub use superblock::DbfsSuperBlock;

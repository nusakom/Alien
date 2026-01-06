#![feature(error_in_core)]
#![cfg_attr(not(test), no_std)]
extern crate alloc;

// Common modules (no VFS dependency)
#[cfg(feature = "rvfs")]
mod attr;
mod common;
mod inode_common;
#[cfg(feature = "rvfs")]
mod link;

// WAL Transaction Layer
pub mod wal;

// Elle + Jepsen 测试支持
pub mod elle_protocol;
pub mod elle_handler;
pub mod elle_handler_real;

// TCP Server (for Host communication)
#[cfg(feature = "alien_integration")]
pub mod tcp_server;

// WAL Backend v2 - Optional architecture for pluggable backends
// Note: Currently commented out due to dependency issues in no_std environment
// pub mod wal_backend_v2;
// pub use wal_backend_v2 as wal_backend;

// New RVFS2 support
#[cfg(feature = "rvfs2")]
pub mod rvfs2;

// RVFS2 Demo - Minimal proof of concept
#[cfg(feature = "rvfs2_demo")]
pub mod rvfs2_demo;

// Alien Integration - Phase 1: Basic filesystem (no transactions)
#[cfg(feature = "alien_integration")]
pub mod alien_integration;

// Re-export DBFS types for VFS integration
#[cfg(feature = "alien_integration")]
pub use alien_integration::{DbfsFsType, DbfsSuperBlock};

// Re-export transaction functions
#[cfg(feature = "alien_integration")]
pub use alien_integration::{begin_tx, commit_tx, rollback_tx};

// Re-export test runner modules
#[cfg(feature = "alien_integration")]
pub use alien_integration::{tests, tests_enhanced, tests_elle_jepsen};

// Common DBFS functions for both old and new RVFS
#[cfg(feature = "rvfs")]
mod common;

// fs_common is only needed for dbop
#[cfg(feature = "dbop")]
mod fs_common;

// Old RVFS modules (only compile when rvfs feature is available)
#[cfg(feature = "rvfs")]
mod dir;
#[cfg(feature = "rvfs")]
mod file;
#[cfg(feature = "rvfs")]
mod fs_type;
#[cfg(feature = "rvfs")]
mod inode;

use alloc::{alloc::alloc, sync::Arc};
use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
};

use buddy_system_allocator::LockedHeap;

#[cfg(feature = "rvfs")]
pub use fs_type::DBFS;

// jammdb dependency removed - using custom WAL instead
#[cfg(feature = "dbop")]
use jammdb::DB;
use log::error;
use spin::Once;

#[cfg(feature = "dbop")]
pub mod extend;

#[cfg(feature = "dbop")]
pub mod models;

#[cfg(feature = "dbop")]
pub mod log_manager;

#[cfg(feature = "dbop")]
pub mod tx_engine;

#[cfg(feature = "dbop")]
pub mod rvfs_adapter;

#[cfg(all(test, feature = "dbop"))]
mod rvfs_test;
#[cfg(feature = "fuse")]
pub use file::FLAG;

#[cfg(feature = "fuse")]
pub mod fuse;

#[cfg(feature = "fuse")]
extern crate std;

// jammdb-related code - only compile when dbop feature is enabled
#[cfg(feature = "dbop")]
struct SafeDb(DB);

#[cfg(feature = "dbop")]
impl Deref for SafeDb {
    type Target = DB;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "dbop")]
impl DerefMut for SafeDb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "dbop")]
unsafe impl Sync for SafeDb {}
#[cfg(feature = "dbop")]
unsafe impl Send for SafeDb {}

#[cfg(feature = "dbop")]
static DB: Once<Arc<SafeDb>> = Once::new();

/// Initialize the global DBFS database
#[cfg(feature = "dbop")]
pub fn init_dbfs(db: DB) {
    DB.call_once(|| Arc::new(SafeDb(db)));
}

#[cfg(feature = "dbop")]
fn clone_db() -> Arc<SafeDb> {
    DB.get().unwrap().clone()
}

// Stub for when dbop feature is not enabled
#[cfg(not(feature = "dbop"))]
fn clone_db() -> Result<(), &'static str> {
    Err("dbop feature not enabled")
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

/// Run DBFS tests
///
/// This function is called from VFS initialization to run DBFS transaction tests
/// Runs three test suites:
/// 1. Basic WAL tests (5 tests)
/// 2. Enhanced tests (13 tests)
/// 3. Elle + Jepsen tests (10 tests)
#[cfg(feature = "alien_integration")]
pub fn run_dbfs_tests() {
    log::info!("========================================");
    log::info!("DBFS Transactional Filesystem Tests");
    log::info!("========================================");

    let mut total_passed = 0;
    let mut total_tests = 0;

    // Run basic WAL tests
    log::info!("\n📋 Running Basic WAL Tests...");
    let (passed, total) = crate::alien_integration::tests::run_all_tests();
    total_passed += passed;
    total_tests += total;

    // Run enhanced tests
    log::info!("\n📋 Running Enhanced Tests...");
    let (passed, total) = crate::alien_integration::tests_enhanced::run_all_tests();
    total_passed += passed;
    total_tests += total;

    // Run Elle + Jepsen tests
    log::info!("\n📋 Running Elle + Jepsen Tests...");
    let (passed, total) = crate::alien_integration::tests_elle_jepsen::run_elle_jepsen_tests();
    total_passed += passed;
    total_tests += total;

    log::info!("========================================");
    log::info!("DBFS Tests Complete");
    log::info!("========================================");
    log::info!("Final Result: {}/{} tests passed ({:.1}%)",
               total_passed, total_tests,
               (total_passed as f64 / total_tests as f64) * 100.0);

    if total_passed == total_tests {
        log::info!("🎉 All tests passed!");
    } else {
        log::info!("⚠️  {} tests failed", total_tests - total_passed);
    }
    log::info!("========================================");
}

//! DBFS2 最小运行验证（B 方案测试入口，可整体删除）。
//!
//! 目标：在 Alien 内核 FS 子系统就绪后，真的走一遍
//!   FS map 取 "dbfs" -> i_mount(0, "/", None, &[]) -> DbfsFs::mount()
//!   -> init_dbfs_mem() -> root inode -> create/write/read/readdir/unlink
//!
//! 约束遵守：
//! - 不改 vfscore / Alien VFS trait；
//! - 不设计完整 syscall 层；
//! - 只做「Alien VFS 层对象 -> dbfs 后端」的最小运行验证；
//! - 本文件是测试性质，正式适配代码在 dbfs2-adapter，二者分离。
//!
//! 启用：`subsystems/vfs/Cargo.toml` 的 `dbfs_selftest` feature（默认关）。
//! 移除：删 vfs/src/lib.rs 里 `cfg(feature="dbfs_selftest")` 的调用 + 本文件 + Cargo feature。
#![allow(unused_imports)]
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;

use vfscore::error::VfsError;
use vfscore::file::VfsFile;
use vfscore::inode::VfsInode;
use vfscore::utils::{VfsNodePerm, VfsNodeType};
use vfscore::{fstype::VfsFsType, VfsResult};

use crate::FS;

fn dbfs_fs_type() -> Option<Arc<dyn VfsFsType>> {
    FS.lock().get("dbfs").cloned()
}

/// 主入口：挂载 dbfs 并做一轮数据通路操作，全程 println 观测。
pub fn dbfs_selftest() -> VfsResult<()> {
    println!("[dbfs-selftest] begin");
    let fs_type = dbfs_fs_type().ok_or(VfsError::NoDev)?;

    // 1) mount：`DbfsFs::mount` -> init_dbfs_mem -> dbfs_common_root_inode
    //    dev=None（内存后端无需块设备），ab_mnt 传挂载点路径。
    let root_dentry = fs_type.i_mount(0, "/", None, &[])?;
    let root = root_dentry.inode()?;
    println!(
        "[dbfs-selftest] mounted, root type={:?}",
        root.inode_type()
    );

    // 2) create a file
    let perm = VfsNodePerm::from_bits_truncate(0o666);
    let f = root.create("hello.txt", VfsNodeType::File, perm, None)?;
    println!(
        "[dbfs-selftest] created hello.txt, type={:?}",
        f.inode_type()
    );

    // 3) write
    let payload = b"hello from alien dbfs";
    let n = f.write_at(0, payload)?;
    println!("[dbfs-selftest] write {} bytes", n);

    // 4) read back
    let mut buf = [0u8; 64];
    let r = f.read_at(0, &mut buf[..payload.len()])?;
    println!(
        "[dbfs-selftest] read {} bytes = {:?}",
        r,
        core::str::from_utf8(&buf[..r]).unwrap_or("<bad utf8>")
    );
    if &buf[..r] != payload {
        println!("[dbfs-selftest] ERROR: read content mismatch");
        return Err(VfsError::IoError);
    }

    // 5) readdir(root)：应有 hello.txt（以及 . ..）
    let mut names: Vec<String> = Vec::new();
    for idx in 0..32 {
        match root.readdir(idx)? {
            Some(e) => names.push(e.name),
            None => break,
        }
    }
    println!("[dbfs-selftest] readdir(root) = {:?}", names);
    if !names.iter().any(|s| s == "hello.txt") {
        println!("[dbfs-selftest] ERROR: hello.txt not in readdir");
        return Err(VfsError::NoEntry);
    }

    // 6) unlink
    root.unlink("hello.txt")?;
    println!("[dbfs-selftest] unlink hello.txt OK");

    // 7) confirm gone
    let gone = match root.lookup("hello.txt") {
        Ok(_) => false,
        Err(_) => true,
    };
    if gone {
        println!("[dbfs-selftest] lookup after unlink -> error (expected) OK");
    } else {
        println!("[dbfs-selftest] ERROR: hello.txt still exists after unlink");
        return Err(VfsError::EExist);
    }

    println!("[dbfs-selftest] PASS: alien->mount->dbfs create/write/read/readdir/unlink OK");
    Ok(())
}

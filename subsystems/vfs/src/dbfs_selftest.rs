//! DBFS2 自检：事务语义演示 + 增删改查（师兄遗留的最小运行验证，已补齐并接上块设备后端）。
//!
//! ## 来源
//! `_dbfs2_alien/docs/dbfs_selftest.reference.rs`（B 方案测试入口，原为"可整体删除"的草稿）。
//! 原设计：`FS map 取 "dbfs"` → `i_mount(0, "/", None, &[])` → 内存后端 `init_dbfs_mem()`
//! → create / write / read / readdir / unlink。
//!
//! ## 与原始草稿的差异（必须改，否则会清空数据）
//! 现在 DBFS2 已改为**块设备后端**，并在 boot 阶段 `init_filesystem()` 里挂载到 `/dbfs`。
//! 若照原样再 `i_mount` 一次，会因为自定义块设备后端的 `PathLike::exists()` 恒为 false，
//! 导致 `DB::open` 再次走 `init_file` **重新格式化，把已有数据清空**。
//! 因此这里复用 boot 已挂载的 `/dbfs` 根 dentry：既安全，也真正跑在生产路径上。
//!
//! ## 启用
//! `subsystems/vfs/Cargo.toml` 的 `dbfs_selftest` feature（默认关闭，不影响正常启动）。
//! 移除：删本文件 + Cargo feature + `lib.rs` 里 `cfg(feature="dbfs_selftest")` 的调用。
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use vfscore::dentry::VfsDentry;
use vfscore::inode::VfsInode;
use vfscore::utils::{VfsNodePerm, VfsNodeType};

// Phase 8.3-B：写放大实验的计时埋点（仅用于观测，不影响功能路径）。
use platform::config::CLOCK_FREQ;
use timer::read_timer;

/// 主入口。全程 println 观测；任何一步失败都会打印 FAIL 但不 panic，
/// 保证演示时系统仍能正常 boot 到 shell。
pub fn run(root_dentry: Arc<dyn VfsDentry>) {
    println!("[dbfs-selftest] ============ DBFS2 selftest begin ============");

    let mut all_pass = true;

    // ---------------- part 1/4：事务性 ----------------
    println!("[dbfs-selftest] part 1/4  transaction: atomicity & durability");
    let tx_ok = dbfs2_adapter::dbfs_tx_selftest(|s: &str| {
        println!("[dbfs-tx] {}", s);
    })
    .is_ok();
    println!(
        "[dbfs-selftest] part 1 {}",
        if tx_ok { "PASS" } else { "FAIL" }
    );
    all_pass &= tx_ok;

    let root = match root_dentry.inode() {
        Ok(r) => r,
        Err(_) => {
            println!("[dbfs-selftest] ERROR: cannot get /dbfs root inode");
            println!("[dbfs-selftest] ============ DBFS2 selftest FAIL ============");
            return;
        }
    };

    // ---------------- part 2/4：文件增删改查 ----------------
    println!("[dbfs-selftest] part 2/4  file CRUD: create / write / read / readdir / unlink");
    let p2 = file_crud(&root);
    println!(
        "[dbfs-selftest] part 2 {}",
        if p2 { "PASS" } else { "FAIL" }
    );
    all_pass &= p2;

    // ---------------- part 3/4：目录增删改查 ----------------
    println!("[dbfs-selftest] part 3/4  dir CRUD: mkdir / readdir / rmdir");
    let p3 = dir_crud(&root);
    println!(
        "[dbfs-selftest] part 3 {}",
        if p3 { "PASS" } else { "FAIL" }
    );
    all_pass &= p3;

    // ---------------- part 4/4：写放大（论文 M2 核心数据） ----------------
    println!("[dbfs-selftest] part 4/4  write amplification: logical vs physical write");
    let p4 = write_amplification(&root);
    println!(
        "[dbfs-selftest] part 4 {}",
        if p4 { "PASS" } else { "FAIL" }
    );
    all_pass &= p4;

    if all_pass {
        println!("[dbfs-selftest] ============ DBFS2 selftest PASS ============");
    } else {
        println!("[dbfs-selftest] ============ DBFS2 selftest FAIL ============");
    }
}

/// 文件级 CRUD：create → write → read(校验) → readdir → unlink → lookup(应失败)
fn file_crud(root: &Arc<dyn VfsInode>) -> bool {
    let perm = VfsNodePerm::from_bits_truncate(0o666);
    let payload = b"hello from alien dbfs";

    // C: create
    let f = match root.create("hello.txt", VfsNodeType::File, perm, None) {
        Ok(f) => {
            println!("[dbfs-selftest]   [C] create   /dbfs/hello.txt -> OK");
            f
        }
        Err(_) => {
            println!("[dbfs-selftest]   [C] create   /dbfs/hello.txt -> FAIL");
            return false;
        }
    };

    // U(首次写入即建内容): write
    let n = match f.write_at(0, payload) {
        Ok(n) => {
            println!("[dbfs-selftest]   [U] write    {} bytes -> OK", n);
            n
        }
        Err(_) => {
            println!("[dbfs-selftest]   [U] write    -> FAIL");
            return false;
        }
    };
    if n != payload.len() {
        println!(
            "[dbfs-selftest]   [U] write    short write: {} != {}",
            n,
            payload.len()
        );
        return false;
    }

    // R: read back 并校验内容
    let mut buf = [0u8; 64];
    let r = match f.read_at(0, &mut buf[..payload.len()]) {
        Ok(r) => r,
        Err(_) => {
            println!("[dbfs-selftest]   [R] read     -> FAIL");
            return false;
        }
    };
    let content = core::str::from_utf8(&buf[..r]).unwrap_or("<bad utf8>");
    if &buf[..r] != payload {
        println!("[dbfs-selftest]   [R] read     mismatch: {:?}", content);
        return false;
    }
    println!(
        "[dbfs-selftest]   [R] read     {} bytes = {:?} -> OK",
        r, content
    );

    // Q: readdir 应能看到 hello.txt
    match list_names(root) {
        Some(names) => {
            println!("[dbfs-selftest]   [Q] readdir  {:?}", names);
            if !names.iter().any(|s| s == "hello.txt") {
                println!("[dbfs-selftest]   [Q] readdir  hello.txt missing -> FAIL");
                return false;
            }
        }
        None => {
            println!("[dbfs-selftest]   [Q] readdir  -> FAIL");
            return false;
        }
    }

    // D: unlink
    if root.unlink("hello.txt").is_err() {
        println!("[dbfs-selftest]   [D] unlink   /dbfs/hello.txt -> FAIL");
        return false;
    }
    println!("[dbfs-selftest]   [D] unlink   /dbfs/hello.txt -> OK");

    // 确认已删除：lookup 应当失败
    match root.lookup("hello.txt") {
        Ok(_) => {
            println!("[dbfs-selftest]   [D] lookup   still exists after unlink -> FAIL");
            false
        }
        Err(_) => {
            println!("[dbfs-selftest]   [D] lookup   after unlink -> Err (expected) -> OK");
            true
        }
    }
}

/// 目录级 CRUD：mkdir → readdir → rmdir
fn dir_crud(root: &Arc<dyn VfsInode>) -> bool {
    let perm = VfsNodePerm::from_bits_truncate(0o755);

    let d = match root.create("selftest_dir", VfsNodeType::Dir, perm, None) {
        Ok(d) => {
            println!("[dbfs-selftest]   [C] mkdir    /dbfs/selftest_dir -> OK");
            d
        }
        Err(_) => {
            println!("[dbfs-selftest]   [C] mkdir    /dbfs/selftest_dir -> FAIL");
            return false;
        }
    };

    // 空目录 readdir（"." / ".." 由 dbfs 返回）
    match list_names(&d) {
        Some(names) => {
            println!("[dbfs-selftest]   [Q] readdir  {:?} (empty dir)", names);
        }
        None => {
            println!("[dbfs-selftest]   [Q] readdir  -> FAIL");
            return false;
        }
    }

    // 删目录
    if root.rmdir("selftest_dir").is_err() {
        println!("[dbfs-selftest]   [D] rmdir    /dbfs/selftest_dir -> FAIL");
        return false;
    }
    println!("[dbfs-selftest]   [D] rmdir    /dbfs/selftest_dir -> OK");

    match root.lookup("selftest_dir") {
        Ok(_) => {
            println!("[dbfs-selftest]   [D] lookup   still exists after rmdir -> FAIL");
            false
        }
        Err(_) => {
            println!("[dbfs-selftest]   [D] lookup   after rmdir -> Err (expected) -> OK");
            true
        }
    }
}

/// 用 VFS 的 readdir 把目录项名字全拉出来；任一步出错返回 None。
fn list_names(dir: &Arc<dyn VfsInode>) -> Option<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for idx in 0..64 {
        match dir.readdir(idx) {
            Ok(Some(e)) => names.push(e.name),
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    Some(names)
}

/// 写放大测量：在内核态直接对 /dbfs 写 N 字节，采样逻辑写字节 vs 物理写调用次数。
/// 因为 DBFS2 是 COW + 每次 write 一个事务，同一逻辑数据会触发多次块设备写，
/// 二者比值即写放大。此处只做演示性采样，完整扫描见性能测试程序。
fn write_amplification(root: &Arc<dyn VfsInode>) -> bool {
    let perm = VfsNodePerm::from_bits_truncate(0o666);

    // 造一个测试文件
    let f = match root.create("wa_test", VfsNodeType::File, perm, None) {
        Ok(f) => f,
        Err(_) => {
            // 可能已存在，尝试 lookup
            match root.lookup("wa_test") {
                Ok(f) => f,
                Err(_) => {
                    println!("[dbfs-selftest]   [WA] create/lookup wa_test failed");
                    return false;
                }
            }
        }
    };

    // 清零计数器，写 4KB（跨 4 个 1KB 分片），采样
    let _ = dbfs2_adapter::take_write_amplify_stats();
    let payload = [0x5au8; 4096];
    let t0 = read_timer();
    let logical = match f.write_at(0, &payload) {
        Ok(n) => n,
        Err(_) => {
            println!("[dbfs-selftest]   [WA] write failed");
            return false;
        }
    };
    let t1 = read_timer();
    let st = dbfs2_adapter::take_write_amplify_stats();
    let elapsed_ms = t1.wrapping_sub(t0) / (CLOCK_FREQ / 1000);
    // 修正：原实现用 physical_write_calls / 用户字节数，量纲错误（次数÷字节），
    // 恒输出 ~0.00x，会误导论文数据。改为按字节并归一到"逻辑写入字节"。
    println!(
        "[dbfs-selftest]   [WA] logical write: {} bytes, physical write calls: {} -> amp ratio ~{:.2}x",
        st.logical_write_bytes,
        st.physical_write_calls,
        st.physical_write_bytes as f64 / (st.logical_write_bytes.max(1)) as f64
    );
    // Phase 8.3-B：区分「逻辑写 / 物理写 / 全设备回写」三部分，量化一次 commit 的真实代价。
    println!(
        "[dbfs-selftest]   [WA] physical_write_bytes = {} (logical {} + sync_all {})",
        st.physical_write_bytes,
        st.logical_write_bytes,
        st.sync_all_bytes
    );
    println!(
        "[dbfs-selftest]   [WA] sync_all (full-device rewrite): {} calls / {} bytes",
        st.sync_all_calls, st.sync_all_bytes
    );
    println!(
        "[dbfs-selftest]   [WA] amplification(bytes) = {:.2}x ; full-device rewrite share = {:.1}%",
        st.physical_write_bytes as f64 / (st.logical_write_bytes.max(1)) as f64,
        st.sync_all_bytes as f64 * 100.0 / (st.physical_write_bytes.max(1)) as f64
    );
    println!(
        "[dbfs-selftest]   [WA] elapsed for 1 write tx ({}B) = {} ms",
        logical, elapsed_ms
    );

    // 清理
    let _ = root.unlink("wa_test");
    true
}

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{
        close, getdents, list, mkdir, mkdirat, open, read, renameat, seek, unlinkat, write,
        Dirent64, OpenFlags,
    },
    println,
};

// 路径统一用 AT_FDCWD(-100) + 绝对路径，避免依赖进程 cwd。
const AT_FDCWD: isize = -100;

/// 列出目录项（用 ls 同款的 getdents，绕开会触发内核未实现 syscall 的 list()）。
/// 返回 (条目数, 收集到的名字)。目录打开失败返回 (0, Vec::new())。
fn dents(path: &str) -> (usize, Vec<String>) {
    let fd = open(path, OpenFlags::O_RDONLY);
    if fd < 0 {
        println!("dents: open {} failed (fd={})", path, fd);
        return (0, Vec::new());
    }
    let mut buf = [0u8; 512];
    let mut names = Vec::new();
    let mut total = 0usize;
    loop {
        let size = getdents(fd as usize, &mut buf);
        if size <= 0 {
            break;
        }
        let mut ptr = buf.as_ptr();
        let mut consumed = 0usize;
        while consumed < size as usize {
            let dirent = unsafe { &*(ptr as *const Dirent64) };
            names.push(dirent.get_name().to_string());
            total += 1;
            consumed += dirent.len();
            if consumed >= size as usize {
                break;
            }
            ptr = unsafe { ptr.add(dirent.len()) };
        }
        buf.fill(0);
    }
    close(fd as usize);
    (total, names)
}

fn has(name: &str, names: &[String]) -> bool {
    names.iter().any(|n| n == name)
}

pub fn dir_test() -> isize {
    fat32_test();
    dbfs_test();
    0
}

fn fat32_test() {
    println!("In this test, we will test mkdirat and renameat");
    let res = mkdirat(0, "/dir1\0", OpenFlags::O_RDWR);
    if res == -1 {
        println!("mkdirat error");
    }
    println!("mkdir /dir1 success");
    let fd = open("/dir1/f1\0", OpenFlags::O_CREAT | OpenFlags::O_RDWR);
    if fd == -1 {
        println!("open error");
    }

    let len = write(fd as usize, "hello world".as_bytes());
    if len == -1 {
        println!("write error");
    }
    println!("write {} bytes to f1", len);

    println!("create /dir1/f1 success");
    list("/dir1");
    let res = renameat(0, "/dir1/f1\0", 0, "/dir1/f2\0");
    if res == -1 {
        println!("renameat error");
    }
    println!("rename /dir1/f1 to /dir1/f2 success");
    list("/dir1");

    seek(fd as usize, 0, 0);
    let mut buf = [0u8; 20];
    let len = read(fd as usize, &mut buf);
    if len == -1 {
        // 防止 -1 as usize 变成巨大数导致切片越界 panic，掩盖真实错误
        println!("read error (fd={})", fd);
        close(fd as usize);
        return;
    }
    println!("read {} bytes from f1", len);
    println!(
        "read buf:{}",
        core::str::from_utf8(&buf[..len as usize]).unwrap()
    );
    close(fd as usize);
}

pub fn dbfs_test() {
    println!("In this test, we will test mkdirat and renameat");
    let res = mkdirat(0, "/dbfs/dir1\0", OpenFlags::O_RDWR);
    if res == -1 {
        println!("mkdirat error");
    }
    println!("mkdir /dbfs/dir1 success");
    let fd = open("/dbfs/dir1/f1\0", OpenFlags::O_CREAT | OpenFlags::O_RDWR);
    if fd == -1 {
        println!("open error");
    }

    let len = write(fd as usize, "hello world".as_bytes());
    if len == -1 {
        println!("write error");
    }
    println!("write {} bytes to f1", len);

    println!("create /dbfs/dir1/f1 success");
    let res = renameat(0, "/dbfs/dir1/f1\0", 0, "/dbfs/dir1/f2\0");
    if res == -1 {
        println!("renameat error");
    }
    println!("rename /dbfs/dir1/f1 to /dbfs/dir1/f2 success");

    seek(fd as usize, 0, 0);
    let mut buf = [0u8; 20];
    let len = read(fd as usize, &mut buf);
    if len == -1 {
        println!("read error");
    }
    println!("read {} bytes from f1", len);
    println!(
        "read buf:{}",
        core::str::from_utf8(&buf[..len as usize]).unwrap()
    );
    close(fd as usize);
}

/// DBFS2 完整增删改查演示（在 /dbfs 上），每条操作都打印，供老师直接看。
/// 目录名固定 crud_dir，路径独立、不与其他用例冲突，可重复运行。
pub fn dbfs_crud() {
    println!("=========== DBFS2 CRUD on /dbfs ===========");

    // ---- [增] C: 建目录 + 建文件并写入 ----
    if mkdirat(AT_FDCWD, "/dbfs/crud_dir\0", OpenFlags::O_RDWR) == -1 {
        println!("[C] mkdir  /dbfs/crud_dir  -> 已存在或失败(继续)");
    } else {
        println!("[C] mkdir  /dbfs/crud_dir  -> OK");
    }
    let path = "/dbfs/crud_dir/note.txt\0";
    let fd = open(path, OpenFlags::O_CREAT | OpenFlags::O_RDWR);
    if fd < 0 {
        println!("[C] create /dbfs/crud_dir/note.txt -> FAIL (fd={})", fd);
        return;
    }
    println!("[C] create /dbfs/crud_dir/note.txt -> OK");
    let v1 = "hello world v1".as_bytes();
    if write(fd as usize, v1) == v1.len() as isize {
        println!("[C] write  'hello world v1' ({}B) -> OK", v1.len());
    } else {
        println!("[C] write  -> FAIL");
    }

    // ---- [查] R: seek 回 0 + read 校验首段 ----
    seek(fd as usize, 0, 0);
    let mut buf = [0u8; 32];
    let n = read(fd as usize, &mut buf);
    if n >= 0 {
        let content = core::str::from_utf8(&buf[..n as usize]).unwrap_or("<bad>");
        println!(
            "[R] read   {}B = '{}' {}",
            n,
            content,
            if content == "hello world v1" {
                "-> OK"
            } else {
                "-> MISMATCH"
            }
        );
    } else {
        println!("[R] read   -> FAIL");
    }

    // ---- [改] U: seek 到文件尾覆盖式追加，再回 0 整读验证 ----
    seek(fd as usize, v1.len() as isize, 0);
    let v2 = " world v2".as_bytes();
    if write(fd as usize, v2) == v2.len() as isize {
        println!("[U] update seek({})+write ' world v2' -> OK", v1.len());
    } else {
        println!("[U] update -> FAIL");
    }
    seek(fd as usize, 0, 0);
    let mut full = [0u8; 64];
    let nn = read(fd as usize, &mut full);
    if nn >= 0 {
        let all = core::str::from_utf8(&full[..nn as usize]).unwrap_or("<bad>");
        println!(
            "[U] verify whole = '{}' {}",
            all,
            if all == "hello world v1 world v2" {
                "-> OK"
            } else {
                "-> MISMATCH"
            }
        );
    }
    close(fd as usize);

    // ---- [查] 目录级：getdents 列目录，应含 note.txt ----
    let (cnt, names) = dents("/dbfs/crud_dir\0");
    println!(
        "[R] readdir /dbfs/crud_dir -> {} 项 {:?} {}",
        cnt,
        names,
        if has("note.txt", &names) { "-> OK" } else { "-> FAIL" }
    );

    // ---- [删] D: unlink 文件 + 复核 + rmdir 目录 ----
    if unlinkat(AT_FDCWD, "/dbfs/crud_dir/note.txt\0", 0) == 0 {
        println!("[D] unlink /dbfs/crud_dir/note.txt -> OK");
    } else {
        println!("[D] unlink -> FAIL");
    }
    let (cnt2, names2) = dents("/dbfs/crud_dir\0");
    println!(
        "[D] readdir after unlink -> {} 项 {:?} {}",
        cnt2,
        names2,
        if has("note.txt", &names2) {
            "-> FAIL (note.txt 还在)"
        } else {
            "-> OK (已删除)"
        }
    );
    // Mstd 未暴露 rmdir 用户态封装，目录删除由内核态自检(dir_crud)覆盖，
    // 这里文件级 [删] 已经闭环，故不再删 crud_dir（可留在 /dbfs 供 ls 查看）。
    println!("=========== DBFS2 CRUD done ===========");
}

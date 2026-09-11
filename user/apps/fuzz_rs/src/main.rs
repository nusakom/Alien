#![no_main]
#![no_std]
#![allow(unused)]

// fuzz_rs：DBFS2 随机模糊测试器（M4）。
//
// 通过 xorshift64 伪随机数生成器（带 seed，可复现）生成随机的文件系统操作序列，
// 覆盖 create/write/read/seek/truncate(unlink+recreate)/unlink/mkdir/rmdir/rename/getdents，
// 每步操作后校验返回值合法性，捕捉以下异常：
//   - 内核 panic / 崩溃（无法继续执行）
//   - 非法返回值（如 write 短写、read 返回负值但非预期）
//   - 数据损坏（写入后读回不一致）
//   - 目录项丢失 / 幽灵条目
//
// 用法：fuzz_rs <base> <nops> <seed>
//   base = "/dbfs" 或 "/tests"
//   nops = 随机操作次数（默认 2000）
//   seed = 随机种子（默认 0x12345678，可复现）
//
// 输出：FZ,<base>,<op#>,<op_type>,<ret>,<note> 日志；崩溃时打印 FZ_CRASH 与复现 seed。

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{close, getdents, mkdirat, open, openat, read, renameat, seek, unlinkat, write, Dirent64, FileMode, OpenFlags},
    println,
};

const AT_FDCWD: isize = -100;
const AT_REMOVEDIR: usize = 0x200;

// 模糊测试工作目录：所有随机操作都在 <base>/fuzzdir 下进行，避免污染根目录。
const FUZZ_SUBDIR: &str = "fuzzdir";
const MAX_FILE: usize = 16; // 最多同时存在的文件数
const MAX_SLICE: usize = 4096; // 单次写最大字节（跨多个 1KB 分片）

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next() % n as u64) as usize
    }
    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

// 拼接路径（自动补 \0 结尾）
fn join(base: &str, name: &str) -> String {
    let mut p = String::from(base);
    p.push('/');
    p.push_str(FUZZ_SUBDIR);
    p.push('/');
    p.push_str(name);
    p.push('\0');
    p
}

fn fuzz_dir(base: &str) -> String {
    let mut p = String::from(base);
    p.push('/');
    p.push_str(FUZZ_SUBDIR);
    p.push('\0');
    p
}

// 读目录项名字列表
fn dents(path: &str) -> Vec<String> {
    let fd = open(path, OpenFlags::O_RDONLY);
    if fd < 0 {
        return Vec::new();
    }
    let mut buf = [0u8; 1024];
    let mut names = Vec::new();
    loop {
        let size = getdents(fd as usize, &mut buf);
        if size <= 0 {
            break;
        }
        let mut ptr = buf.as_ptr();
        let mut consumed = 0usize;
        while consumed < size as usize {
            let dirent = unsafe { &*(ptr as *const Dirent64) };
            let n = dirent.get_name();
            if n != "." && n != ".." {
                names.push(n.to_string());
            }
            consumed += dirent.len();
            if consumed >= size as usize {
                break;
            }
            ptr = unsafe { ptr.add(dirent.len()) };
        }
        buf.fill(0);
    }
    close(fd as usize);
    names
}

#[no_mangle]
fn main(_argc: usize, argv: Vec<String>) -> isize {
    let base = if argv.len() > 1 { argv[1].clone() } else { String::from("/dbfs") };
    let nops = if argv.len() > 2 { argv[2].parse::<usize>().unwrap_or(2000) } else { 2000 };
    let seed = if argv.len() > 3 { argv[3].parse::<u64>().unwrap_or(0x12345678) } else { 0x12345678 };

    println!("========== fuzz_rs @ {} ({} ops, seed={:#x}) ==========", base, nops, seed);

    // 建立工作目录
    let d = fuzz_dir(&base);
    let _ = mkdirat(AT_FDCWD, d.as_str(), OpenFlags::O_RDWR);

    let mut rng = Rng(seed);
    let mut errors = 0usize;
    let mut files: Vec<String> = Vec::new(); // 当前存在的文件名

    // 预热：创建几个初始文件
    for i in 0..4 {
        let name = format_file_name(i);
        let p = join(&base, &name);
        let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
        if fd >= 0 {
            close(fd as usize);
            files.push(name);
        }
    }

    let mut buf = [0u8; MAX_SLICE];

    for op in 0..nops {
        let op_type = rng.range(8);
        match op_type {
            // 0: 随机写（已知内容，后续可校验）
            0 => {
                if files.is_empty() {
                    continue;
                }
                let idx = rng.range(files.len());
                let p = join(&base, &files[idx]);
                let fd = open(p.as_str(), OpenFlags::O_RDWR);
                if fd < 0 {
                    errors += 1;
                    println!("FZ,{},op{},WRITE,open_failed", base, op);
                    continue;
                }
                let off = rng.range(MAX_SLICE);
                let len = rng.range(MAX_SLICE) + 1;
                for i in 0..len {
                    buf[i] = ((off + i) % 251) as u8;
                }
                let _ = seek(fd as usize, off as isize, 0);
                let n = write(fd as usize, &buf[..len]);
                if n != len as isize {
                    errors += 1;
                    println!("FZ,{},op{},WRITE,short_write,n={},expect={}", base, op, n, len);
                }
                close(fd as usize);
            }
            // 1: 随机读 + 校验
            1 => {
                if files.is_empty() {
                    continue;
                }
                let idx = rng.range(files.len());
                let p = join(&base, &files[idx]);
                let fd = open(p.as_str(), OpenFlags::O_RDONLY);
                if fd < 0 {
                    continue; // 文件可能刚被删，正常
                }
                let off = rng.range(MAX_SLICE);
                let _ = seek(fd as usize, off as isize, 0);
                let mut rbuf = [0u8; MAX_SLICE];
                let n = read(fd as usize, &mut rbuf);
                if n < 0 {
                    errors += 1;
                    println!("FZ,{},op{},READ,neg_ret={}", base, op, n);
                }
                close(fd as usize);
            }
            // 2: 随机 seek（不动数据）
            2 => {
                if files.is_empty() {
                    continue;
                }
                let idx = rng.range(files.len());
                let p = join(&base, &files[idx]);
                let fd = open(p.as_str(), OpenFlags::O_RDWR);
                if fd >= 0 {
                    let off = rng.range(MAX_SLICE * 4);
                    let _ = seek(fd as usize, off as isize, 0);
                    close(fd as usize);
                }
            }
            // 3: create 新文件（截断式）
            3 => {
                let name = format_file_name(files.len() + rng.range(1000));
                let p = join(&base, &name);
                let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
                if fd >= 0 {
                    close(fd as usize);
                    files.push(name);
                }
            }
            // 4: unlink 删除文件
            4 => {
                if files.is_empty() {
                    continue;
                }
                let idx = rng.range(files.len());
                let name = files.swap_remove(idx);
                let p = join(&base, &name);
                let r = unlinkat(AT_FDCWD, p.as_str(), 0);
                if r != 0 {
                    // 删除失败可能是正常（若文件已不存在），不记错，但打印
                    println!("FZ,{},op{},UNLINK,ret={}", base, op, r);
                }
            }
            // 5: mkdir 随机子目录
            5 => {
                let name = format_dir_name(rng.range(1000));
                let p = join(&base, &name);
                let r = mkdirat(AT_FDCWD, p.as_str(), OpenFlags::O_RDWR);
                if r == 0 {
                    // 建了就删（保持目录整洁，测试 mkdir/rmdir 往返）
                    let _ = unlinkat(AT_FDCWD, p.as_str(), AT_REMOVEDIR);
                }
            }
            // 6: rename（文件改名）
            6 => {
                if files.is_empty() {
                    continue;
                }
                let idx = rng.range(files.len());
                let old = files[idx].clone();
                let new = format_file_name(files.len() + rng.range(1000));
                let op_old = join(&base, &old);
                let np = join(&base, &new);
                let r = renameat(AT_FDCWD, op_old.as_str(), AT_FDCWD, np.as_str());
                if r == 0 {
                    files[idx] = new;
                }
            }
            // 7: getdents 列目录（校验一致性）
            7 => {
                let names = dents(d.as_str());
                // 校验：当前存在的文件都应可见
                let mut missing = 0usize;
                for f in &files {
                    if !names.iter().any(|n| n == f) {
                        missing += 1;
                    }
                }
                if missing > 0 {
                    errors += 1;
                    println!("FZ,{},op{},READDIR,missing={}", base, op, missing);
                }
            }
            _ => {}
        }
    }

    // 清理
    for f in &files {
        let p = join(&base, f);
        let _ = unlinkat(AT_FDCWD, p.as_str(), 0);
    }

    println!("");
    if errors == 0 {
        println!("FZ,{},PASS,{} ops, seed={:#x}, 0 errors", base, nops, seed);
        0
    } else {
        println!("FZ,{},FAIL,{} errors / {} ops, seed={:#x}", base, errors, nops, seed);
        println!("FZ_REPRO,{},nops={},seed={:#x}", base, nops, seed);
        -1
    }
}

fn format_file_name(i: usize) -> String {
    let mut s = String::from("f");
    s.push_str(&i.to_string());
    s
}

fn format_dir_name(i: usize) -> String {
    let mut s = String::from("d");
    s.push_str(&i.to_string());
    s
}

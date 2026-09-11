#![no_main]
#![no_std]
#![allow(unused)]

// DBFS2 性能基准（M2）：顺序吞吐 / 随机 IOPS / 延迟 / 扩展性（文件大小·数量·目录深度）。
//
// 计时双通道：rdcycle（纳秒级，主）+ get_time_of_day（微秒级，对照）。
// 输出统一 CSV 行，方便脚本抓取绘图。每个测量跑 REPEAT 次取均值。
//
// 用法：dbfs_perf <base>  其中 base = "/dbfs" 或 "/tests"，做 DBFS2 vs fat32 对照。

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{close, mkdirat, open, read, seek, unlinkat, write, OpenFlags},
    println,
    time::{get_time_of_day, read_cycle, TimeVal},
};

const AT_FDCWD: isize = -100;
const REPEAT: usize = 5; // 每个测量重复次数

// ---- 计时工具：返回 (cycle_delta, usec_delta) ----
struct Stopwatch {
    c0: u64,
    t0: TimeVal,
}

impl Stopwatch {
    fn start() -> Self {
        Self {
            c0: read_cycle(),
            t0: TimeVal::now(),
        }
    }
    fn stop(&self) -> (u64, usize) {
        let c1 = read_cycle();
        let t1 = TimeVal::now();
        let cycles = c1.wrapping_sub(self.c0);
        let usec = (t1.tv_sec - self.t0.tv_sec) * 1_000_000 + (t1.tv_usec - self.t0.tv_usec);
        (cycles, usec)
    }
}

fn path_of(base: &str, sub: &str) -> String {
    let mut p = String::from(base);
    p.push_str("/perf/");
    p.push_str(sub);
    p.push('\0');
    p
}

fn ensure_dir(base: &str) {
    let mut d = String::from(base);
    d.push_str("/perf\0");
    let _ = mkdirat(AT_FDCWD, d.as_str(), OpenFlags::O_RDWR);
}

// ---- 顺序写吞吐 ----
fn seq_write(base: &str, size: usize) {
    ensure_dir(base);
    let p = path_of(base, "seq_w");
    let mut buf = [0xabu8; 4096];
    for i in 0..4096 {
        buf[i] = (i % 251) as u8;
    }
    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    for _ in 0..REPEAT {
        let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
        if fd < 0 {
            println!("PERF,seq_write,{},{},ERR,open failed", base, size);
            return;
        }
        let sw = Stopwatch::start();
        let mut written = 0usize;
        while written < size {
            let chunk = core::cmp::min(4096, size - written);
            let _ = write(fd as usize, &buf[..chunk]);
            written += chunk;
        }
        let (c, u) = sw.stop();
        total_cycles += c;
        total_usec += u;
        close(fd as usize);
    }
    let mb = size as f64 / (1024.0 * 1024.0);
    let sec = (total_usec / REPEAT) as f64 / 1_000_000.0;
    println!(
        "PERF,seq_write,{},{},OK,{:.2},MB/s,cycles={}",
        base, size, mb / sec, total_cycles / REPEAT as u64
    );
}

// ---- 顺序读吞吐 ----
fn seq_read(base: &str, size: usize) {
    ensure_dir(base);
    let p = path_of(base, "seq_r");
    let mut buf = [0xabu8; 4096];
    for i in 0..4096 {
        buf[i] = (i % 251) as u8;
    }
    // 先写一次
    let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
    if fd < 0 {
        println!("PERF,seq_read,{},{},ERR,open failed", base, size);
        return;
    }
    let mut written = 0usize;
    while written < size {
        let chunk = core::cmp::min(4096, size - written);
        let _ = write(fd as usize, &buf[..chunk]);
        written += chunk;
    }
    close(fd as usize);

    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    let mut rbuf = [0u8; 4096];
    for _ in 0..REPEAT {
        let fd = open(p.as_str(), OpenFlags::O_RDONLY);
        if fd < 0 {
            continue;
        }
        let sw = Stopwatch::start();
        let mut readn = 0usize;
        while readn < size {
            let chunk = core::cmp::min(4096, size - readn);
            let r = read(fd as usize, &mut rbuf[..chunk]);
            if r <= 0 {
                break;
            }
            readn += r as usize;
        }
        let (c, u) = sw.stop();
        total_cycles += c;
        total_usec += u;
        close(fd as usize);
    }
    let mb = size as f64 / (1024.0 * 1024.0);
    let sec = (total_usec / REPEAT) as f64 / 1_000_000.0;
    println!(
        "PERF,seq_read,{},{},OK,{:.2},MB/s,cycles={}",
        base, size, mb / sec, total_cycles / REPEAT as u64
    );
}

// ---- 随机写 IOPS ----
fn rand_write(base: &str, size: usize, ops: usize) {
    ensure_dir(base);
    let p = path_of(base, "rand_w");
    let mut buf = [0x5au8; 512];
    let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
    if fd < 0 {
        println!("PERF,rand_write,{},{},ERR,open failed", base, ops);
        return;
    }
    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    let mut rng = 0x12345678u64;
    for _ in 0..REPEAT {
        let sw = Stopwatch::start();
        for _ in 0..ops {
            // 伪随机 offset（xorshift）
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let off = (rng % (size as u64 / 512)) as isize * 512;
            let _ = seek(fd as usize, off, 0);
            let _ = write(fd as usize, &buf);
        }
        let (c, u) = sw.stop();
        total_cycles += c;
        total_usec += u;
    }
    let sec = (total_usec / REPEAT) as f64 / 1_000_000.0;
    let iops = ops as f64 / sec;
    println!(
        "PERF,rand_write,{},{},OK,{:.2},IOPS,cycles={}",
        base, ops, iops, total_cycles / REPEAT as u64
    );
    close(fd as usize);
}

// ---- 随机读 IOPS ----
fn rand_read(base: &str, size: usize, ops: usize) {
    ensure_dir(base);
    let p = path_of(base, "rand_r");
    let mut buf = [0x5au8; 512];
    let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
    if fd < 0 {
        println!("PERF,rand_read,{},{},ERR,open failed", base, ops);
        return;
    }
    // 先填满
    let mut written = 0usize;
    while written < size {
        let chunk = core::cmp::min(512, size - written);
        let _ = write(fd as usize, &buf[..chunk]);
        written += chunk;
    }
    let mut rbuf = [0u8; 512];
    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    let mut rng = 0x87654321u64;
    for _ in 0..REPEAT {
        let sw = Stopwatch::start();
        for _ in 0..ops {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let off = (rng % (size as u64 / 512)) as isize * 512;
            let _ = seek(fd as usize, off, 0);
            let _ = read(fd as usize, &mut rbuf);
        }
        let (c, u) = sw.stop();
        total_cycles += c;
        total_usec += u;
    }
    let sec = (total_usec / REPEAT) as f64 / 1_000_000.0;
    let iops = ops as f64 / sec;
    println!(
        "PERF,rand_read,{},{},OK,{:.2},IOPS,cycles={}",
        base, ops, iops, total_cycles / REPEAT as u64
    );
    close(fd as usize);
}

// ---- 延迟（单次小写操作，测事务开销）----
fn latency_write(base: &str, ops: usize) {
    ensure_dir(base);
    let p = path_of(base, "lat_w");
    let mut buf = [0x33u8; 64];
    let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
    if fd < 0 {
        println!("PERF,latency_write,{},{},ERR,open failed", base, ops);
        return;
    }
    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    for _ in 0..REPEAT {
        let sw = Stopwatch::start();
        for i in 0..ops {
            let _ = seek(fd as usize, (i % 64) as isize * 64, 0);
            let _ = write(fd as usize, &buf);
        }
        let (c, u) = sw.stop();
        total_cycles += c;
        total_usec += u;
    }
    let avg_usec = (total_usec / REPEAT) as f64 / ops as f64;
    let avg_cycles = (total_cycles / REPEAT as u64) as f64 / ops as f64;
    println!(
        "PERF,latency_write,{},{},OK,{:.2},us/op,cycles={:.1}",
        base, ops, avg_usec, avg_cycles
    );
    close(fd as usize);
}

// ---- 扩展性：文件数量 ----
fn scale_files(base: &str, n: usize) {
    ensure_dir(base);
    let mut total_cycles = 0u64;
    let mut total_usec = 0usize;
    let sw = Stopwatch::start();
    for i in 0..n {
        let mut p = String::from(base);
        p.push_str("/perf/sf_");
        p.push_str(&i.to_string());
        p.push('\0');
        let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR);
        if fd >= 0 {
            let _ = write(fd as usize, b"x");
            close(fd as usize);
        }
    }
    let (c, u) = sw.stop();
    total_cycles += c;
    total_usec += u;
    let avg_usec = total_usec as f64 / n as f64;
    println!(
        "PERF,scale_files,{},{},OK,{:.2},us/file,cycles={}",
        base, n, avg_usec, total_cycles
    );
}

// ---- 扩展性：目录深度 ----
fn scale_depth(base: &str, depth: usize) {
    ensure_dir(base);
    let mut cur = String::from(base);
    cur.push_str("/perf");
    let sw = Stopwatch::start();
    for i in 0..depth {
        cur.push_str("/d");
        cur.push_str(&i.to_string());
        let mut c = cur.clone();
        c.push('\0');
        let _ = mkdirat(AT_FDCWD, c.as_str(), OpenFlags::O_RDWR);
    }
    let (c, u) = sw.stop();
    println!(
        "PERF,scale_depth,{},{},OK,{:.2},us,total_cycles={}",
        base, depth, u as f64, c
    );
}

#[no_mangle]
fn main(_argc: usize, argv: Vec<String>) -> isize {
    let base = if argv.len() > 1 {
        argv[1].clone()
    } else {
        String::from("/dbfs")
    };
    println!("========== DBFS2 性能基准 @ {} ==========", base);

    // 顺序吞吐：文件大小扫描
    let sizes = [4096usize, 16384, 65536, 262144, 1048576, 4194304];
    for &s in &sizes {
        seq_write(&base, s);
    }
    for &s in &sizes {
        seq_read(&base, s);
    }

    // 随机 IOPS
    rand_write(&base, 1048576, 1000);
    rand_read(&base, 1048576, 1000);

    // 延迟
    latency_write(&base, 1000);

    // 扩展性
    let counts = [10usize, 100, 500];
    for &n in &counts {
        scale_files(&base, n);
    }
    for &d in &[1usize, 8, 16, 32] {
        scale_depth(&base, d);
    }

    println!("========== DBFS2 性能基准完成 ==========");
    0
}

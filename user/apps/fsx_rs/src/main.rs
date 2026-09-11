#![no_main]
#![no_std]
#![allow(unused)]

// fsx_rs：File System eXerciser 的 Rust 精简版（M3 压力测试）。
//
// 参考经典 fsx 的伪随机文件操作序列模型，对目标文件反复执行
// map/read/write/truncate/seek 的随机组合，并在每次写入后回读校验内容，
// 检测 I/O 路径的正确性（数据损坏 / 短写 / 越界）。
//
// 用法：fsx_rs <base> <nops>   base = "/dbfs" 或 "/tests"

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{close, open, read, seek, unlinkat, write, OpenFlags},
    println,
};

const AT_FDCWD: isize = -100;
const FILE_SIZE: usize = 65536; // 64KB 测试文件

// xorshift64 伪随机数生成器（确定性，可复现）
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
        (self.next() % n as u64) as usize
    }
}

fn path_of(base: &str) -> String {
    let mut p = String::from(base);
    p.push_str("/fsx_test\0");
    p
}

#[no_mangle]
fn main(_argc: usize, argv: Vec<String>) -> isize {
    let base = if argv.len() > 1 {
        argv[1].clone()
    } else {
        String::from("/dbfs")
    };
    let nops = if argv.len() > 2 {
        argv[2].parse::<usize>().unwrap_or(1000)
    } else {
        1000
    };
    println!("========== fsx_rs @ {} ({} ops) ==========", base, nops);

    let p = path_of(&base);
    let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
    if fd < 0 {
        println!("FSX,{},ERR,open failed", base);
        return -1;
    }

    // 初始填满已知内容（offset 作为字节值）
    let mut init = [0u8; 4096];
    let mut written = 0usize;
    while written < FILE_SIZE {
        let chunk = core::cmp::min(4096, FILE_SIZE - written);
        for i in 0..chunk {
            init[i] = ((written + i) % 251) as u8;
        }
        let _ = write(fd as usize, &init[..chunk]);
        written += chunk;
    }

    let mut rng = Rng(0xdeadbeef);
    let mut errors = 0usize;
    let mut buf = [0u8; 4096];

    for op in 0..nops {
        let op_type = rng.range(4);
        match op_type {
            0 => {
                // 随机读 + 校验
                let off = rng.range(FILE_SIZE - 4096);
                let _ = seek(fd as usize, off as isize, 0);
                let n = read(fd as usize, &mut buf);
                if n <= 0 {
                    errors += 1;
                    println!("FSX,op{},read_short,n={}", op, n);
                    continue;
                }
                // 校验内容 = (off + i) % 251
                let mut ok = true;
                for i in 0..n as usize {
                    if buf[i] != ((off + i) % 251) as u8 {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    errors += 1;
                    println!("FSX,op{},read_mismatch,off={}", op, off);
                }
            }
            1 => {
                // 随机写（已知内容）
                let off = rng.range(FILE_SIZE - 4096);
                let len = rng.range(4096) + 1;
                for i in 0..len {
                    buf[i] = ((off + i) % 251) as u8;
                }
                let _ = seek(fd as usize, off as isize, 0);
                let n = write(fd as usize, &buf[..len]);
                if n != len as isize {
                    errors += 1;
                    println!("FSX,op{},write_short,n={},expect={}", op, n, len);
                }
            }
            2 => {
                // 随机 seek（不读不写，仅移动指针）
                let off = rng.range(FILE_SIZE);
                let _ = seek(fd as usize, off as isize, 0);
            }
            3 => {
                // 随机读（不校验）
                let off = rng.range(FILE_SIZE - 4096);
                let _ = seek(fd as usize, off as isize, 0);
                let _ = read(fd as usize, &mut buf);
            }
            _ => {}
        }
    }

    close(fd as usize);
    let _ = unlinkat(AT_FDCWD, p.as_str(), 0);

    if errors == 0 {
        println!("FSX,{},PASS,{} ops, 0 errors", base, nops);
        0
    } else {
        println!("FSX,{},FAIL,{} errors / {} ops", base, errors, nops);
        -1
    }
}

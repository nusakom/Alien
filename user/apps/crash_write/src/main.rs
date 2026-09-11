#![no_main]
#![no_std]
#![allow(unused)]

// crash_write：崩溃一致性测试 · 阶段 1（写入 + 落盘）。
//
// 用法：crash_write <base>
//   base = "/dbfs"（持久化后端下，写入会 write-through 到 virtio-blk 盘）
//
// 功能：向 /dbfs 写入一批带序号的文件（每个文件内容可预测），
// 然后正常退出（不删文件）。数据已通过 write-through 落到持久化盘。
// 之后由测试脚本在写入过程中 kill qemu（模拟断电），再重启跑 crash_verify 校验。
//
// 关键：DBFS2 的每个 write 都是完整事务（db.tx -> put -> commit），且 commit 时
// jammdb 会写 B+tree 页，这些页经 BlockDevFile::write 的 write-through 落盘。
// 因此 commit 过的数据在断电后应完整可读（持久性 D）；未 commit 的应不可见（原子性 A）。

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{close, open, seek, write, OpenFlags},
    println,
};

const N_FILES: usize = 8;
const FILE_PREFIX: &str = "crash";

fn path_of(base: &str, i: usize) -> String {
    let mut p = String::from(base);
    p.push('/');
    p.push_str(FILE_PREFIX);
    p.push_str(&i.to_string());
    p.push('\0');
    p
}

// 生成可预测的内容：文件 i 的内容 = 重复 (i*37+11) 的字节，长度 1024
fn fill(i: usize, buf: &mut [u8; 1024]) {
    let v = (i * 37 + 11) % 251;
    for b in buf.iter_mut() {
        *b = v as u8;
    }
}

#[no_mangle]
fn main(_argc: usize, argv: Vec<String>) -> isize {
    let base = if argv.len() > 1 { argv[1].clone() } else { String::from("/dbfs") };
    println!("========== crash_write @ {} ==========", base);

    let mut buf = [0u8; 1024];

    for i in 0..N_FILES {
        let p = path_of(&base, i);
        let fd = open(p.as_str(), OpenFlags::O_CREAT | OpenFlags::O_RDWR | OpenFlags::O_TRUNC);
        if fd < 0 {
            println!("CRASH_WRITE,file{},open_failed", i);
            continue;
        }
        fill(i, &mut buf);
        let n = write(fd as usize, &buf);
        close(fd as usize);
        println!("CRASH_WRITE,file{},write,{} bytes", i, n);
    }

    println!("CRASH_WRITE,DONE,{} files written to {}", N_FILES, base);
    println!("========== crash_write 完成，等待断电 / 重启 ==========");
    0
}

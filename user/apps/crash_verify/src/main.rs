#![no_main]
#![no_std]
#![allow(unused)]

// crash_verify：崩溃一致性测试 · 阶段 2（重启后校验）。
//
// 用法：crash_verify <base>
//   base = "/dbfs"（持久化后端下，重启后数据应从 virtio-blk 盘读回）
//
// 功能：重启后重新挂载 /dbfs，校验 crash_write 阶段写入的文件是否完整可读。
//   - 若文件内容与预期一致 → 证明 commit 过的数据在断电后完整保留（持久性 D）
//   - 若文件缺失或内容损坏 → 说明存在数据丢失 / torn write（一致性缺陷）
//
// DBFS2 的 MVCC + COW B+tree 保证：只有 commit 的事务才对后续可见，
// 崩溃时未 commit 的写（部分页已落盘、部分未落盘）不会产生 torn write——
// 旧版本数据页仍是完整一致的，新版本数据页要么完整出现要么完全不可见。

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use Mstd::{
    fs::{close, open, read, seek, OpenFlags},
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

// 与 crash_write 相同的可预测内容
fn expected_val(i: usize) -> u8 {
    ((i * 37 + 11) % 251) as u8
}

#[no_mangle]
fn main(_argc: usize, argv: Vec<String>) -> isize {
    let base = if argv.len() > 1 { argv[1].clone() } else { String::from("/dbfs") };
    println!("========== crash_verify @ {} ==========", base);

    let mut ok_files = 0usize;
    let mut bad_files = 0usize;
    let mut missing_files = 0usize;

    for i in 0..N_FILES {
        let p = path_of(&base, i);
        let fd = open(p.as_str(), OpenFlags::O_RDONLY);
        if fd < 0 {
            missing_files += 1;
            println!("CRASH_VERIFY,file{},MISSING", i);
            continue;
        }
        let mut buf = [0u8; 1024];
        let _ = seek(fd as usize, 0, 0);
        let n = read(fd as usize, &mut buf);
        close(fd as usize);

        let exp = expected_val(i);
        let mut all_match = n == 1024;
        if all_match {
            for b in buf.iter().take(1024) {
                if *b != exp {
                    all_match = false;
                    break;
                }
            }
        }

        if all_match {
            ok_files += 1;
            println!("CRASH_VERIFY,file{},OK,content intact", i);
        } else {
            bad_files += 1;
            println!("CRASH_VERIFY,file{},CORRUPT,n={}", i, n);
        }
    }

    println!("");
    println!("CRASH_VERIFY,SUMMARY,ok={},corrupt={},missing={}", ok_files, bad_files, missing_files);

    if bad_files == 0 && missing_files == 0 && ok_files == N_FILES {
        println!("CRASH_CONSISTENCY,PASS,all {} files intact after restart", N_FILES);
        0
    } else {
        println!(
            "CRASH_CONSISTENCY,FAIL,ok={},corrupt={},missing={}",
            ok_files, bad_files, missing_files
        );
        -1
    }
}

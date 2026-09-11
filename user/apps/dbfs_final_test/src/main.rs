#![no_main]
#![no_std]

extern crate alloc;
use alloc::string::ToString;

// DBFS2 一键测试入口 —— 仿 final_test 的 fork+exec 模式。
//
// 三块测试的分工：
//   1. 事务性自检 + 师兄遗留测试（文件/目录 CRUD）
//        => 在内核 boot 时已自动执行，打印 [dbfs-tx] / [dbfs-selftest] 段落。
//   2. 用户态真实读写（dbfs_test）+ 增删改查（dbfs_crud）
//        => 由本程序 exec("./tests") 依次跑，走真实文件 syscall。
//
// 跑完打印 "!DBFS FINISH!" 并 system_shutdown() 自动关机，供无人值守判分。

use Mstd::{
    println,
    process::{exec, exit, fork, waitpid},
    system_shutdown,
    thread::m_yield,
};

#[no_mangle]
fn main() -> isize {
    run_dbfs_tests();
    println!("!DBFS FINISH!");
    system_shutdown();
}

fn run_dbfs_tests() {
    // 每个命令 = exec("./tests") + 一个 argv[1]（tests 程序按 argv 名字分发）。
    // 注意：exec 的 args 数组须以 null 指针结尾（C 风格 argv）。
    let cases: [(&str, &str); 2] = [
        ("dbfs_test", "DBFS2 读写往返 (mkdir/write/read/rename)"),
        ("dbfs_crud", "DBFS2 增删改查 (create/write/read/getdents/unlink)"),
    ];

    for (name, desc) in cases.into_iter() {
        println!("========== [DBFS2] {} ==========", desc);

        // 构造 argv: ["./tests\0", "<name>\0", null]
        let cmd = "./tests\0";
        let mut arg_name = name.to_string();
        arg_name.push('\0');
        let args: [*const u8; 3] = [cmd.as_ptr(), arg_name.as_ptr(), core::ptr::null()];

        let pid = fork();
        if pid == 0 {
            // 子进程：exec 替换为 tests 程序，传入测试名
            exec(cmd, &args, BASH_ENV);
            exit(0);
        } else {
            // 父进程：等待子进程结束
            m_yield();
            let mut exit_code: i32 = 0;
            let _ = waitpid(pid as usize, &mut exit_code);
            if exit_code == 0 {
                println!("[DBFS2] {} -> PASS (exit 0)", name);
            } else {
                println!("[DBFS2] {} -> FAIL (exit {})", name, exit_code);
            }
        }
    }
}

const BASH_ENV: &[*const u8] = &[
    "SHELL=/bash\0".as_ptr(),
    "PWD=/\0".as_ptr(),
    "LOGNAME=root\0".as_ptr(),
    "MOTD_SHOWN=pam\0".as_ptr(),
    "HOME=/root\0".as_ptr(),
    "LANG=C.UTF-8\0".as_ptr(),
    "TERM=vt220\0".as_ptr(),
    "USER=root\0".as_ptr(),
    "SHLVL=0\0".as_ptr(),
    "OLDPWD=/root\0".as_ptr(),
    "PATH=/:/bin:/sbin:/tests\0".as_ptr(),
    "LD_LIBRARY_PATH=/tests:/bin\0".as_ptr(),
    core::ptr::null(),
];

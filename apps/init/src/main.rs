#![no_std]
#![no_main]

use Mstd::println;
use Mstd::process::{exec, fork, wait};
use Mstd::thread::m_yield;

#[no_mangle]
fn main() -> isize {
    if fork() == 0 {
        exec("shell\0",&[0 as *const u8]);
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -1 {
                m_yield();
                continue;
            }
            println!(
                "[Init] Released a process, pid={}, exit_code={}",
                pid, exit_code,
            );
        }
    }
    0
}
#![no_std]
#![no_main]

use Mstd::println;
use Mstd::thread::sleep;
use Mstd::time::get_time_ms;

#[no_mangle]
fn main() -> isize {
    println!("Test sleep....");
    let now_time = get_time_ms();
    sleep(1000);
    let end_time = get_time_ms();
    println!("sleep 1000ms, cost time: {}ms", end_time - now_time);
    0
}
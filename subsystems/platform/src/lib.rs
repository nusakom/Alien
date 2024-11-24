#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
extern crate alloc;

#[macro_use]
pub mod console;

mod common_riscv;
#[cfg(feature = "hifive")]
mod hifive_riscv;
#[cfg(feature = "qemu_riscv")]
mod qemu_riscv;
#[cfg(feature = "vf2")]
mod starfive2_riscv;

use ::config::CPU_NUM;
pub use common_riscv::basic::MachineInfo as PlatformInfo;
use spin::Once;

pub mod logging;

#[cfg(feature = "hifive")]
pub use hifive_riscv::{basic_machine_info, config, console_putchar, set_timer, system_shutdown};
#[cfg(feature = "qemu_riscv")]
pub use qemu_riscv::{basic_machine_info, config, console_putchar, set_timer, system_shutdown};
#[cfg(feature = "vf2")]
pub use starfive2_riscv::{basic_machine_info, config, console_putchar, set_timer, system_shutdown};

use crate::{common_riscv::sbi::hart_start, console::PrePrint};

/// 平台初始化函数
#[no_mangle]
pub fn platform_init(hart_id: usize, dtb: usize) {
    println!("{}", ::config::FLAG);

    // 初始化 DTB
    #[cfg(feature = "hifive")]
    hifive_riscv::init_dtb(None);
    #[cfg(feature = "vf2")]
    starfive2_riscv::init_dtb(None);
    #[cfg(feature = "qemu_riscv")]
    qemu_riscv::init_dtb(Some(dtb));

    // 初始化机器信息
    let machine_info = basic_machine_info();
    MACHINE_INFO.call_once(|| machine_info);

    // 初始化日志
    logging::init_logger();
    preprint::init_print(&PrePrint);

    // 启动其他核（如果启用了 SMP）
    #[cfg(feature = "smp")]
    init_other_hart(hart_id);

    unsafe { main(hart_id) }
}

/// 启动其他核
fn init_other_hart(hart_id: usize) {
    let start_hart = if cfg!(any(feature = "vf2", feature = "hifive")) {
        1
    } else {
        0
    };

    for i in start_hart..CPU_NUM {
        if i != hart_id {
            let res = hart_start(i, _start_secondary as usize, 0);
            assert_eq!(res.error, 0);
        }
    }
}

/// 获取 DTB 指针
pub fn platform_dtb_ptr() -> usize {
    #[cfg(feature = "hifive")]
    return *hifive_riscv::DTB.get().unwrap();
    #[cfg(feature = "vf2")]
    return *starfive2_riscv::DTB.get().unwrap();
    #[cfg(feature = "qemu_riscv")]
    return *qemu_riscv::DTB.get().unwrap();
}

static MACHINE_INFO: Once<PlatformInfo> = Once::new();

/// 获取平台机器信息
pub fn platform_machine_info() -> PlatformInfo {
    MACHINE_INFO.get().unwrap().clone()
}

extern "C" {
    fn main(hart_id: usize);
    fn _start_secondary();
}

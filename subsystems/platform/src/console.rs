use crate::common_riscv::sbi::console_putchar; // 确保路径正确
use core::fmt::{Arguments, Result, Write};
use ksync::Mutex;
use preprint::Print;

/// 系统启动初期使用的输出函数
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        let hart_id = arch::hart_id(); // 获取当前 hart ID
        $crate::console::__print(format_args!("[{}] {}", hart_id, format_args!($($arg)*)));
    }};
}

/// 系统启动初期使用的输出函数
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}

/// 彩色输出
#[macro_export]
macro_rules! println_color {
    ($color:expr, $fmt:expr) => {
        $crate::print!(concat!("\x1b[", $color, "m", $fmt, "\x1b[0m\n"));
    };
    ($color:expr, $fmt:expr, $($arg:tt)*) => {
        $crate::print!(concat!("\x1b[", $color, "m", $fmt, "\x1b[0m\n"), $($arg)*);
    };
}

/// 定义 `Stdout` 结构体
pub struct Stdout;

/// 对 `Stdout` 实现输出的 `Write` Trait
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result {
        s.as_bytes().iter().for_each(|&x| {
            // 使用 SBI 的 `console_putchar` 输出字符
            console_putchar(x);
        });
        Ok(())
    }
}

/// 定义一个全局的 `Mutex<Stdout>` 用于同步控制
static STDOUT: Mutex<Stdout> = Mutex::new(Stdout);

/// 输出函数，供宏调用
#[doc(hidden)]
pub fn __print(args: Arguments) {
    // 使用全局 `STDOUT` 打印格式化字符串
    STDOUT.lock().write_fmt(args).unwrap();
}

/// 系统启动初期的输出函数
/// 使用 SBI 调用输出字符串
pub fn console_write(s: &str) {
    let mut stdout = Stdout;
    stdout.write_str(s).unwrap();
}

/// 定义 `PrePrint` 结构体
pub struct PrePrint;

/// 为 `PrePrint` 实现 `Print` Trait
impl Print for PrePrint {
    fn print(&self, args: Arguments) {
        print!("{}", args);
    }
}

/// 为 `PrePrint` 实现 `Write` Trait
impl Write for PrePrint {
    fn write_str(&mut self, s: &str) -> Result {
        print!("{}", s);
        Ok(())
    }
}

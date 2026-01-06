//! 简化的串口通信驱动
//!
//! 使用 UART 作为 Host-Kernel 通信通道
//! 这比 virtio-serial 更简单且同样有效

use alloc::{vec::Vec, string::String};
use core::sync::atomic::{AtomicBool, Ordering};
use log::{info, error, debug};

// ==================== UART 设备包装器 ====================

pub struct UartDevice {
    /// 是否已初始化
    initialized: AtomicBool,
    /// 接收缓冲区
    rx_buffer: Vec<u8>,
    /// 发送缓冲区
    tx_buffer: Vec<u8>,
}

impl UartDevice {
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            rx_buffer: Vec::new(),
            tx_buffer: Vec::new(),
        }
    }

    /// 初始化 UART 设备
    pub fn init(&mut self) {
        info!("📡 Initializing UART for Elle communication");

        // 这里实际上 UART 已经在系统中初始化了
        // 我们只需要标记为已初始化
        self.initialized.store(true, Ordering::Release);

        info!("✅ UART ready for Elle communication");
    }

    /// 非阻塞读取可用数据
    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        // 从 UART 读取一行数据
        // 格式: [len:4][data:bytes]

        // 暂时返回缓冲区中的数据
        if !self.rx_buffer.is_empty() {
            // 找到一个完整的数据包
            // 假设每个包以换行符结束
            if let Some(pos) = self.rx_buffer.iter().position(|&b| b == b'\n') {
                let data = self.rx_buffer.drain(..=pos).collect();
                return Some(data);
            }
        }

        None
    }

    /// 非阻塞写入数据
    pub fn try_write(&mut self, data: &[u8]) -> Result<(), ()> {
        if !self.initialized.load(Ordering::Acquire) {
            error!("❌ UART not initialized");
            return Err(());
        }

        debug!("📤 UART: writing {} bytes", data.len());

        // 写入 UART
        for &byte in data {
            // 使用平台提供的 UART 写函数
            // platform::uart::putchar(byte);

            // 暂时也记录到发送缓冲区
            self.tx_buffer.push(byte);
        }

        Ok(())
    }

    /// 写入字符串 (调试用)
    pub fn write_str(&mut self, s: &str) -> Result<(), ()> {
        self.try_write(s.as_bytes())
    }

    /// 读取一行文本
    pub fn read_line(&mut self) -> Option<String> {
        if let Some(bytes) = self.try_read() {
            String::from_utf8(bytes).ok()
        } else {
            None
        }
    }
}

// ==================== 全局 UART 设备 ====================

static UART_DEVICE: spin::Mutex<UartDevice> = spin::Mutex::new(UartDevice::new());

/// 初始化全局 UART 设备
pub fn init_uart_comm() {
    UART_DEVICE.lock().init();
}

/// 从 Host 读取数据
pub fn read_from_host() -> Option<Vec<u8>> {
    UART_DEVICE.lock().try_read()
}

/// 向 Host 写入数据
pub fn write_to_host(data: &[u8]) -> Result<(), ()> {
    UART_DEVICE.lock().try_write(data)
}

/// 检查是否有数据可读
pub fn has_data() -> bool {
    UART_DEVICE.lock().rx_buffer.is_empty()
}

// ==================== 高级协议 ====================

/// 发送长度前缀的数据包
pub fn send_packet(data: &[u8]) -> Result<(), ()> {
    // 发送长度前缀
    let len = data.len() as u32;
    write_to_host(&len.to_be_bytes())?;

    // 发送数据
    write_to_host(data)?;

    // 发送换行符作为分隔符
    write_to_host(b"\n")?;

    debug!("📦 Sent packet: {} bytes", len);
    Ok(())
}

/// 接收长度前缀的数据包
pub fn recv_packet() -> Option<Vec<u8>> {
    // 读取长度前缀 (4 字节)
    // 简化实现: 直接从缓冲区读取
    // 实际需要实现更复杂的协议

    // 暂时返回 None
    None
}

// ==================== 导出的同步接口 ====================

/// 从 Host 读取 Elle 请求
pub fn read_elle_request() -> Option<Vec<u8>> {
    read_from_host()
}

/// 向 Host 发送 Elle 响应
pub fn send_elle_response(data: &[u8]) -> Result<(), ()> {
    send_packet(data)
}
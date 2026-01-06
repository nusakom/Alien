//! Virtio-Serial Device Driver
//!
//! 用于 Host Linux 与 Alien 内核之间的通信
//! 支持 DBFS Elle 测试框架

use alloc::{boxed::Box, vec::Vec};
use core::sync::atomic::{AtomicU64, Ordering};

use log::{info, error, debug};

// ==================== Virtio-Serial 设备 ====================

pub struct VirtioSerialDevice {
    /// 设备寄存器基址
    base_addr: usize,
    /// 接收队列
    rx_queue: VirtQueue,
    /// 发送队列
    tx_queue: VirtQueue,
    /// 接收缓冲区
    rx_buffer: Vec<u8>,
    /// 发送缓冲区
    tx_buffer: Vec<u8>,
    /// 接收到的字节数
    rx_count: AtomicU64,
}

/// VirtQueue (简化版)
struct VirtQueue {
    queue_num: u16,
    desc: *const VirtqDesc,
    avail: *const VirtqAvail,
    used: *const VirtqUsed,
}

/// Virtqueue 描述符
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

/// Virtqueue 可用环
#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 1],  // 实际大小由 queue_size 决定
}

/// Virtqueue 已用环
#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 1],
}

#[repr(C)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

impl VirtioSerialDevice {
    /// 创建新的 virtio-serial 设备
    ///
    /// # Safety
    /// 需要确保 base_addr 是有效的 MMIO 地址
    pub unsafe fn new(base_addr: usize) -> Self {
        info!("🔌 Initializing Virtio-Serial device at 0x{:x}", base_addr);

        // TODO: 初始化 virtqueues
        let rx_queue = VirtQueue {
            queue_num: 0,
            desc: core::ptr::null(),
            avail: core::ptr::null(),
            used: core::ptr::null(),
        };

        let tx_queue = VirtQueue {
            queue_num: 1,
            desc: core::ptr::null(),
            avail: core::ptr::null(),
            used: core::ptr::null(),
        };

        Self {
            base_addr,
            rx_queue,
            tx_queue,
            rx_buffer: Vec::with_capacity(4096),
            tx_buffer: Vec::with_capacity(4096),
            rx_count: AtomicU64::new(0),
        }
    }

    /// 非阻塞读取可用数据
    pub fn try_read(&mut self) -> Option<Vec<u8>> {
        // TODO: 从 virtqueue 读取数据
        // 1. 检查 used ring
        // 2. 获取完成的 descriptor
        // 3. 复制数据到 buffer
        // 4. 释放 descriptor 回 avail ring

        // 暂时返回 None (需要实际硬件/模拟器)
        None
    }

    /// 非阻塞写入数据
    pub fn try_write(&mut self, data: &[u8]) -> Result<(), ()> {
        // TODO: 写入数据到 virtqueue
        // 1. 分配 descriptor
        // 2. 设置 addr = data 的物理地址
        // 3. 设置 len = data.len()
        // 4. 添加到 avail ring
        // 5. 通知设备

        debug!("📤 Virtio-Serial: writing {} bytes", data.len());

        // 暂时只记录日志
        self.tx_buffer.extend_from_slice(data);
        Ok(())
    }

    /// 获取接收到的字节数
    pub fn rx_count(&self) -> u64 {
        self.rx_count.load(Ordering::Acquire)
    }

    /// 获取发送缓冲区大小
    pub fn tx_pending(&self) -> usize {
        self.tx_buffer.len()
    }

    /// 清空发送缓冲区
    pub fn flush_tx(&mut self) {
        self.tx_buffer.clear();
    }
}

// ==================== 全局设备实例 ====================

use core::sync::atomic::AtomicBool;

static VIRTIO_SERIAL_ENABLED: AtomicBool = AtomicBool::new(false);
static mut VIRTIO_SERIAL_DEVICE: Option<VirtioSerialDevice> = None;

/// 初始化全局 virtio-serial 设备
pub fn init_virtio_serial(base_addr: usize) {
    unsafe {
        VIRTIO_SERIAL_DEVICE = Some(VirtioSerialDevice::new(base_addr));
        VIRTIO_SERIAL_ENABLED.store(true, Ordering::Release);
        info!("✅ Virtio-Serial initialized");
    }
}

/// 获取全局设备实例
///
/// # Safety
/// 必须在 init_virtio_serial 之后调用
pub unsafe fn get_virtio_serial() -> Option<&'static mut VirtioSerialDevice> {
    if VIRTIO_SERIAL_ENABLED.load(Ordering::Acquire) {
        VIRTIO_SERIAL_DEVICE.as_mut()
    } else {
        None
    }
}

// ==================== 简化实现 (用于测试) ====================

/// 模拟从 Host 读取 (用于开发阶段)
pub fn mock_read_from_host() -> Option<Vec<u8>> {
    // 暂时返回 None,等待真实 virtqueue 实现
    None
}

/// 模拟向 Host 写入 (用于开发阶段)
pub fn mock_write_to_host(data: &[u8]) -> Result<(), ()> {
    // 打印日志模拟发送
    info!("📤 [MOCK] Sending to Host: {} bytes", data.len());

    // 尝试解析为文本 (调试用)
    if let Ok(text) = core::str::from_utf8(data) {
        if text.len() < 200 {
            debug!("  Data: {}", text);
        }
    }

    Ok(())
}

/// 检查是否使用 mock 模式
pub fn is_mock_mode() -> bool {
    // 如果 virtio-serial 未初始化,使用 mock 模式
    !VIRTIO_SERIAL_ENABLED.load(Ordering::Acquire)
}
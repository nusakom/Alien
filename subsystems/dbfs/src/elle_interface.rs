//! DBFS Elle Interface - 内核端 virtio-serial 通信
//!
//! 接收来自 Host Linux 的 DBFS 请求并执行

use alloc::{format, string::String, vec::Vec};
use log::info;

// ==================== 协议定义 (与 Host 共享) ====================

#[derive(Debug, Clone)]
pub enum DbfsOp {
    BeginTx,
    WriteFile { path: String, offset: u64, data: Vec<u8> },
    CreateFile { path: String },
    DeleteFile { path: String },
    Mkdir { path: String },
    Readdir { path: String },
    CommitTx,
    RollbackTx,
}

#[derive(Debug, Clone)]
pub struct DbfsRequest {
    pub tx_id: u64,
    pub op: DbfsOp,
}

#[derive(Debug, Clone)]
pub struct DbfsResponse {
    pub tx_id: u64,
    pub status: i32,
    pub lsn: u64,
    pub data: Vec<u8>,
}

// ==================== Elle 请求处理器 ====================

pub struct ElleRequestHandler {
    // TODO: virtio-serial 设备
}

impl ElleRequestHandler {
    pub fn new() -> Self {
        Self {}
    }

    /// 处理来自 Host 的 Elle 请求
    pub fn handle_request(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("📨 Elle Request: TX-{} {:?}", req.tx_id, req.op);

        // TODO: 调用 DBFS 实际接口
        // 这里需要集成到 alien_integration 模块
        match &req.op {
            DbfsOp::BeginTx => {
                // let tx_id = crate::alien_integration::begin_tx();
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: req.tx_id,  // 暂时用 tx_id
                    data: Vec::new(),
                }
            }

            DbfsOp::WriteFile { path, offset, data } => {
                info!("  Write: {} @{} ({} bytes)", path, offset, data.len());
                // TODO: 调用 DBFS write
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: 0,
                    data: Vec::new(),
                }
            }

            DbfsOp::CreateFile { path } => {
                info!("  Create: {}", path);
                // TODO: 调用 DBFS create
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: 0,
                    data: Vec::new(),
                }
            }

            DbfsOp::Readdir { path } => {
                info!("  Readdir: {}", path);
                // TODO: 调用 DBFS readdir
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: 0,
                    data: Vec::new(),
                }
            }

            DbfsOp::CommitTx => {
                info!("  Commit TX-{}", req.tx_id);
                // TODO: 调用 DBFS commit
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: req.tx_id,
                    data: Vec::new(),
                }
            }

            DbfsOp::RollbackTx => {
                info!("  Rollback TX-{}", req.tx_id);
                // TODO: 调用 DBFS rollback
                DbfsResponse {
                    tx_id: req.tx_id,
                    status: 0,
                    lsn: 0,
                    data: Vec::new(),
                }
            }

            _ => DbfsResponse {
                tx_id: req.tx_id,
                status: -1,  // Unsupported
                lsn: 0,
                data: Vec::new(),
            }
        }
    }

    /// 从 virtio-serial 读取请求并处理
    pub fn run(&self) {
        info!("🚀 Elle Request Handler started");

        loop {
            // TODO: 从 virtio-serial 读取
            // 1. 读取请求字节流
            // 2. 反序列化为 DbfsRequest
            // 3. 调用 handle_request
            // 4. 序列化 DbfsResponse
            // 5. 写回 virtio-serial

            // 暂时避免死循环
            break;
        }
    }
}

// ==================== 使用示例 ====================

/// 在内核初始化时启动 Elle 请求处理器
///
/// 在 kernel/main.rs 中:
///
/// ```rust
/// #[no_mangle]
/// pub extern "C" fn rust_main() {
///     // ... 其他初始化 ...
///
///     #[cfg(feature = "elle_testing")]
///     {
///         use dbfs::elle_interface::ElleRequestHandler;
///         let handler = ElleRequestHandler::new();
///         kernel::spawn(|| handler.run());
///     }
/// }
/// ```
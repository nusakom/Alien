//! DBFS Elle 请求处理器
//!
//! 接收来自 virtio-serial 的请求并调用实际的 DBFS 接口

use alloc::{format, string::String, vec::Vec};
use log::{info, error, debug, warn};

use crate::elle_protocol::{DbfsRequest, DbfsResponse, DbfsOpType, ProtocolError};
use crate::alien_integration::{DbfsSuperBlock, begin_tx, commit_tx, rollback_tx};

/// Elle 请求处理器
pub struct ElleRequestHandler {
    /// DBFS superblock (用于获取当前 DBFS 实例)
    _dbfs: Option<*const DbfsSuperBlock>,
    /// 是否启用 mock 模式
    mock_mode: bool,
}

impl ElleRequestHandler {
    /// 创建新的处理器
    pub fn new() -> Self {
        info!("🎯 Initializing Elle Request Handler");

        // 检查是否有真实的 virtio-serial 设备
        let mock_mode = true;  // 暂时使用 mock 模式

        if mock_mode {
            info!("⚠️  Elle running in MOCK mode (no real virtio-serial)");
        } else {
            info!("✅ Elle running with virtio-serial device");
        }

        Self {
            _dbfs: None,
            mock_mode,
        }
    }

    /// 处理单个请求
    pub fn handle_request(&self, req: &DbfsRequest) -> DbfsResponse {
        debug!("📨 Processing TX-{} {:?}", req.tx_id, req.op_type);

        match req.op_type {
            DbfsOpType::BeginTx => self.handle_begin_tx(req),

            DbfsOpType::WriteFile => self.handle_write_file(req),

            DbfsOpType::CreateFile => self.handle_create_file(req),

            DbfsOpType::DeleteFile => self.handle_delete_file(req),

            DbfsOpType::Mkdir => self.handle_mkdir(req),

            DbfsOpType::Readdir => self.handle_readdir(req),

            DbfsOpType::CommitTx => self.handle_commit_tx(req),

            DbfsOpType::RollbackTx => self.handle_rollback_tx(req),
        }
    }

    /// 处理 BeginTx
    fn handle_begin_tx(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: BEGIN", req.tx_id);

        // TODO: 调用实际的 DBFS begin_tx
        // let tx_id = begin_tx();

        // 暂时返回成功
        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: req.tx_id,  // 暂时用 tx_id 作为 LSN
            data: Vec::new(),
        }
    }

    /// 处理 WriteFile
    fn handle_write_file(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: WRITE {} @{} ({} bytes)",
              req.tx_id, req.path, req.offset, req.data.len());

        // TODO: 调用实际的 DBFS write
        // dbfs.write_at(tx_id, &req.path, req.offset, &req.data);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 CreateFile
    fn handle_create_file(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: CREATE {}", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS create
        // dbfs.create(tx_id, &req.path);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 DeleteFile
    fn handle_delete_file(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: DELETE {}", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS unlink
        // dbfs.unlink(tx_id, &req.path);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 Mkdir
    fn handle_mkdir(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: MKDIR {}", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS mkdir
        // dbfs.mkdir(tx_id, &req.path, 0o755);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 Readdir
    fn handle_readdir(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: READDIR {}", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS readdir
        // let entries = dbfs.readdir(tx_id, &req.path);

        // 暂时返回空目录
        let entries_json = b"[]".to_vec();

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: entries_json,
        }
    }

    /// 处理 CommitTx
    fn handle_commit_tx(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: COMMIT", req.tx_id);

        // TODO: 调用实际的 DBFS commit
        // commit_tx(req.tx_id)?;

        // 暂时返回成功
        let lsn = req.tx_id;  // 事务 ID 作为 LSN

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn,
            data: Vec::new(),
        }
    }

    /// 处理 RollbackTx
    fn handle_rollback_tx(&self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: ROLLBACK", req.tx_id);

        // TODO: 调用实际的 DBFS rollback
        // rollback_tx(req.tx_id);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 主循环: 处理所有传入的请求
    pub fn run(&self) {
        info!("🚀 Elle Request Handler started");

        if self.mock_mode {
            self.run_mock();
        } else {
            self.run_real();
        }
    }

    /// Mock 模式: 模拟处理请求 (用于测试)
    fn run_mock(&self) {
        info!("📭 Running in MOCK mode");

        // 模拟处理一些请求
        let mock_requests: alloc::vec::Vec<DbfsRequest> = alloc::vec![
            DbfsRequest {
                tx_id: 1,
                op_type: DbfsOpType::BeginTx,
                path: String::new(),
                offset: 0,
                data: Vec::new(),
            },
            DbfsRequest {
                tx_id: 1,
                op_type: DbfsOpType::Readdir,
                path: String::from("/"),
                offset: 0,
                data: Vec::new(),
            },
            DbfsRequest {
                tx_id: 1,
                op_type: DbfsOpType::CreateFile,
                path: String::from("/test.txt"),
                offset: 0,
                data: Vec::new(),
            },
            DbfsRequest {
                tx_id: 1,
                op_type: DbfsOpType::CommitTx,
                path: String::new(),
                offset: 0,
                data: Vec::new(),
            },
        ];

        for req in mock_requests {
            let resp = self.handle_request(&req);

            // 序列化响应
            let resp_bytes = resp.serialize();
            info!("📤 Response: {} bytes", resp_bytes.len());

            // TODO: 发送回 Host
        }

        info!("✅ Mock test completed");
    }

    /// 真实模式: 从 virtio-serial 读取并处理请求
    fn run_real(&self) {
        info!("📭 Running in REAL mode with virtio-serial");

        // TODO: 实现 virtio-serial 轮询循环
        loop {
            // 1. 从 virtio-serial 读取请求字节流
            // let req_bytes = virtio_serial.read()?;

            // 2. 反序列化请求
            // let req = DbfsRequest::deserialize(&req_bytes)?;

            // 3. 处理请求
            // let resp = self.handle_request(&req);

            // 4. 序列化响应
            // let resp_bytes = resp.serialize();

            // 5. 写回 virtio-serial
            // virtio_serial.write(&resp_bytes)?;

            // 暂时避免死循环
            break;
        }
    }
}

// ==================== 全局处理器 ====================

use core::sync::atomic::AtomicBool;

static ELLE_HANDLER_ENABLED: AtomicBool = AtomicBool::new(false);
static mut ELLE_HANDLER: Option<ElleRequestHandler> = None;

/// 初始化全局 Elle 处理器
pub fn init_elle_handler() {
    unsafe {
        ELLE_HANDLER = Some(ElleRequestHandler::new());
        ELLE_HANDLER_ENABLED.store(true, core::sync::atomic::Ordering::Release);
        info!("✅ Elle Handler initialized");
    }
}

/// 获取全局处理器实例
///
/// # Safety
/// 必须在 init_elle_handler 之后调用
pub unsafe fn get_elle_handler() -> Option<&'static ElleRequestHandler> {
    if ELLE_HANDLER_ENABLED.load(core::sync::atomic::Ordering::Acquire) {
        ELLE_HANDLER.as_ref()
    } else {
        None
    }
}

/// 运行 Elle 处理器 (在独立的内核线程中调用)
pub fn run_elle_handler() {
    unsafe {
        if let Some(handler) = get_elle_handler() {
            handler.run();
        } else {
            error!("❌ Elle Handler not initialized");
        }
    }
}
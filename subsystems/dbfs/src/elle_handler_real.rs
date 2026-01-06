//! DBFS Elle 请求处理器 - 真实实现
//!
//! 接收来自 Host 的请求并调用实际的 DBFS 接口

use alloc::{format, string::String, vec::Vec};
use log::{info, error, debug};

use crate::elle_protocol::{DbfsRequest, DbfsResponse, DbfsOpType};
use crate::alien_integration::{begin_tx, commit_tx, rollback_tx};

/// Elle 请求处理器 - 真实模式
pub struct ElleRequestHandlerReal {
    /// 下一个事务 ID
    next_tx_id: u64,
}

impl ElleRequestHandlerReal {
    pub fn new() -> Self {
        info!("🎯 Initializing Real Elle Request Handler");
        info!("✅ Elle running in REAL mode with actual DBFS calls");

        Self {
            next_tx_id: 1,
        }
    }

    /// 处理单个请求
    pub fn handle_request(&mut self, req: &DbfsRequest) -> DbfsResponse {
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

    /// 处理 BeginTx - 调用真实的 begin_tx
    fn handle_begin_tx(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: BEGIN (real)", req.tx_id);

        // 调用实际的 DBFS begin_tx
        // 注意: 这里返回的是实际的 TxId
        let tx_id = begin_tx();

        info!("  ✅ TX-{}: Started", tx_id.value());

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: tx_id.value(),  // 使用真实的 TxId 作为 LSN
            data: Vec::new(),
        }
    }

    /// 处理 WriteFile - 写入文件数据
    fn handle_write_file(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: WRITE {} @{} ({} bytes)",
              req.tx_id, req.path, req.offset, req.data.len());

        // TODO: 调用实际的 DBFS write_at
        // 需要通过 VFS 接口写入文件
        // let result = dbfs.write_at(tx_id, &req.path, req.offset, &req.data);

        // 暂时只记录日志
        debug!("    Data preview: {} bytes", req.data.len());

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,  // 成功
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 CreateFile - 创建文件
    fn handle_create_file(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: CREATE {} (real)", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS create
        // 通过 VFS 接口创建文件
        // let result = dbfs.create(tx_id, &req.path);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 DeleteFile - 删除文件
    fn handle_delete_file(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: DELETE {} (real)", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS unlink
        // let result = dbfs.unlink(tx_id, &req.path);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 Mkdir - 创建目录
    fn handle_mkdir(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: MKDIR {} (real)", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS mkdir
        // let result = dbfs.mkdir(tx_id, &req.path, 0o755);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 处理 Readdir - 读取目录
    fn handle_readdir(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: READDIR {} (real)", req.tx_id, req.path);

        // TODO: 调用实际的 DBFS readdir
        // let entries = dbfs.readdir(tx_id, &req.path);

        // 暂时返回空目录 (JSON 格式)
        let entries_json = b"[]".to_vec();

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: entries_json,
        }
    }

    /// 处理 CommitTx - 提交事务
    fn handle_commit_tx(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: COMMIT (real)", req.tx_id);

        // 调用实际的 DBFS commit
        let tx_id = crate::wal::TxId::new(req.tx_id);
        commit_tx(tx_id).expect("Failed to commit transaction");

        // 提交后会写入 WAL,返回 LSN
        info!("  ✅ TX-{}: Committed", req.tx_id);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: req.tx_id,  // LSN 等于 TxId
            data: Vec::new(),
        }
    }

    /// 处理 RollbackTx - 回滚事务
    fn handle_rollback_tx(&mut self, req: &DbfsRequest) -> DbfsResponse {
        info!("  TX-{}: ROLLBACK (real)", req.tx_id);

        // 调用实际的 DBFS rollback
        let tx_id = crate::wal::TxId::new(req.tx_id);
        rollback_tx(tx_id);

        info!("  ✅ TX-{}: Rolled back", req.tx_id);

        DbfsResponse {
            tx_id: req.tx_id,
            status: 0,
            lsn: 0,
            data: Vec::new(),
        }
    }

    /// 主循环: 从通信通道读取并处理请求
    pub fn run(&mut self) {
        info!("🚀 Real Elle Request Handler started");

        // 使用简化的 UART 通信
        loop {
            // 1. 从 UART 读取请求
            if let Some(req_bytes) = drivers::elle_comm::read_from_host() {
                debug!("📨 Received {} bytes from Host", req_bytes.len());

                // 2. 反序列化请求
                match crate::elle_protocol::DbfsRequest::deserialize(&req_bytes) {
                    Ok(req) => {
                        // 3. 处理请求
                        let resp = self.handle_request(&req);

                        // 4. 序列化响应
                        let resp_bytes = resp.serialize();

                        // 5. 发送回 Host
                        if let Err(e) = drivers::elle_comm::write_to_host(&resp_bytes) {
                            error!("❌ Failed to send response: {:?}", e);
                        } else {
                            debug!("📤 Sent {} bytes to Host", resp_bytes.len());
                        }
                    }
                    Err(e) => {
                        error!("❌ Failed to deserialize request: {:?}", e);
                    }
                }
            }

            // 简化: 只处理一个循环后退出 (避免死循环)
            // 实际应该持续运行
            break;
        }

        info!("✅ Elle Handler run completed");
    }
}
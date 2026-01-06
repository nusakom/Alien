//! Elle DBFS Client - 真正的 Elle + Jepsen 测试客户端
//!
//! 运行在 Host Linux 上,通过 socket 与 Alien 内核中的 DBFS 通信

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Serialize, Deserialize};

// 引入 socket 客户端
mod dbfs_client;
use dbfs_client::{DbfsClient, DbfsOpType, DbfsRequest, DbfsResponse};

// ==================== Async DBFS 客户端封装 ====================

pub struct AsyncDbfsClient {
    client: Arc<Mutex<dbfs_client::DbfsClient>>,
    next_tx_id: Arc<AtomicU64>,
}

impl AsyncDbfsClient {
    pub async fn new(addr: &str) -> anyhow::Result<Self> {
        // 在单独的线程中运行阻塞的 socket 客户端
        let addr = addr.to_string();
        let client = tokio::task::spawn_blocking(move || {
            dbfs_client::DbfsClient::connect(&addr)
        }).await??;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            next_tx_id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn begin_tx(&self) -> anyhow::Result<u64> {
        let tx_id = self.next_tx_id.fetch_add(1, Ordering::SeqCst);

        let client = self.client.clone();
        let resp = tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.begin_tx(tx_id)
        }).await??;

        println!("TX-{}: begin", tx_id);
        Ok(resp.lsn)
    }

    pub async fn write_file(&self, tx_id: u64, path: &str, offset: u64, data: Vec<u8>) -> anyhow::Result<()> {
        println!("TX-{}: write {} @{} ({} bytes)", tx_id, path, offset, data.len());

        let client = self.client.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.write_file(tx_id, &path, offset, &data)
        }).await??;

        Ok(())
    }

    pub async fn create_file(&self, tx_id: u64, path: &str) -> anyhow::Result<()> {
        println!("TX-{}: create {}", tx_id, path);

        let client = self.client.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.create_file(tx_id, &path)
        }).await??;

        Ok(())
    }

    pub async fn readdir(&self, tx_id: u64, path: &str) -> anyhow::Result<Vec<String>> {
        println!("TX-{}: readdir {}", tx_id, path);

        let client = self.client.clone();
        let path = path.to_string();
        let resp = tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.readdir(tx_id, &path)
        }).await??;

        // 从响应数据中解析文件列表
        // 假设响应数据是 JSON 编码的字符串数组
        if resp.data.is_empty() {
            return Ok(vec![]);
        }

        let files: Vec<String> = serde_json::from_slice(&resp.data)
            .unwrap_or_else(|_| vec![]);

        Ok(files)
    }

    pub async fn commit_tx(&self, tx_id: u64) -> anyhow::Result<u64> {
        println!("TX-{}: commit", tx_id);

        let client = self.client.clone();
        let resp = tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.commit_tx(tx_id)
        }).await??;

        Ok(resp.lsn)
    }

    pub async fn rollback_tx(&self, tx_id: u64) -> anyhow::Result<()> {
        println!("TX-{}: rollback", tx_id);

        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            let mut client = client.lock().unwrap();
            client.rollback_tx(tx_id)
        }).await??;

        Ok(())
    }
}

// ==================== Elle 操作历史 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElleOp {
    pub tx_id: u64,
    pub op_type: String,  // "begin", "read", "write", "commit"
    pub path: Option<String>,
    pub lsn: Option<u64>,
    pub timestamp: u64,  // Unix timestamp (seconds)
}

pub struct ElleHistory {
    ops: Arc<Mutex<Vec<ElleOp>>>,
}

impl ElleHistory {
    pub fn new() -> Self {
        Self {
            ops: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn record(&self, op: ElleOp) {
        let mut ops = self.ops.lock().unwrap();
        ops.push(op);
    }

    pub async fn export(&self, path: &str) -> anyhow::Result<()> {
        let ops = self.ops.lock().unwrap();
        let json = serde_json::to_string_pretty(&*ops)?;
        tokio::fs::write(path, json).await?;
        println!("History exported to {}", path);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.ops.lock().unwrap().len()
    }
}

// ==================== Elle 测试工作流 ====================

pub async fn run_elle_test(
    num_ops: usize,
    concurrency: usize,
    addr: &str,
) -> anyhow::Result<()> {
    println!("========================================");
    println!("Elle DBFS Test Starting");
    println!("Target: {}", addr);
    println!("Operations: {}", num_ops);
    println!("Concurrency: {}", concurrency);
    println!("========================================");

    let history = Arc::new(ElleHistory::new());
    let mut tasks = Vec::new();

    // 启动并发任务 - 每个任务独立的连接
    for task_id in 0..concurrency {
        let history = history.clone();
        let addr = addr.to_string();

        let handle = tokio::spawn(async move {
            let ops_per_task = num_ops / concurrency;

            // 每个任务创建独立的客户端连接
            let client = match AsyncDbfsClient::new(&addr).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Task {}: Failed to connect: {:?}", task_id, e);
                    return Err(anyhow::anyhow!("Connection failed"));
                }
            };

            for i in 0..ops_per_task {
                // 记录开始时间 (Unix timestamp in seconds)
                let start = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Begin TX
                let tx_id = match client.begin_tx().await {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("Task {} op {}: begin_tx failed: {:?}", task_id, i, e);
                        continue;
                    }
                };

                history.record(ElleOp {
                    tx_id: tx_id,
                    op_type: "begin".to_string(),
                    path: None,
                    lsn: None,
                    timestamp: start,
                });

                // Readdir (模拟 List-Append 的 read)
                match client.readdir(tx_id, "/").await {
                    Ok(_files) => {},
                    Err(e) => {
                        eprintln!("Task {} op {}: readdir failed: {:?}", task_id, i, e);
                        continue;
                    }
                };

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                history.record(ElleOp {
                    tx_id: tx_id,
                    op_type: "read".to_string(),
                    path: Some("/".to_string()),
                    lsn: None,
                    timestamp: now,
                });

                // Create file (模拟 List-Append 的 append)
                let new_file = format!("/file-{}-{}", task_id, i);
                if let Err(e) = client.create_file(tx_id, &new_file).await {
                    eprintln!("Task {} op {}: create failed: {:?}", task_id, i, e);
                    continue;
                }

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                history.record(ElleOp {
                    tx_id: tx_id,
                    op_type: "write".to_string(),
                    path: Some(new_file),
                    lsn: None,
                    timestamp: now,
                });

                // Commit TX
                let lsn = match client.commit_tx(tx_id).await {
                    Ok(lsn) => lsn,
                    Err(e) => {
                        eprintln!("Task {} op {}: commit failed: {:?}", task_id, i, e);
                        continue;
                    }
                };

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                history.record(ElleOp {
                    tx_id: tx_id,
                    op_type: "commit".to_string(),
                    path: None,
                    lsn: Some(lsn),
                    timestamp: now,
                });

                // 每 100 次操作报告一次
                if (i + 1) % 100 == 0 {
                    println!("Task {}: completed {}/{} ops", task_id, i + 1, ops_per_task);
                }
            }

            Ok::<(), anyhow::Error>(())
        });

        tasks.push(handle);
    }

    // 等待所有任务完成
    for handle in tasks {
        handle.await??;
    }

    println!("========================================");
    println!("Elle Test Complete");
    println!("Total operations: {}", history.len());
    println!("========================================");

    // 导出历史
    history.export("history.json").await?;

    // TODO: 调用 Elle checker
    // let report = elle::check(&history).await?;
    // println!("Elle Report: {:#?}", report);

    Ok(())
}

// ==================== Main ====================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Elle DBFS Client v0.1.0");
    println!("Testing Alien Kernel DBFS with Elle framework");

    // 连接地址 (通过 virtio-serial 或 TCP socket)
    // 使用 localhost 作为测试地址
    let addr = "127.0.0.1:12345";

    println!("Connecting to Alien kernel at {}", addr);

    // 运行 Elle 测试
    // 参数: 50000 个操作, 200 个并发客户端
    run_elle_test(50000, 200, addr).await?;

    Ok(())
}
//! DBFS Client - 与内核通信的客户端
//!
//! 运行在 Host Linux 上,通过 socket 与 Alien 内核通信

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ==================== 协议定义 (与内核同步) ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DbfsOpType {
    BeginTx = 1,
    WriteFile = 2,
    CreateFile = 3,
    DeleteFile = 4,
    Mkdir = 5,
    Readdir = 6,
    CommitTx = 7,
    RollbackTx = 8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbfsRequest {
    pub tx_id: u64,
    pub op_type: DbfsOpType,
    pub path: String,
    pub offset: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbfsResponse {
    pub tx_id: u64,
    pub status: i32,
    pub lsn: u64,
    pub data: Vec<u8>,
}

// ==================== DBFS 客户端 ====================

pub struct DbfsClient {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

impl DbfsClient {
    /// 连接到 Alien 内核
    pub fn connect(addr: &str) -> Result<Self, anyhow::Error> {
        println!("🔌 Connecting to Alien kernel at {}", addr);

        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;

        let reader = BufReader::new(stream.try_clone()?);
        let writer = BufWriter::new(stream);

        println!("✅ Connected to Alien kernel");

        Ok(Self {
            reader,
            writer,
        })
    }

    /// 发送请求
    fn send_request(&mut self, req: &DbfsRequest) -> Result<(), anyhow::Error> {
        // 序列化
        let bytes = bincode::serialize(&req)?;

        // 发送长度前缀
        let len = bytes.len() as u32;
        self.writer.write_all(&len.to_be_bytes())?;

        // 发送数据
        self.writer.write_all(&bytes)?;
        self.writer.flush()?;

        Ok(())
    }

    /// 接收响应
    fn recv_response(&mut self) -> Result<DbfsResponse, anyhow::Error> {
        // 读取长度前缀
        let mut len_bytes = [0u8; 4];
        self.reader.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;

        // 读取数据
        let mut data = vec![0u8; len];
        self.reader.read_exact(&mut data)?;

        // 反序列化
        let resp: DbfsResponse = bincode::deserialize(&data)?;

        Ok(resp)
    }

    /// 发送请求并接收响应
    fn call(&mut self, req: DbfsRequest) -> Result<DbfsResponse, anyhow::Error> {
        self.send_request(&req)?;
        self.recv_response()
    }

    // ==================== DBFS 操作 ====================

    pub fn begin_tx(&mut self, tx_id: u64) -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::BeginTx,
            path: String::new(),
            offset: 0,
            data: Vec::new(),
        };

        self.call(req)
    }

    pub fn write_file(&mut self, tx_id: u64, path: &str, offset: u64, data: &[u8])
        -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::WriteFile,
            path: path.to_string(),
            offset,
            data: data.to_vec(),
        };

        self.call(req)
    }

    pub fn create_file(&mut self, tx_id: u64, path: &str) -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::CreateFile,
            path: path.to_string(),
            offset: 0,
            data: Vec::new(),
        };

        self.call(req)
    }

    pub fn readdir(&mut self, tx_id: u64, path: &str) -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::Readdir,
            path: path.to_string(),
            offset: 0,
            data: Vec::new(),
        };

        self.call(req)
    }

    pub fn commit_tx(&mut self, tx_id: u64) -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::CommitTx,
            path: String::new(),
            offset: 0,
            data: Vec::new(),
        };

        self.call(req)
    }

    pub fn rollback_tx(&mut self, tx_id: u64) -> Result<DbfsResponse, anyhow::Error> {
        let req = DbfsRequest {
            tx_id,
            op_type: DbfsOpType::RollbackTx,
            path: String::new(),
            offset: 0,
            data: Vec::new(),
        };

        self.call(req)
    }
}
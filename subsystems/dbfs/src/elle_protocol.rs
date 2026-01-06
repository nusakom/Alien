//! DBFS Elle 协议序列化/反序列化
//!
//! 定义 Host Linux 与 Alien 内核之间的通信协议

use alloc::{format, string::String, vec::Vec};
use core::mem::size_of;

// ==================== 协议定义 ====================

/// DBFS 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl DbfsOpType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            1 => Some(DbfsOpType::BeginTx),
            2 => Some(DbfsOpType::WriteFile),
            3 => Some(DbfsOpType::CreateFile),
            4 => Some(DbfsOpType::DeleteFile),
            5 => Some(DbfsOpType::Mkdir),
            6 => Some(DbfsOpType::Readdir),
            7 => Some(DbfsOpType::CommitTx),
            8 => Some(DbfsOpType::RollbackTx),
            _ => None,
        }
    }
}

/// DBFS 请求
#[derive(Debug, Clone)]
pub struct DbfsRequest {
    pub tx_id: u64,
    pub op_type: DbfsOpType,
    pub path: String,
    pub offset: u64,
    pub data: Vec<u8>,
}

/// DBFS 响应
#[derive(Debug, Clone)]
pub struct DbfsResponse {
    pub tx_id: u64,
    pub status: i32,
    pub lsn: u64,
    pub data: Vec<u8>,
}

// ==================== 序列化 ====================

impl DbfsRequest {
    /// 序列化为字节流
    ///
    /// 格式:
    /// [tx_id:8][op_type:1][path_len:2][path:data][offset:8][data_len:4][data:data]
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // tx_id (8 bytes)
        bytes.extend_from_slice(&self.tx_id.to_be_bytes());

        // op_type (1 byte)
        bytes.push(self.op_type as u8);

        // path (2 bytes length + data)
        let path_bytes = self.path.as_bytes();
        let path_len = path_bytes.len() as u16;
        bytes.extend_from_slice(&path_len.to_be_bytes());
        bytes.extend_from_slice(path_bytes);

        // offset (8 bytes)
        bytes.extend_from_slice(&self.offset.to_be_bytes());

        // data (4 bytes length + data)
        let data_len = self.data.len() as u32;
        bytes.extend_from_slice(&data_len.to_be_bytes());
        bytes.extend_from_slice(&self.data);

        bytes
    }

    /// 从字节流反序列化
    pub fn deserialize(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < 19 {
            // 最小长度: tx_id(8) + op_type(1) + path_len(2) + offset(8) = 19
            return Err(ProtocolError::InvalidLength);
        }

        let mut pos = 0;

        // tx_id
        let tx_id = u64::from_be_bytes(bytes[pos..pos+8].try_into().unwrap());
        pos += 8;

        // op_type
        let op_type = DbfsOpType::from_u8(bytes[pos])
            .ok_or(ProtocolError::InvalidOpType)?;
        pos += 1;

        // path_len
        let path_len = u16::from_be_bytes(bytes[pos..pos+2].try_into().unwrap()) as usize;
        pos += 2;

        // path
        if bytes.len() < pos + path_len {
            return Err(ProtocolError::InvalidLength);
        }
        let path = String::from_utf8(bytes[pos..pos+path_len].to_vec())
            .map_err(|_| ProtocolError::InvalidUtf8)?;
        pos += path_len;

        // offset
        if bytes.len() < pos + 8 {
            return Err(ProtocolError::InvalidLength);
        }
        let offset = u64::from_be_bytes(bytes[pos..pos+8].try_into().unwrap());
        pos += 8;

        // data_len
        if bytes.len() < pos + 4 {
            return Err(ProtocolError::InvalidLength);
        }
        let data_len = u32::from_be_bytes(bytes[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;

        // data
        if bytes.len() < pos + data_len {
            return Err(ProtocolError::InvalidLength);
        }
        let data = bytes[pos..pos+data_len].to_vec();

        Ok(Self {
            tx_id,
            op_type,
            path,
            offset,
            data,
        })
    }
}

impl DbfsResponse {
    /// 序列化为字节流
    ///
    /// 格式:
    /// [tx_id:8][status:4][lsn:8][data_len:4][data:data]
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // tx_id (8 bytes)
        bytes.extend_from_slice(&self.tx_id.to_be_bytes());

        // status (4 bytes)
        bytes.extend_from_slice(&self.status.to_be_bytes());

        // lsn (8 bytes)
        bytes.extend_from_slice(&self.lsn.to_be_bytes());

        // data (4 bytes length + data)
        let data_len = self.data.len() as u32;
        bytes.extend_from_slice(&data_len.to_be_bytes());
        bytes.extend_from_slice(&self.data);

        bytes
    }

    /// 从字节流反序列化
    pub fn deserialize(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < 24 {
            // 最小长度: tx_id(8) + status(4) + lsn(8) + data_len(4) = 24
            return Err(ProtocolError::InvalidLength);
        }

        let mut pos = 0;

        // tx_id
        let tx_id = u64::from_be_bytes(bytes[pos..pos+8].try_into().unwrap());
        pos += 8;

        // status
        let status = i32::from_be_bytes(bytes[pos..pos+4].try_into().unwrap());
        pos += 4;

        // lsn
        let lsn = u64::from_be_bytes(bytes[pos..pos+8].try_into().unwrap());
        pos += 8;

        // data_len
        let data_len = u32::from_be_bytes(bytes[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;

        // data
        if bytes.len() < pos + data_len {
            return Err(ProtocolError::InvalidLength);
        }
        let data = bytes[pos..pos+data_len].to_vec();

        Ok(Self {
            tx_id,
            status,
            lsn,
            data,
        })
    }
}

// ==================== 错误类型 ====================

#[derive(Debug, Clone)]
pub enum ProtocolError {
    InvalidLength,
    InvalidOpType,
    InvalidUtf8,
}

impl core::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ProtocolError::InvalidLength => write!(f, "Invalid protocol length"),
            ProtocolError::InvalidOpType => write!(f, "Invalid operation type"),
            ProtocolError::InvalidUtf8 => write!(f, "Invalid UTF-8"),
        }
    }
}

// ==================== 测试辅助函数 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialize() {
        let req = DbfsRequest {
            tx_id: 12345,
            op_type: DbfsOpType::CreateFile,
            path: String::from("/test.txt"),
            offset: 0,
            data: Vec::new(),
        };

        let bytes = req.serialize();
        let req2 = DbfsRequest::deserialize(&bytes).unwrap();

        assert_eq!(req2.tx_id, 12345);
        assert_eq!(req2.op_type, DbfsOpType::CreateFile);
        assert_eq!(req2.path, "/test.txt");
    }

    #[test]
    fn test_response_serialize() {
        let resp = DbfsResponse {
            tx_id: 12345,
            status: 0,
            lsn: 67890,
            data: b"test data".to_vec(),
        };

        let bytes = resp.serialize();
        let resp2 = DbfsResponse::deserialize(&bytes).unwrap();

        assert_eq!(resp2.tx_id, 12345);
        assert_eq!(resp2.status, 0);
        assert_eq!(resp2.lsn, 67890);
        assert_eq!(resp2.data, b"test data");
    }
}
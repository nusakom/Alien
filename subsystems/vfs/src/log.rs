// log.rs

#[derive(Debug, Clone)]
pub enum LogEntry {
    Write { offset: u64, data: Vec<u8> },
    Flush { timestamp: u64 },
    // 可以在这里添加更多类型的操作
}

impl LogEntry {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            LogEntry::Write { offset, data } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(&offset.to_le_bytes());
                buf.extend_from_slice(&data);
                buf
            },
            LogEntry::Flush { timestamp } => {
                let mut buf = Vec::new();
                buf.extend_from_slice(&timestamp.to_le_bytes());
                buf
            },
        }
    }

    pub fn deserialize(bytes: &[u8]) -> LogEntry {
        if bytes.len() > 8 {
            let offset = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
            let data = bytes[8..].to_vec();
            LogEntry::Write { offset, data }
        } else {
            let timestamp = u64::from_le_bytes(bytes.try_into().unwrap());
            LogEntry::Flush { timestamp }
        }
    }
}

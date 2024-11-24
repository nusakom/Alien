// wal.rs

impl WalFile {
    fn write_log_entry(&self, entry: LogEntry) -> AlienResult<()> {
        let log_data = entry.serialize();
        let mut pos = self.pos.lock();
        let write_result = self.write_at(*pos, &log_data);
        *pos += write_result? as u64;
        Ok(())
    }

    fn apply_log_entry(&self, entry: LogEntry) -> AlienResult<()> {
        match entry {
            LogEntry::Write { offset, data } => {
                // 在实际的数据文件上应用写操作
                let inode = self.dentry.inode()?; 
                inode.write_at(offset, &data)?; 
            },
            LogEntry::Flush { timestamp } => {
                // 执行清理操作
                let inode = self.dentry.inode()?; 
                inode.flush()?; 
            },
        }
        Ok(())
    }

    fn recover_log(&self) -> AlienResult<()> {
        // 恢复过程中读取 WAL 文件，回放其中的日志条目
        let inode = self.dentry.inode()?; 
        let log_data = inode.read_at(0, &mut Vec::new())?; 
        for entry_bytes in log_data.chunks(256) { 
            let entry = LogEntry::deserialize(entry_bytes);
            self.apply_log_entry(entry)?; 
        }
        Ok(())
    }
}

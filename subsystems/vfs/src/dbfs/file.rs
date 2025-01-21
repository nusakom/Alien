// dbfs/file.rs

#![no_std]

use crate::db::Db;
use crate::error::VfsError;
use heapless::Vec;

pub fn sync_create_file(db: &mut Db, path: &str, content: &[u8]) -> Result<(), VfsError> {
    // 检查文件是否已经存在
    if let Ok(Some(_)) = db.get_file_metadata(path) {
        return Err(VfsError::IoError("File already exists".to_string()));
    }

    // 将文件元数据保存到数据库
    db.insert_file_metadata(path, content.len())?;

    // 模拟将文件内容写入内存
    let _content: Vec<u8, 1024> = content.into();  // 假设文件最大 1024 字节
    Ok(())
}

pub fn sync_read_file(path: &str, db: &Db) -> Result<Vec<u8>, VfsError> {
    // 从数据库获取文件元数据
    if let Some((_, size)) = db.get_file_metadata(path)? {
        // 模拟读取文件内容（这里只返回一个大小，实际操作中可能需要更复杂的内存映射）
        let content: Vec<u8, 1024> = Vec::from_slice(&vec![0; size]).map_err(|_| VfsError::IoError("Failed to read file".to_string()))?;
        Ok(content)
    } else {
        Err(VfsError::IoError("File not found".to_string()))
    }
}

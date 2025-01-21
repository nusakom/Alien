// dbfs/file_ops.rs
use crate::db::get_db;  // 引用数据库操作
use crate::metadata::{insert_file_metadata, get_file_metadata};
use std::fs::{File, read};
use std::io::{Write, Result as IoResult};

pub async fn async_create_file(path: &str, content: &[u8]) -> Result<(), String> {
    // 检查文件是否已存在
    if let Some(_) = get_file_metadata(path).await {
        return Err("File already exists".to_string());
    }

    // 将文件内容写入磁盘
    let file = File::create(path).map_err(|e| e.to_string())?;
    file.write_all(content).map_err(|e| e.to_string())?;

    // 将文件的元数据插入到数据库
    insert_file_metadata(path, content.len()).await?;

    Ok(())
}

pub async fn async_read_file(path: &str) -> Result<Vec<u8>, String> {
    // 读取文件内容
    read(path).map_err(|e| e.to_string())
}

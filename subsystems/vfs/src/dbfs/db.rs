// dbfs/db.rs

#![no_std]

use heapless::Vec;
use crate::error::VfsError;

pub struct Db {
    files: Vec<(String, usize), 16>, // 假设最多支持 16 个文件的元数据
}

impl Db {
    pub fn new() -> Self {
        Db {
            files: Vec::new(),
        }
    }

    pub fn insert_file_metadata(&mut self, path: &str, size: usize) -> Result<(), VfsError> {
        // 模拟插入文件元数据
        if self.files.len() >= 16 {
            return Err(VfsError::DbError("Database full".to_string()));
        }
        self.files.push((path.to_string(), size))
            .map_err(|_| VfsError::DbError("Failed to insert data".to_string()))?;
        Ok(())
    }

    pub fn get_file_metadata(&self, path: &str) -> Result<Option<(String, usize)>, VfsError> {
        // 模拟查询文件元数据
        for (stored_path, size) in &self.files {
            if stored_path == path {
                return Ok(Some((stored_path.clone(), *size)));
            }
        }
        Ok(None)
    }
}

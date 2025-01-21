// rvfs.rs
use dbfs2::{dbfs_common_write, dbfs_common_removexattr, DBFSResult};
use crate::vfs::{File, VfsError};

/// 将数据写入 DBFS 文件系统
/// 
/// # Arguments
/// * `file` - 要写入的文件
/// * `buf` - 要写入的数据
/// 
/// # Returns
/// * 返回成功写入的字节数，或者返回 VfsError
pub fn vfs_write_to_dbfs(file: &mut File, buf: &[u8]) -> Result<usize, VfsError> {
    // 获取文件当前的写入位置
    let offset = file.pos.lock();

    // 调用 DBFS 的写操作
    let result = dbfs_common_write(buf.len(), buf, *offset);
    
    // 处理 DBFS 的返回结果
    match result {
        Ok(bytes_written) => {
            // 更新文件的写入位置
            file.pos.lock_mut() += bytes_written as u64;
            Ok(bytes_written) // 返回成功写入的字节数
        },
        Err(e) => Err(VfsError::IoError(format!("DBFS write failed: {}", e))), // 错误处理
    }
}

/// 删除文件的扩展属性
/// 
/// # Arguments
/// * `file` - 要删除扩展属性的文件
/// * `key` - 扩展属性的键
/// 
/// # Returns
/// * 成功则返回 Ok(())，失败则返回 VfsError
pub fn vfs_remove_xattr(file: &File, key: &str) -> Result<(), VfsError> {
    // 获取文件的 inode、UID、GID 和创建时间等元数据
    let inode = file.inode(); // 获取 inode
    let uid = file.uid(); // 获取用户ID
    let gid = file.gid(); // 获取组ID
    let ctime = file.ctime(); // 获取创建时间

    // 调用 DBFS 删除扩展属性操作
    let result = dbfs_common_removexattr(uid, gid, inode.ino(), key, ctime);
    
    // 处理 DBFS 删除扩展属性的结果
    match result {
        Ok(_) => Ok(()), // 成功时返回 Ok
        Err(e) => Err(VfsError::IoError(format!("Failed to remove xattr: {}", e))), // 错误处理
    }
}


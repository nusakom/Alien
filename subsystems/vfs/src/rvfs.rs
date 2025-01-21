use fuse::Request;
use fuse::FileType;
use fuse::FileAttr;
use fuse::ReplyData;
use fuse::ReplyAttr;
use fuse::ReplyEntry;
use fuse::ReplyWrite;
use dbfs2::{dbfs_common_write, dbfs_common_removexattr, DBFSResult};
use crate::vfs::{File, VfsError};

// FUSE 结构体：这里封装了 DBFS 操作
pub struct FuseDbfs;

// 获取文件属性
impl fuse::Filesystem for FuseDbfs {
    fn getattr(&self, req: &Request, ino: u64, reply: ReplyAttr) {
        // 根据文件的 inode 获取文件的属性
        let file_attr = get_file_attr_from_dbfs(ino);
        reply.attr(&std::time::Duration::from_secs(1), &file_attr);
    }

    fn read(&self, req: &Request, ino: u64, size: u32, offset: u64, reply: ReplyData) {
        // 从 DBFS 中读取文件
        let result = dbfs_common_read(ino, size, offset);
        match result {
            Ok(data) => reply.data(&data),
            Err(e) => reply.error(libc::EIO), // 返回错误码
        }
    }

    fn write(&self, req: &Request, ino: u64, buf: &[u8], offset: u64, reply: ReplyWrite) {
        // 将数据写入 DBFS
        let result = dbfs_common_write(buf.len(), buf, offset);
        match result {
            Ok(bytes_written) => reply.written(bytes_written as u32),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn remove_xattr(&self, req: &Request, ino: u64, key: &str, reply: fuse::ReplyEmpty) {
        // 删除扩展属性
        let result = dbfs_common_removexattr(ino, key);
        match result {
            Ok(_) => reply.ok(),
            Err(_) => reply.error(libc::EIO),
        }
    }
}

// 用于从 DBFS 获取文件属性的辅助函数
fn get_file_attr_from_dbfs(ino: u64) -> FileAttr {
    // 假设我们从 DBFS 获取文件属性
    FileAttr {
        ino,
        size: 0, // 根据实际需要获取文件大小
        blocks: 0,
        atime: std::time::SystemTime::now(),
        mtime: std::time::SystemTime::now(),
        ctime: std::time::SystemTime::now(),
        crtime: std::time::SystemTime::now(),
        kind: FileType::RegularFile,
        perm: 0o755,
        nlink: 1,
        uid: 1000,
        gid: 1000,
        rdev: 0,
    }
}

// 启动 FUSE 文件系统
pub fn run_fuse_server() -> Result<(), Box<dyn std::error::Error>> {
    let mountpoint = std::path::Path::new("/mnt/dbfs");
    fuse::mount(FuseDbfs, mountpoint, &[])?;
    Ok(())
}

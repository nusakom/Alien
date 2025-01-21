use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;
use core::io::SeekFrom;
use std::collections::HashMap;

// 假设已有的库
use vfscore::{
    dentry::VfsDentry,
    inode::{VfsInode, VfsInodeMode},
    poll::{PollEvents},
    path::VfsPath,
    utils::{AlienResult, LinuxErrno, OpenFlags, VfsFileStat},
    downcast::DowncastSync,
};

// 定义文件 trait，描述文件操作的接口
pub trait File: DowncastSync + Debug {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize>;
    fn write(&self, buf: &[u8]) -> AlienResult<usize>;
    fn read_at(&self, _offset: u64, _buf: &mut [u8]) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS) // 未实现错误
    }
    fn write_at(&self, _offset: u64, _buf: &[u8]) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS) // 未实现错误
    }
    fn flush(&self) -> AlienResult<()> {
        Ok(())
    }
    fn fsync(&self) -> AlienResult<()> {
        Ok(())
    }
    fn seek(&self, pos: SeekFrom) -> AlienResult<u64>;
    fn get_attr(&self) -> AlienResult<VfsFileStat>;
    fn ioctl(&self, _cmd: u32, _arg: usize) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS) // 未实现错误
    }
    fn set_open_flag(&self, _flag: OpenFlags);
    fn get_open_flag(&self) -> OpenFlags;
    fn dentry(&self) -> Arc<dyn VfsDentry>;
    fn inode(&self) -> Arc<dyn VfsInode>;
    fn readdir(&self, _buf: &mut [u8]) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS) // 未实现错误
    }
    fn truncate(&self, _len: u64) -> AlienResult<()> {
        Err(LinuxErrno::ENOSYS) // 未实现错误
    }
    fn is_readable(&self) -> bool;
    fn is_writable(&self) -> bool;
    fn is_append(&self) -> bool;
    fn poll(&self, _event: PollEvents) -> AlienResult<PollEvents> {
        panic!("poll is not implemented for :{:?}", self)
    }
}

// 文件的内存实现
pub struct RvfsFile {
    name: String,
    data: Vec<u8>,
    open_flag: OpenFlags,
    inode: Arc<dyn VfsInode>,
    dentry: Arc<dyn VfsDentry>,
}

impl RvfsFile {
    pub fn new(name: String, inode: Arc<dyn VfsInode>, dentry: Arc<dyn VfsDentry>) -> Self {
        RvfsFile {
            name,
            data: Vec::new(),
            open_flag: OpenFlags::O_RDONLY,
            inode,
            dentry,
        }
    }
}

impl File for RvfsFile {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize> {
        let len = buf.len().min(self.data.len());
        buf[..len].copy_from_slice(&self.data[..len]);
        Ok(len)
    }

    fn write(&self, buf: &[u8]) -> AlienResult<usize> {
        if !self.is_writable() {
            return Err(LinuxErrno::EPERM);
        }
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&self, pos: SeekFrom) -> AlienResult<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                if offset > self.data.len() as u64 {
                    Err(LinuxErrno::EINVAL)
                } else {
                    Ok(offset)
                }
            }
            SeekFrom::End(offset) => {
                let new_pos = self.data.len() as u64 + offset;
                if new_pos < 0 {
                    Err(LinuxErrno::EINVAL)
                } else {
                    Ok(new_pos)
                }
            }
            SeekFrom::Current(offset) => {
                let new_pos = (self.data.len() as i64 + offset) as u64;
                if new_pos < 0 {
                    Err(LinuxErrno::EINVAL)
                } else {
                    Ok(new_pos)
                }
            }
        }
    }

    fn get_attr(&self) -> AlienResult<VfsFileStat> {
        let stat = VfsFileStat {
            size: self.data.len() as u64,
            blocks: (self.data.len() as u64 + 511) / 512, // 块大小假设为 512 字节
            mode: self.inode().mode(),
            nlink: 1,
            uid: 0,
            gid: 0,
        };
        Ok(stat)
    }

    fn set_open_flag(&self, flag: OpenFlags) {
        self.open_flag = flag;
    }

    fn get_open_flag(&self) -> OpenFlags {
        self.open_flag
    }

    fn dentry(&self) -> Arc<dyn VfsDentry> {
        self.dentry.clone()
    }

    fn inode(&self) -> Arc<dyn VfsInode> {
        self.inode.clone()
    }

    fn is_readable(&self) -> bool {
        self.open_flag.contains(OpenFlags::O_RDONLY) || self.open_flag.contains(OpenFlags::O_RDWR)
    }

    fn is_writable(&self) -> bool {
        self.open_flag.contains(OpenFlags::O_WRONLY) || self.open_flag.contains(OpenFlags::O_RDWR)
    }

    fn is_append(&self) -> bool {
        self.open_flag.contains(OpenFlags::O_APPEND)
    }
}

// 文件系统实现
pub struct Rvfs {
    files: HashMap<String, Arc<dyn File>>,
}

impl Rvfs {
    pub fn new() -> Self {
        Rvfs {
            files: HashMap::new(),
        }
    }

    pub fn create_file(&mut self, name: String, inode: Arc<dyn VfsInode>, dentry: Arc<dyn VfsDentry>) {
        let file = Arc::new(RvfsFile::new(name.clone(), inode, dentry));
        self.files.insert(name, file);
    }

    pub fn get_file(&self, name: &str) -> Option<Arc<dyn File>> {
        self.files.get(name).cloned()
    }
}

// 测试示例
fn main() {
    // 假设我们有 inode 和 dentry 实例
    let inode: Arc<dyn VfsInode> = Arc::new(...); // 填入具体的 inode 实现
    let dentry: Arc<dyn VfsDentry> = Arc::new(...); // 填入具体的 dentry 实现

    let mut fs = Rvfs::new();
    fs.create_file("test.txt".to_string(), inode.clone(), dentry.clone());

    if let Some(file) = fs.get_file("test.txt") {
        let mut buf = vec![0u8; 100];
        file.read(&mut buf).unwrap();
        println!("File read data: {:?}", buf);
    }
}

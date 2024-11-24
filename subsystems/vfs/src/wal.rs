// wal.rs

use alloc::sync::Arc;
use core::fmt::{Debug, Formatter};

use constants::{AlienResult, LinuxErrno, io::{OpenFlags, SeekFrom}};
use downcast_rs::{impl_downcast, DowncastSync};
use ksync::Mutex;
use vfscore::{
    dentry::VfsDentry,
    inode::VfsInode,
    path::VfsPath,
    utils::{VfsFileStat, VfsNodeType, VfsPollEvents},
};

use crate::system_root_fs;

pub struct WalFile {
    pos: Mutex<u64>,             // 当前文件指针的位置
    open_flag: Mutex<OpenFlags>, // 文件打开标志
    dentry: Arc<dyn VfsDentry>,  // 文件的目录项（dentry）
}

impl Debug for WalFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("WalFile")
            .field("pos", &self.pos)
            .field("open_flag", &self.open_flag)
            .field("name", &self.dentry.name())
            .finish()
    }
}

impl WalFile {
    pub fn new(dentry: Arc<dyn VfsDentry>, open_flag: OpenFlags) -> Self {
        let pos = if open_flag.contains(OpenFlags::O_APPEND) {
            dentry.inode().unwrap().get_attr().unwrap().st_size
        } else {
            0
        };
        Self {
            pos: Mutex::new(pos),
            open_flag: Mutex::new(open_flag),
            dentry,
        }
    }

    // clean_up_logs 方法，用于清理日志
    pub fn clean_up_logs(&self) -> AlienResult<()> {
        // 获取 inode，执行清理操作
        let inode = self.dentry.inode()?;  // 获取 inode 对象
        
        // 假设 truncate(0) 会清空文件内容，如果你希望清除已应用的日志条目，可以调整这个逻辑
        inode.truncate(0)?;  // 将文件大小裁剪为0，清空日志内容
        
        // 其他清理逻辑（如释放内存、清理元数据等）可以在此添加
        Ok(())
    }
}

pub trait File: DowncastSync + Debug {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize>;
    fn write(&self, buf: &[u8]) -> AlienResult<usize>;
    fn read_at(&self, _offset: u64, _buf: &mut [u8]) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS)
    }
    fn write_at(&self, _offset: u64, _buf: &[u8]) -> AlienResult<usize> {
        Err(LinuxErrno::ENOSYS)
    }
    fn flush(&self) -> AlienResult<()>;
    fn fsync(&self) -> AlienResult<()>;
    fn seek(&self, pos: SeekFrom) -> AlienResult<u64>;
    fn get_attr(&self) -> AlienResult<VfsFileStat>;
    fn set_open_flag(&self, _flag: OpenFlags);
    fn get_open_flag(&self) -> OpenFlags;
    fn dentry(&self) -> Arc<dyn VfsDentry>;
    fn inode(&self) -> Arc<dyn VfsInode>;
}

impl_downcast!(sync File);

impl File for WalFile {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let pos = *self.pos.lock();
        let read = self.read_at(pos, buf)?;  // Read at current position
        *self.pos.lock() += read as u64;
        Ok(read)
    }

    fn write(&self, buf: &[u8]) -> AlienResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let mut pos = self.pos.lock();
        let write = self.write_at(*pos, buf)?;  // Write at current position
        *pos += write as u64;
        Ok(write)
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> AlienResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let open_flag = self.open_flag.lock();
        if !open_flag.contains(OpenFlags::O_RDONLY) && !open_flag.contains(OpenFlags::O_RDWR) {
            return Err(LinuxErrno::EPERM);
        }
        drop(open_flag);
        let inode = self.dentry.inode()?;
        let read = inode.read_at(offset, buf)?;
        Ok(read)
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> AlienResult<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }
        let open_flag = self.open_flag.lock();
        if !open_flag.contains(OpenFlags::O_WRONLY) && !open_flag.contains(OpenFlags::O_RDWR) {
            return Err(LinuxErrno::EPERM);
        }
        let inode = self.dentry.inode()?;
        let write = inode.write_at(offset, buf)?;
        Ok(write)
    }

    fn flush(&self) -> AlienResult<()> {
        let open_flag = self.open_flag.lock();
        if !open_flag.contains(OpenFlags::O_WRONLY) && !open_flag.contains(OpenFlags::O_RDWR) {
            return Err(LinuxErrno::EPERM);
        }
        let inode = self.dentry.inode()?;
        inode.flush()?;
        Ok(())
    }

    fn fsync(&self) -> AlienResult<()> {
        let open_flag = self.open_flag.lock();
        if !open_flag.contains(OpenFlags::O_WRONLY) && !open_flag.contains(OpenFlags::O_RDWR) {
            return Err(LinuxErrno::EPERM);
        }
        let inode = self.dentry.inode()?;
        inode.fsync()?;
        Ok(())
    }

    fn seek(&self, pos: SeekFrom) -> AlienResult<u64> {
        let mut spos = self.pos.lock();
        let size = self.get_attr()?.st_size;
        let new_offset = match pos {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(off) => spos.checked_add_signed(off),
            SeekFrom::End(off) => size.checked_add_signed(off),
        }
        .ok_or_else(|| LinuxErrno::EINVAL)?;
        *spos = new_offset;
        Ok(new_offset)
    }

    fn get_attr(&self) -> AlienResult<VfsFileStat> {
        self.dentry.inode()?.get_attr().map_err(Into::into)
    }

    fn set_open_flag(&self, flag: OpenFlags) {
        *self.open_flag.lock() = flag;
    }

    fn get_open_flag(&self) -> OpenFlags {
        *self.open_flag.lock()
    }

    fn dentry(&self) -> Arc<dyn VfsDentry> {
        self.dentry.clone()
    }

    fn inode(&self) -> Arc<dyn VfsInode> {
        self.dentry.inode().unwrap()
    }
}

impl Drop for WalFile {
    fn drop(&mut self) {
        let _ = self.flush();
        let _ = self.fsync();
    }
}

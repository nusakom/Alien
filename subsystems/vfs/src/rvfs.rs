use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::fmt::Debug;
use core::io::SeekFrom;
use std::collections::HashMap;

// 假设我们有DBFS2的接口
use dbfs2::{
    File as Dbfs2File,
    FileSystem as Dbfs2FileSystem,
    FileStat,
    OpenFlags,
};

// 文件 trait，描述文件操作的接口
pub trait File: DowncastSync + Debug {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize>;
    fn write(&self, buf: &[u8]) -> AlienResult<usize>;
    fn seek(&self, pos: SeekFrom) -> AlienResult<u64>;
    fn get_attr(&self) -> AlienResult<FileStat>;
    fn set_open_flag(&self, flag: OpenFlags);
    fn get_open_flag(&self) -> OpenFlags;
    fn dentry(&self) -> Arc<dyn VfsDentry>;
    fn inode(&self) -> Arc<dyn VfsInode>;
}

// 内存实现的文件
pub struct RvfsFile {
    name: String,
    dbfs2_file: Arc<Dbfs2File>,
    open_flag: OpenFlags,
}

impl RvfsFile {
    pub fn new(name: String, dbfs2_file: Arc<Dbfs2File>) -> Self {
        RvfsFile {
            name,
            dbfs2_file,
            open_flag: OpenFlags::O_RDONLY,
        }
    }
}

impl File for RvfsFile {
    fn read(&self, buf: &mut [u8]) -> AlienResult<usize> {
        self.dbfs2_file.read(buf)
    }

    fn write(&self, buf: &[u8]) -> AlienResult<usize> {
        self.dbfs2_file.write(buf)
    }

    fn seek(&self, pos: SeekFrom) -> AlienResult<u64> {
        self.dbfs2_file.seek(pos)
    }

    fn get_attr(&self) -> AlienResult<FileStat> {
        self.dbfs2_file.get_attr()
    }

    fn set_open_flag(&self, flag: OpenFlags) {
        self.open_flag = flag;
    }

    fn get_open_flag(&self) -> OpenFlags {
        self.open_flag
    }

    fn dentry(&self) -> Arc<dyn VfsDentry> {
        self.dbfs2_file.dentry()
    }

    fn inode(&self) -> Arc<dyn VfsInode> {
        self.dbfs2_file.inode()
    }
}

// 文件系统实现
pub struct Rvfs {
    dbfs2_fs: Arc<Dbfs2FileSystem>,
}

impl Rvfs {
    pub fn new(dbfs2_fs: Arc<Dbfs2FileSystem>) -> Self {
        Rvfs { dbfs2_fs }
    }

    pub fn create_file(&self, name: String, open_flags: OpenFlags) -> AlienResult<Arc<dyn File>> {
        let dbfs2_file = self.dbfs2_fs.create_file(name.clone(), open_flags)?;
        Ok(Arc::new(RvfsFile::new(name, dbfs2_file)))
    }

    pub fn get_file(&self, name: &str) -> Option<Arc<dyn File>> {
        if let Some(dbfs2_file) = self.dbfs2_fs.get_file(name) {
            Some(Arc::new(RvfsFile::new(name.to_string(), dbfs2_file)))
        } else {
            None
        }
    }
}

// 测试示例
fn main() {
    // 假设我们有一个 DBFS2 文件系统实例
    let dbfs2_fs: Arc<Dbfs2FileSystem> = Arc::new(...); // 填入具体的 DBFS2 文件系统实现

    let rvfs = Rvfs::new(dbfs2_fs);

    // 创建文件
    let file = rvfs.create_file("test.txt".to_string(), OpenFlags::O_RDWR).unwrap();

    // 读取文件
    let mut buf = vec![0u8; 100];
    file.read(&mut buf).unwrap();
    println!("File read data: {:?}", buf);

    // 写入文件
    file.write(b"Hello, RVFS!").unwrap();

    // 获取文件属性
    let stat = file.get_attr().unwrap();
    println!("File attributes: {:?}", stat);
}

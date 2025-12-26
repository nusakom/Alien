use std::sync::Arc;
use dbfs_vfs::{DBFS, DBFSProvider};
use vfscore::{
    dentry::VfsDentry,
    fstype::VfsFsType,
    utils::{VfsNodePerm, VfsNodeType, VfsTimeSpec},
    VfsResult,
};

#[derive(Clone)]
struct TestDBFSProviderImpl;

impl DBFSProvider for TestDBFSProviderImpl {
    fn current_time(&self) -> VfsTimeSpec {
        Default::default()
    }
}

fn make_dbfs() -> VfsResult<Arc<dyn VfsDentry>> {
    let dbfs = DBFS::new("test_db");
    dbfs.i_mount(0, "/", None, &[])
}

#[test]
fn test_dbfs_basic_operations() {
    let root = make_dbfs().unwrap();
    let inode = root.inode().unwrap();
    
    // Test file creation
    let file = inode.create("test.txt", VfsNodeType::File, "rw-rw-rw-".into(), None).unwrap();
    
    // Test file write
    let write_result = file.write_at(0, b"Hello, DBFS!");
    assert!(write_result.is_ok());
    
    // Test file read
    let mut buf = [0u8; 100];
    let read_result = file.read_at(0, &mut buf);
    assert!(read_result.is_ok());
    
    let len = read_result.unwrap();
    let content = std::str::from_utf8(&buf[..len]).unwrap();
    assert_eq!(content, "Hello, DBFS!");
    
    println!("DBFS basic operations test passed!");
}

#[test]
fn test_dbfs_directory_operations() {
    let root = make_dbfs().unwrap();
    let inode = root.inode().unwrap();
    
    // Test directory creation
    let dir = inode.create("test_dir", VfsNodeType::Dir, "rwxrwxrwx".into(), None).unwrap();
    
    // Verify it's a directory
    assert_eq!(dir.inode_type(), VfsNodeType::Dir);
    
    println!("DBFS directory operations test passed!");
}
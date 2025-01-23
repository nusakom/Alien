use alloc::{sync::Arc, vec};

use constants::AlienResult;
use core2::io::Read;
use cpio_reader::Mode;
use vfscore::{
    dentry::VfsDentry,
    path::VfsPath,
    utils::{VfsInodeMode, VfsNodeType},
};

pub fn populate_initrd(root: Arc<dyn VfsDentry>) -> AlienResult<()> {
    root.inode()?
        .create("bin", VfsNodeType::Dir, "rwxr-xr-x".into(), None)?;
    root.inode()?
        .create("sbin", VfsNodeType::Dir, "rwxr-xr-x".into(), None)?;
    parse_initrd_data(root)?;
    println!("Initrd populate success");
    Ok(())
}
fn parse_initrd_data(root: Arc<dyn VfsDentry>) -> AlienResult<()> {
    let mut guard = mem::data::INITRD_DATA.lock();
    if guard.is_some() {
        let path = VfsPath::new(root.clone(), root.clone());
        let data = guard.as_ref().unwrap();
        let st = data.data_ptr;
        let size = data.size;
        let data = unsafe { core::slice::from_raw_parts(st as *const u8, size) };
        let mut decoder = libflate::gzip::Decoder::new(data).unwrap();
        let mut buf = vec![];
        let _r = decoder.read_to_end(&mut buf).unwrap();
        for entry in cpio_reader::iter_files(&buf) {
            let mode = entry.mode();
            let name = entry.name();
            if name.starts_with("bin/") | name.starts_with("sbin/") {
                let inode_mode = VfsInodeMode::from_bits_truncate(mode.bits());
                if mode.contains(Mode::SYMBOLIK_LINK) {
                    // create symlink
                    let data = entry.file();
                    let target = core::str::from_utf8(data).unwrap();
                    path.join(name)?.symlink(target)?;
                } else if mode.contains(Mode::REGULAR_FILE) {
                    // create file
                    let f = path.join(name)?.open(Some(inode_mode))?;
                    f.inode()?.write_at(0, entry.file())?;
                }
            }
        }
        // release the page frame
        guard.take();
    }
    Ok(())
}
let db = DB::open::<FileOpenOptions, _>(Arc::new(FakeMap), "my-database.db").unwrap();
init_db(&db);
dbfs2::init_dbfs(db); // 初始化全局的DBFS
pub fn init_db(db: &DB, size: u64) {
    let tx = db.tx(true).unwrap();
    let bucket = tx.get_bucket("super_blk");
    let bucket = if bucket.is_ok() {
        return;
    } else {
        tx.create_bucket("super_blk").unwrap()
    };
    bucket.put("continue_number", 1usize.to_be_bytes()).unwrap();
    bucket.put("magic", 1111u32.to_be_bytes()).unwrap();
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).unwrap();
    bucket.put("disk_size", size.to_be_bytes()).unwrap(); //16MB
    tx.commit().unwrap()
}

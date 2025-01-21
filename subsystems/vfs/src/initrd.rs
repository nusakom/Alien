use alloc::{sync::Arc, vec};
use constants::AlienResult;
use core2::io::Read;
use cpio_reader::Mode;
use vfscore::{
    dentry::VfsDentry,
    path::VfsPath,
    utils::{VfsInodeMode, VfsNodeType},
};
use tokio::sync::RwLock;  // 使用 tokio::sync 来处理异步锁

pub async fn populate_initrd(root: Arc<dyn VfsDentry>) -> AlienResult<()> {
    // 创建目录
    root.inode()?
        .create("bin", VfsNodeType::Dir, "rwxr-xr-x".into(), None).map_err(|e| {
            log::error!("Error creating 'bin' directory: {:?}", e);
            e
        })?;

    root.inode()?
        .create("sbin", VfsNodeType::Dir, "rwxr-xr-x".into(), None).map_err(|e| {
            log::error!("Error creating 'sbin' directory: {:?}", e);
            e
        })?;

    // 解析initrd数据
    parse_initrd_data(root).await?;
    log::info!("Initrd populate success");

    Ok(())
}

async fn parse_initrd_data(root: Arc<dyn VfsDentry>) -> AlienResult<()> {
    let mut guard = mem::data::INITRD_DATA.lock().await; // 使用异步锁
    if let Some(data) = guard.take() {
        let path = VfsPath::new(root.clone(), root.clone());
        let st = data.data_ptr;
        let size = data.size;
        let data = unsafe { core::slice::from_raw_parts(st as *const u8, size) };
        let mut decoder = libflate::gzip::Decoder::new(data).unwrap();
        let mut buf = vec![];

        // 解压数据
        let _r = decoder.read_to_end(&mut buf).unwrap();

        // 处理文件
        for entry in cpio_reader::iter_files(&buf) {
            let mode = entry.mode();
            let name = entry.name();
            if name.starts_with("bin/") || name.starts_with("sbin/") {
                let inode_mode = VfsInodeMode::from_bits_truncate(mode.bits());

                if mode.contains(Mode::SYMBOLIK_LINK) {
                    // 创建符号链接
                    let data = entry.file();
                    let target = core::str::from_utf8(data).unwrap();
                    path.join(name)?.symlink(target).map_err(|e| {
                        log::error!("Error creating symlink for {}: {:?}", name, e);
                        e
                    })?;
                } else if mode.contains(Mode::REGULAR_FILE) {
                    // 创建常规文件
                    let f = path.join(name)?.open(Some(inode_mode)).map_err(|e| {
                        log::error!("Error opening file {}: {:?}", name, e);
                        e
                    })?;
                    f.inode()?.write_at(0, entry.file()).map_err(|e| {
                        log::error!("Error writing to file {}: {:?}", name, e);
                        e
                    })?;
                }
            }
        }
    } else {
        log::error!("No initrd data found.");
    }

    Ok(())
}

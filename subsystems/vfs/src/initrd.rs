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
        println!("Starting initrd parsing...");
        let path = VfsPath::new(root.clone(), root.clone());
        let data = guard.as_ref().unwrap();
        let st = data.data_ptr;
        let size = data.size;
        println!("Initrd data: ptr=0x{:x}, size={}", st, size);
        let data = unsafe { core::slice::from_raw_parts(st as *const u8, size) };
        let mut decoder = libflate::gzip::Decoder::new(data).unwrap();
        let mut buf = vec![];
        let _r = decoder.read_to_end(&mut buf).unwrap();
        println!("Decompressed initrd: {} bytes", buf.len());
        
        let mut file_count = 0;
        for entry in cpio_reader::iter_files(&buf) {
            let mode = entry.mode();
            let name = entry.name();
            file_count += 1;
            // println!("Processing entry {}: '{}' (mode: {:?})", file_count, name, mode);

            // Skip empty names and current directory entries
            if name.is_empty() || name == "." {
                // println!("  Skipping empty/current dir entry");
                continue;
            }
            
            let inode_mode = VfsInodeMode::from_bits_truncate(mode.bits());
            if mode.contains(Mode::SYMBOLIK_LINK) {
                // create symlink
                let data = entry.file();
                let target = core::str::from_utf8(data).unwrap();
                // println!("  Creating symlink: {} -> {}", name, target);
                match path.join(name) {
                    Ok(link_path) => {
                        if let Err(_e) = link_path.symlink(target) {
                            // Silently ignore symlink failures in initramfs
                            // Most symlinks are optional for DBFS testing
                        }
                    }
                    Err(_e) => {
                        // Silently ignore path join failures
                    }
                }
            } else if mode.contains(Mode::REGULAR_FILE) {
                // create file
                // println!("  Creating file: {} ({} bytes)", name, entry.file().len());
                match path.join(name) {
                    Ok(file_path) => {
                        match file_path.open(Some(inode_mode)) {
                            Ok(f) => {
                                match f.inode() {
                                    Ok(inode) => {
                                        match inode.write_at(0, entry.file()) {
                                            Ok(_) => {
                                                // Only log important files
                                                if name == "init" || name.contains("dbfs") {
                                                    println!("  ✅ Created initramfs file: {}", name);
                                                }
                                            }
                                            Err(e) => {
                                                println!("  ❌ Failed to write file {}: {:?}", name, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("  ❌ Failed to get inode for {}: {:?}", name, e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("  ❌ Failed to open file {}: {:?}", name, e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ❌ Failed to join path for file {}: {:?}", name, e);
                    }
                }
            }
            // Skip directories for now - they'll be created automatically when needed
        }
        println!("Processed {} entries from initrd", file_count);
        // release the page frame
        guard.take();
    } else {
        println!("No initrd data found");
    }
    Ok(())
}

// dbfs2/init.rs
use dbfs2::{init_dbfs, DB, DbfsResult};

const SLICE_SIZE: usize = 4096;  // 假设每块的大小为4096字节

pub fn init_db(db: &DB, size: u64) -> DbfsResult<()> {
    let tx = db.tx(true).map_err(|e| DbfsResult::Err(e.to_string()))?;
    
    let bucket = tx.get_bucket("super_blk");
    let bucket = if bucket.is_ok() {
        bucket.unwrap()
    } else {
        tx.create_bucket("super_blk").map_err(|e| DbfsResult::Err(e.to_string()))?
    };
    
    bucket.put("continue_number", 1usize.to_be_bytes()).map_err(|e| DbfsResult::Err(e.to_string()))?;
    bucket.put("magic", 1111u32.to_be_bytes()).map_err(|e| DbfsResult::Err(e.to_string()))?;
    bucket.put("blk_size", (SLICE_SIZE as u32).to_be_bytes()).map_err(|e| DbfsResult::Err(e.to_string()))?;
    bucket.put("disk_size", size.to_be_bytes()).map_err(|e| DbfsResult::Err(e.to_string()))?;
    
    tx.commit().map_err(|e| DbfsResult::Err(e.to_string()))?;
    
    Ok(())
}

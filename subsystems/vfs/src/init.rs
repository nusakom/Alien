// dbfs2/init.rs
use dbfs2::{init_dbfs, DB, DbfsResult};

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
    tx.commit().unwrap();
}

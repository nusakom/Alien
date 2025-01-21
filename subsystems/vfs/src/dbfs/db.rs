// dbfs/db.rs
use jammdb::{Database, Query};

pub async fn get_db() -> Result<Database, String> {
    // 打开数据库（可以配置为从配置文件加载数据库路径等）
    Database::open("/path/to/db")
        .await
        .map_err(|e| e.to_string())
}

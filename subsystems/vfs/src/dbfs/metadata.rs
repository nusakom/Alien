// dbfs/metadata.rs
use crate::db::get_db;
use jammdb::{Database, Query};

pub async fn insert_file_metadata(path: &str, size: usize) -> Result<(), String> {
    let db = get_db().await.map_err(|e| e.to_string())?;

    let query = Query::insert_into("files")
        .values(vec![("path", path), ("size", &size.to_string())]);

    db.execute(query).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_file_metadata(path: &str) -> Option<(String, usize)> {
    let db = get_db().await.unwrap();
    let query = Query::select()
        .from("files")
        .where_condition(format!("path = '{}'", path));

    let result = db.execute(query).await.unwrap();
    if result.is_empty() {
        None
    } else {
        // 假设返回元数据的第一行数据
        let row = &result[0];
        let path = row.get("path").unwrap().to_string();
        let size = row.get("size").unwrap().parse().unwrap();
        Some((path, size))
    }
}

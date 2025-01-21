// dbfs/error.rs

#[derive(Debug)]
pub enum VfsError {
    IoError(String),
    DbError(String),
}

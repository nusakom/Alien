// dbfs/error.rs

#![no_std]

#[derive(Debug)]
pub enum VfsError {
    IoError(String),
    DbError(String),
}

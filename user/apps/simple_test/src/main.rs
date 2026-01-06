#![no_main]
#![no_std]

use Mstd::{
    println, 
    fs::{open, close, read, write, mkdir, OpenFlags},
    process::exec,
};

#[no_mangle]
fn main() -> isize {
    println!("🚀 Simple Test Runner");
    
    // Test basic file operations
    println!("Testing basic file operations...");
    
    // Create test directory
    let result = mkdir("/tmp/test\0");
    println!("mkdir result: {}", result);
    
    // Create and write to a file
    let fd = open("/tmp/test/hello.txt\0", OpenFlags::O_CREAT | OpenFlags::O_WRONLY);
    println!("open result: {}", fd);
    
    if fd >= 0 {
        let data = b"Hello DBFS!";
        let write_result = write(fd as usize, data);
        println!("write result: {}", write_result);
        close(fd as usize);
        
        // Read back the file
        let fd = open("/tmp/test/hello.txt\0", OpenFlags::O_RDONLY);
        if fd >= 0 {
            let mut buf = [0u8; 32];
            let read_result = read(fd as usize, &mut buf);
            println!("read result: {}", read_result);
            close(fd as usize);
            
            if read_result > 0 {
                println!("✅ File operations working!");
            }
        }
    }
    
    // Try to run dbfs_test if it exists
    println!("Attempting to run dbfs_test...");
    let args: &[*const u8] = &[];
    let env: &[*const u8] = &[];
    let result = exec("./dbfs_test\0", args, env);
    println!("exec dbfs_test result: {}", result);
    
    println!("🏁 Simple test completed");
    0
}
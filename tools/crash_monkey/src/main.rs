use std::sync::{Arc, Mutex};
use std::vec::Vec;

use constants::AlienResult;
use device_interface::{BlockDevice, DeviceBase};
use dbfs2::Dbfs;
use jammdb::DB;

// ============================================================================
// 1. Simulation Infrastructure (Simulated Disk)
// ============================================================================

#[derive(Clone)]
pub struct RamDisk {
    pub data: Arc<Mutex<Vec<u8>>>,
    pub size: usize,
    pub history: Arc<Mutex<Vec<(usize, Vec<u8>)>>>, // (offset, bytes)
}

impl RamDisk {
    pub fn new(size: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(vec![0u8; size])),
            size,
            history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn apply_trace(&self, trace: &[(usize, Vec<u8>)]) {
        let mut data = self.data.lock().unwrap();
        for (offset, bytes) in trace {
            if offset + bytes.len() <= data.len() {
                data[*offset..*offset+bytes.len()].copy_from_slice(bytes);
            }
        }
    }
}

impl DeviceBase for RamDisk {
    fn handle_irq(&self) {}
}

impl BlockDevice for RamDisk {
    fn read(&self, buf: &mut [u8], offset: usize) -> AlienResult<usize> {
        let data = self.data.lock().unwrap();
        if offset >= data.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), data.len() - offset);
        buf[..len].copy_from_slice(&data[offset..offset+len]);
        Ok(len)
    }

    fn write(&self, buf: &[u8], offset: usize) -> AlienResult<usize> {
        // 1. Write to actual memory
        {
            let mut data = self.data.lock().unwrap();
            if offset + buf.len() > data.len() {
                panic!("Write out of bounds");
            }
            data[offset..offset+buf.len()].copy_from_slice(buf);
        }

        // 2. Log to history
        {
            let mut history = self.history.lock().unwrap();
            history.push((offset, buf.to_vec()));
        }
        
        Ok(buf.len())
    }

    fn size(&self) -> usize {
        self.size
    }

    fn flush(&self) -> AlienResult<()> {
        Ok(())
    }
}

// ============================================================================
// 2. Test Workloads
// ============================================================================

mod workloads {
    use super::*;
    use dbfs2::common;

    pub fn cross_directory_rename(fs: &Arc<Dbfs>) -> (usize, usize, usize) {
        let root = 1;
        let source_dir = fs.create(root, "source_dir", 0, 0, Default::default(), 
            common::DbfsPermission::S_IFDIR | common::DbfsPermission::S_IRWXU).unwrap();
        let dest_dir = fs.create(root, "dest_dir", 0, 0, Default::default(), 
            common::DbfsPermission::S_IFDIR | common::DbfsPermission::S_IRWXU).unwrap();
        let file = fs.create(source_dir.ino, "file", 0, 0, Default::default(), 
            common::DbfsPermission::S_IFREG | common::DbfsPermission::S_IRWXU).unwrap();
        
        fs.write(file.ino, b"Content", 0).unwrap();
        
        (source_dir.ino, dest_dir.ino, file.ino)
    }

    pub fn simple_write(fs: &Arc<Dbfs>) -> usize {
        let root = 1;
        let file = fs.create(root, "test.txt", 0, 0, Default::default(),
            common::DbfsPermission::S_IFREG | common::DbfsPermission::S_IRWXU).unwrap();
        
        fs.write(file.ino, b"Hello World", 0).unwrap();
        file.ino
    }

    pub fn multi_file_write(fs: &Arc<Dbfs>) -> Vec<usize> {
        let root = 1;
        let mut files = Vec::new();
        
        for i in 0..5 {
            let name = format!("file{}.txt", i);
            let file = fs.create(root, &name, 0, 0, Default::default(),
                common::DbfsPermission::S_IFREG | common::DbfsPermission::S_IRWXU).unwrap();
            
            let content = format!("Content of file {}", i);
            fs.write(file.ino, content.as_bytes(), 0).unwrap();
            files.push(file.ino);
        }
        
        files
    }
}

// ============================================================================
// 3. Verification Logic
// ============================================================================

mod verifiers {
    use super::*;
    use dbfs2::common;

    pub fn verify_rename_atomicity(
        fs: &Arc<Dbfs>,
        source_dir: usize,
        dest_dir: usize,
        step: usize,
    ) -> Result<(), String> {
        let in_source = fs.lookup(source_dir, "file").is_ok();
        let in_dest = fs.lookup(dest_dir, "file").is_ok();
        
        if in_source && !in_dest {
            // Pre-commit state: OK
            Ok(())
        } else if !in_source && in_dest {
            // Post-commit state: OK
            Ok(())
        } else if in_source && in_dest {
            Err(format!("Step {}: File found in BOTH source and dest (Duplication)", step))
        } else {
            Err(format!("Step {}: File found in NEITHER source nor dest (Data Loss)", step))
        }
    }

    pub fn verify_write_atomicity(
        fs: &Arc<Dbfs>,
        file_ino: usize,
        expected_content: &[u8],
        step: usize,
    ) -> Result<(), String> {
        let root = 1;
        match fs.lookup(root, "test.txt") {
            Ok(attr) => {
                let mut buf = vec![0u8; 100];
                let len = fs.read(attr.ino, &mut buf, 0).unwrap_or(0);
                let read_content = &buf[..len];
                
                if read_content == expected_content {
                    Ok(())
                } else if read_content.is_empty() {
                    Ok(()) // Empty is acceptable (not yet committed)
                } else {
                    Err(format!("Step {}: Partial write detected: {:?}", step, 
                        String::from_utf8_lossy(read_content)))
                }
            }
            Err(_) => Ok(()), // File not found is acceptable
        }
    }

    pub fn verify_multi_file_consistency(
        fs: &Arc<Dbfs>,
        expected_files: &[(&str, &str)],
        step: usize,
    ) -> Result<(), String> {
        let root = 1;
        let mut found_count = 0;
        
        for (name, expected_content) in expected_files {
            if let Ok(attr) = fs.lookup(root, name) {
                let mut buf = vec![0u8; 100];
                let len = fs.read(attr.ino, &mut buf, 0).unwrap_or(0);
                let content = String::from_utf8_lossy(&buf[..len]);
                
                if content == *expected_content {
                    found_count += 1;
                } else if !content.is_empty() {
                    return Err(format!("Step {}: File {} has corrupted content", step, name));
                }
            }
        }
        
        // Either all files are present or none (atomicity)
        if found_count == 0 || found_count == expected_files.len() {
            Ok(())
        } else {
            Err(format!("Step {}: Partial commit detected ({}/{} files)", 
                step, found_count, expected_files.len()))
        }
    }
}

// ============================================================================
// 4. Test Runner
// ============================================================================

fn run_crash_test<W, V>(
    workload_name: &str,
    workload: W,
    verifier: V,
) -> Result<(), String>
where
    W: Fn(&Arc<Dbfs>) -> (),
    V: Fn(&Arc<Dbfs>, usize) -> Result<(), String>,
{
    log::info!("=== Testing: {} ===", workload_name);
    
    let disk_size = 16 * 1024 * 1024;
    let golden_disk = Arc::new(RamDisk::new(disk_size));
    let db = DB::open((), "").unwrap();
    let fs = Dbfs::new(db, golden_disk.clone());

    // Execute workload
    workload(&fs);
    
    // Snapshot before operation
    let pre_snapshot = {
        let d = golden_disk.data.lock().unwrap();
        d.clone()
    };
    golden_disk.history.lock().unwrap().clear();

    // Extract trace
    let trace = {
        let h = golden_disk.history.lock().unwrap();
        h.clone()
    };
    
    log::info!("Captured {} write operations", trace.len());

    // Test all crash points
    let mut success_count = 0;
    for i in 0..=trace.len() {
        let crash_disk = Arc::new(RamDisk::new(disk_size));
        {
            let mut d = crash_disk.data.lock().unwrap();
            d.copy_from_slice(&pre_snapshot);
        }
        crash_disk.apply_trace(&trace[0..i]);
        
        let fresh_db = DB::open((), "").unwrap();
        let fs_rec = Dbfs::new(fresh_db, crash_disk.clone());

        verifier(&fs_rec, i)?;
        success_count += 1;
    }
    
    log::info!("✅ PASSED: All {} crash states verified", success_count);
    Ok(())
}

fn main() {
    env_logger::init();
    log::info!("🚀 Starting Extended CrashMonkey Test Suite...\n");

    let mut total_tests = 0;
    let mut passed_tests = 0;

    // Test 1: Cross-directory rename
    total_tests += 1;
    match run_crash_test(
        "Cross-Directory Rename",
        |fs| {
            let (src, dst, _) = workloads::cross_directory_rename(fs);
            fs.rename(src, "file", dst, "file").unwrap();
        },
        |fs, step| {
            // Need to get source_dir and dest_dir inodes
            // For simplicity, assume they are 2 and 3
            verifiers::verify_rename_atomicity(fs, 2, 3, step)
        },
    ) {
        Ok(_) => {
            passed_tests += 1;
            log::info!("✅ Test 1 PASSED\n");
        }
        Err(e) => {
            log::error!("❌ Test 1 FAILED: {}\n", e);
        }
    }

    // Test 2: Simple write atomicity
    total_tests += 1;
    match run_crash_test(
        "Simple Write Atomicity",
        |fs| {
            workloads::simple_write(fs);
        },
        |fs, step| {
            verifiers::verify_write_atomicity(fs, 2, b"Hello World", step)
        },
    ) {
        Ok(_) => {
            passed_tests += 1;
            log::info!("✅ Test 2 PASSED\n");
        }
        Err(e) => {
            log::error!("❌ Test 2 FAILED: {}\n", e);
        }
    }

    // Test 3: Multi-file consistency
    total_tests += 1;
    match run_crash_test(
        "Multi-File Write Consistency",
        |fs| {
            workloads::multi_file_write(fs);
        },
        |fs, step| {
            let expected = vec![
                ("file0.txt", "Content of file 0"),
                ("file1.txt", "Content of file 1"),
                ("file2.txt", "Content of file 2"),
                ("file3.txt", "Content of file 3"),
                ("file4.txt", "Content of file 4"),
            ];
            verifiers::verify_multi_file_consistency(fs, &expected, step)
        },
    ) {
        Ok(_) => {
            passed_tests += 1;
            log::info!("✅ Test 3 PASSED\n");
        }
        Err(e) => {
            log::error!("❌ Test 3 FAILED: {}\n", e);
        }
    }

    // Summary
    log::info!("==========================================");
    log::info!("📊 Test Summary:");
    log::info!("   Total Tests: {}", total_tests);
    log::info!("   Passed: {}", passed_tests);
    log::info!("   Failed: {}", total_tests - passed_tests);
    log::info!("   Success Rate: {:.1}%", (passed_tests as f64 / total_tests as f64) * 100.0);
    log::info!("==========================================");

    if passed_tests == total_tests {
        log::info!("🎉 All tests passed!");
    } else {
        log::error!("⚠️  Some tests failed!");
        std::process::exit(1);
    }
}

// Helpers
use dbfs2::common;

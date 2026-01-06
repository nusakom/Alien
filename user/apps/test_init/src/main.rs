#![no_std]
#![no_main]

extern crate alloc;

use Mstd::{
    fs::{mkdir, open, close, read, OpenFlags},
    println,
    process::{exec, exit, fork, wait, waitpid},
    system_shutdown,
};

/// DBFS Test Init Process
///
/// This init program runs automatically when the kernel boots.
/// It mounts DBFS to /tests and executes the DBFS test suite.
#[no_mangle]
fn main() -> isize {
    println!("========================================");
    println!("[MODE] DBFS_CORRECTNESS_TEST");
    println!("========================================");
    println!("🧪 DBFS Test Init Process");
    println!("Initializing DBFS test environment...\n");

    // Step 1: Create /tests mount point
    println!("📁 Step 1: Creating /tests mount point...");
    let ret = mkdir("/tests\0");
    if ret < 0 && ret != -17 {
        // -17 is EEXIST (directory already exists)
        println!("  ⚠️  Warning: Failed to create /tests directory (ret={})", ret);
    } else {
        println!("  ✅ /tests directory ready");
    }

    // Step 2: Mount DBFS from /dev/vda to /tests
    println!("\n📂 Step 2: Mounting DBFS filesystem...");
    println!("  Source: /dev/vda");
    println!("  Target: /tests");
    println!("  Type: fat32 (using FAT32 as DBFS backend)");

    // Note: For now we skip the mount operation in userspace
    // The filesystem should already be available at root
    // In a full DBFS implementation with mount syscall available to userspace,
    // we would call: mount("/dev/vda", "/tests", "dbfs", 0, NULL);
    println!("  ⚠️  Skipping mount (filesystem available at root)");

    // Step 3: Verify test binary is accessible
    println!("\n🔍 Step 3: Checking for DBFS test binary...");

    // Try multiple paths for dbfs_test
    let test_paths = [
        "/dbfs_test\0",       // Root directory
        "./dbfs_test\0",      // Current directory
        "/tests/dbfs_test\0", // Tests directory
    ];

    let mut found_path: Option<&str> = None;
    for path in test_paths.iter() {
        let fd = open(path, OpenFlags::O_RDONLY);
        if fd >= 0 {
            close(fd as usize);
            found_path = Some(*path);
            println!("  ✅ Found dbfs_test at: {}", path.trim_end_matches('\0'));
            break;
        }
    }

    if found_path.is_none() {
        println!("  ⚠️  DBFS test binary not found (optional for manual testing)");
    }

    // Step 4: Directly run dbfs_test and shutdown
    println!("\n🚀 Step 4: Running DBFS test suite...");
    println!("========================================\n");

    // Run dbfs_test directly
    let test_pid = fork();
    if test_pid == 0 {
        // Child process: execute dbfs_test
        let test_cmd = "/tests/dbfs_test\0";
        let cmd_ptr: *const u8 = test_cmd.as_ptr();
        let args = [cmd_ptr, core::ptr::null::<u8>()];
        exec(test_cmd, &args, TEST_ENV);
        println!("ERROR: Failed to exec dbfs_test");
        exit(1);
    } else {
        // Parent process: wait for test to complete
        let mut test_exit_code: i32 = 0;
        let _ = waitpid(test_pid as usize, &mut test_exit_code);

        println!("\n========================================");
        println!("DBFS test completed with exit code: {}", test_exit_code);
        println!("");
        println!("✅ All tests completed!");
        println!("🚀 Starting interactive shell...");
        println!("");
        println!("You can now run:");
        println!("  cd / && ./final_test       # Run Elle tests");
        println!("  cd /tests && ./dbfs_test   # Run DBFS tests again");
        println!("  ls /                       # Explore filesystem");
        println!("  exit                       # Shutdown system");
        println!("========================================");
        println!("");

        // Start interactive shell instead of shutting down
        start_shell();
    }

    0
}

const TEST_ENV: &[*const u8] = &[
    "PATH=/:/bin:/sbin:/tests\0".as_ptr(),
    "HOME=/root\0".as_ptr(),
    "USER=root\0".as_ptr(),
    core::ptr::null(),
];

/// Start an interactive shell
///
/// This function execs /bin/sh to provide an interactive shell
/// for manual testing and exploration.
fn start_shell() -> ! {
    // Try multiple possible shell locations
    let shell_paths = [
        "/bin/sh\0",
        "/bin/busybox\0",
        "/busybox\0",
    ];

    for path in shell_paths.iter() {
        let cmd_ptr: *const u8 = path.as_ptr();
        let args = [cmd_ptr, core::ptr::null::<u8>()];

        // Try to exec the shell
        exec(path, &args, TEST_ENV);

        // If exec fails, try next path
        // (note: exec only returns on failure)
    }

    // If all shells fail, print error and shutdown
    println!("ERROR: Failed to start shell. Tried:");
    for path in shell_paths.iter() {
        println!("  {}", path.trim_end_matches('\0'));
    }
    println!("");
    println!("Make sure busybox is statically linked and available.");
    println!("System will shutdown.");

    system_shutdown()
}

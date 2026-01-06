#![no_main]
#![no_std]

use Mstd::{
    println,
    process::{exec, exit, fork, waitpid},
    thread::m_yield,
};

#[no_mangle]
fn main() -> ! {
    println!("========================================");
    println!("🧪 Alien OS - Comprehensive Test Suite");
    println!("========================================");
    println!();

    let mut total_tests = 0;
    let mut passed_tests = 0;

    // Test 1: DBFS Correctness Test
    println!("📦 [1/6] Running DBFS Correctness Test...");
    if run_test("./dbfs_test\0") {
        passed_tests += 1;
    }
    total_tests += 1;
    println!();

    // Test 2: Dhrystone Benchmark (run for 5 seconds)
    println!("🔥 [2/6] Running Dhrystone Benchmark...");
    if run_test_with_args("/tests/dhry2\0", &["dhry2\0".as_ptr(), "5\0".as_ptr(), core::ptr::null()]) {
        passed_tests += 1;
    }
    total_tests += 1;
    println!();

    // Test 3: Arithmetic Benchmark (run for 5 seconds)
    println!("➕ [3/6] Running Arithmetic Benchmark...");
    if run_test_with_args("/tests/arithoh\0", &["arithoh\0".as_ptr(), "5\0".as_ptr(), core::ptr::null()]) {
        passed_tests += 1;
    }
    total_tests += 1;
    println!();

    // Test 4: System Call Benchmark
    println!("⚙️  [4/6] Running System Call Benchmark...");
    if run_test_with_args("/tests/syscall\0", &["syscall\0".as_ptr(), "mix\0".as_ptr(), core::ptr::null()]) {
        passed_tests += 1;
    }
    total_tests += 1;
    println!();

    // Test 5: Hackbench (Concurrency Test)
    println!("🔄 [5/6] Running Hackbench Concurrency Test...");
    if run_test("/tests/hackbench\0") {
        passed_tests += 1;
    }
    total_tests += 1;
    println!();

    // Test 6: Elle Distributed Systems Test
    println!("🔬 [6/6] Running Elle Distributed Systems Test...");
    println!("   Note: Full Elle test requires external setup.");
    println!("   See /tests/run_elle_test.sh for complete Elle test suite.");
    if run_test("/tests/elle_dbfs_client\0") {
        passed_tests += 1;
    } else {
        println!("   ℹ️  Elle client not available - run manually from host:");
        println!("      cd /tests && ./run_elle_test.sh");
    }
    total_tests += 1;
    println!();

    println!();
    println!("========================================");
    println!("📊 Test Results Summary");
    println!("========================================");
    println!("Total Tests:  {}", total_tests);
    println!("Passed:       {}", passed_tests);
    println!("Failed:       {}", total_tests - passed_tests);
    println!("Success Rate: {}%", (passed_tests * 100) / total_tests);
    println!("========================================");
    println!();
    println!("!TEST FINISH!");
    println!();
    println!("📝 Additional tests available:");
    println!("  cd /tests && ./unixbench_testcode.sh  # UnixBench suite");
    println!("  cd /tests && ./lmbench_testcode.sh    # lmbench suite");
    println!("  cd /tests && ./iozone_testcode.sh     # I/O performance");
    println!("  cd /tests && ./hackbench              # Concurrency test");
    println!();
    println!("🔬 Elle + Jepsen distributed testing:");
    println!("  cd /tests && ./run_elle_test.sh       # Full Elle test suite (requires host setup)");
    println!();
    println!("🚀 Process will keep running.");
    println!("   Use Ctrl+A, X to exit QEMU.");
    println!("========================================");

    // Keep the process running so the system doesn't shutdown
    loop {
        m_yield();
    }
}

/// Run a test program and return true if it succeeded (exit code 0)
fn run_test(path: &str) -> bool {
    let args = [path.as_ptr(), core::ptr::null::<u8>()];
    let pid = fork();

    if pid == 0 {
        // Child process
        exec(path, &args, TEST_ENV);
        println!("❌ Failed to exec: {}", path.trim_end_matches('\0'));
        exit(1);
    } else {
        // Parent process
        let mut exit_code: i32 = 0;
        let _ = waitpid(pid as usize, &mut exit_code);

        if exit_code == 0 {
            println!("✅ {} - PASSED", path.trim_end_matches('\0'));
            true
        } else {
            println!("❌ {} - FAILED (exit code: {})", path.trim_end_matches('\0'), exit_code);
            false
        }
    }
}

/// Run a test program with arguments and return true if it succeeded (exit code 0)
fn run_test_with_args(path: &str, args: &[*const u8]) -> bool {
    let pid = fork();

    if pid == 0 {
        // Child process
        exec(path, args, TEST_ENV);
        println!("❌ Failed to exec: {}", path.trim_end_matches('\0'));
        exit(1);
    } else {
        // Parent process
        let mut exit_code: i32 = 0;
        let _ = waitpid(pid as usize, &mut exit_code);

        if exit_code == 0 {
            println!("✅ {} - PASSED", path.trim_end_matches('\0'));
            true
        } else {
            println!("❌ {} - FAILED (exit code: {})", path.trim_end_matches('\0'), exit_code);
            false
        }
    }
}

const TEST_ENV: &[*const u8] = &[
    "PATH=/:/bin:/sbin:/tests\0".as_ptr(),
    "HOME=/root\0".as_ptr(),
    "USER=root\0".as_ptr(),
    core::ptr::null(),
];
const BASH_ENV: &[*const u8] = &[
    "SHELL=/bash\0".as_ptr(),
    "PWD=/\0".as_ptr(),
    "LOGNAME=root\0".as_ptr(),
    "MOTD_SHOWN=pam\0".as_ptr(),
    "HOME=/root\0".as_ptr(),
    "LANG=C.UTF-8\0".as_ptr(),
    "TERM=vt220\0".as_ptr(),
    "USER=root\0".as_ptr(),
    "SHLVL=0\0".as_ptr(),
    "OLDPWD=/root\0".as_ptr(),
    "PS1=\x1b[1m\x1b[32mAlien\x1b[0m:\x1b[1m\x1b[34m\\w\x1b[0m\\$ \0".as_ptr(),
    "_=/bin/bash\0".as_ptr(),
    "PATH=/:/bin:/sbin:/tests\0".as_ptr(),
    "LD_LIBRARY_PATH=/tests:/bin\0".as_ptr(),
    core::ptr::null(),
];

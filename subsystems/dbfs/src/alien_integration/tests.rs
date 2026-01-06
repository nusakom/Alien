//! DBFS事务性测试
//!
//! 测试 WAL、事务管理、文件操作的事务性

use alloc::string::String;
use alloc::format;
use crate::wal::{TxId, Wal, WalRecord, WalRecordType};
use log::info;

/// 测试 1: WAL 序列化/反序列化
pub fn test_wal_serialize() -> bool {
    info!("\n🔬 Test 1: WAL Serialization");

    let tx_id = TxId::new(1);
    let data = b"test data".to_vec();
    let record = WalRecord::new(tx_id, WalRecordType::TxBegin, data.clone());

    // Serialize
    let bytes = record.serialize();
    info!("  Serialized {} bytes", bytes.len());

    // Deserialize
    match WalRecord::deserialize(&bytes) {
        Ok(deserialized) => {
            if deserialized.tx_id == tx_id && deserialized.record_type == WalRecordType::TxBegin {
                info!("  ✅ WAL serialization successful");
                true
            } else {
                info!("  ❌ Deserialized data mismatch");
                false
            }
        }
        Err(e) => {
            info!("  ❌ Deserialization failed: {:?}", e);
            false
        }
    }
}

/// 测试 2: 事务 begin/commit
pub fn test_transaction_begin_commit() -> bool {
    info!("\n🔬 Test 2: Transaction Begin/Commit");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // Begin transaction
    let tx_id = wal.begin_tx();
    info!("  Transaction {} started", tx_id);

    // Commit transaction
    match wal.commit_tx(tx_id) {
        Ok(_) => {
            info!("  ✅ Transaction {} committed", tx_id);
            true
        }
        Err(e) => {
            info!("  ❌ Commit failed: {:?}", e);
            false
        }
    }
}

/// 测试 3: 文件操作记录
pub fn test_file_operations() -> bool {
    info!("\n🔬 Test 3: File Operations");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    let tx_id = wal.begin_tx();

    // Record file operations
    wal.write_file(tx_id, "/test.txt", 0, b"Hello");
    wal.create_file(tx_id, "/newfile.txt");
    wal.delete_file(tx_id, "/oldfile.txt");
    wal.mkdir(tx_id, "/newdir");

    info!("  Recorded 4 operations");

    // Commit
    match wal.commit_tx(tx_id) {
        Ok(_) => {
            info!("  ✅ File operations recorded and committed");
            true
        }
        Err(e) => {
            info!("  ❌ Commit failed: {:?}", e);
            false
        }
    }
}

/// 测试 4: 崩溃恢复
pub fn test_crash_recovery() -> bool {
    info!("\n🔬 Test 4: Crash Recovery");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // Simulate committed transaction
    let tx1 = wal.begin_tx();
    wal.write_file(tx1, "/file1.txt", 0, b"Data 1");
    wal.commit_tx(tx1).unwrap();

    // Simulate uncommitted transaction
    let tx2 = wal.begin_tx();
    wal.write_file(tx2, "/file2.txt", 0, b"Data 2");
    // Don't commit tx2

    // Recover
    match wal.recover() {
        Ok(recovery) => {
            info!("  Found {} committed transactions", recovery.committed.len());
            info!("  Found {} uncommitted transactions", recovery.uncommitted.len());

            if recovery.committed.len() == 1 && recovery.uncommitted.len() == 1 {
                info!("  ✅ Crash recovery successful");
                true
            } else {
                info!("  ❌ Recovery result incorrect");
                false
            }
        }
        Err(e) => {
            info!("  ❌ Recovery failed: {:?}", e);
            false
        }
    }
}

/// 测试 5: 多个连续事务
pub fn test_multiple_transactions() -> bool {
    info!("\n🔬 Test 5: Multiple Transactions");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // Execute 3 transactions
    for i in 1..=3 {
        let tx_id = wal.begin_tx();
        wal.write_file(tx_id, &format!("/file{}.txt", i), 0, b"test");
        wal.commit_tx(tx_id).unwrap();
    }

    // Verify transaction count
    let next_tx = wal.next_tx_id();

    if next_tx == 4 {
        info!("  ✅ Multiple transactions successful");
        true
    } else {
        info!("  ❌ Expected 4 transactions, got {}", next_tx);
        false
    }
}

/// 运行所有测试
pub fn run_all_tests() -> (usize, usize) {
    info!("========================================");
    info!("DBFS 事务性测试套件");
    info!("========================================");

    let mut passed = 0;
    let mut total = 0;

    // Use function pointers to avoid type mismatch
    let tests: &[(&str, fn() -> bool)] = &[
        ("WAL Serialization", test_wal_serialize),
        ("Transaction Begin/Commit", test_transaction_begin_commit),
        ("File Operations", test_file_operations),
        ("Crash Recovery", test_crash_recovery),
        ("Multiple Transactions", test_multiple_transactions),
    ];

    for (name, test_fn) in tests.iter() {
        total += 1;
        info!("\nRunning: {}", name);
        if test_fn() {
            passed += 1;
        }
    }

    info!("\n========================================");
    info!("测试结果: {}/{} 通过", passed, total);
    info!("========================================");

    (passed, total)
}
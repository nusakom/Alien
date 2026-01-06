//! DBFS 增强测试套件
//!
//! 从基础测试到全面验证

use alloc::{format, string::String, vec::Vec};
use crate::wal::{TxId, Wal, WalRecord, WalRecordType};
use log::info;

// ==================== 原有测试 ====================

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

// ==================== 新增测试 ====================

/// 测试 6: WAL Checkpoint 和 Truncate
pub fn test_checkpoint_truncate() -> bool {
    info!("\n🔬 Test 6: WAL Checkpoint and Truncate");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // 创建 10 个事务
    for i in 1..=10 {
        let tx_id = wal.begin_tx();
        wal.write_file(tx_id, &format!("/file{}.txt", i), 0, b"data");
        wal.commit_tx(tx_id).unwrap();
    }

    info!("  Created 10 transactions");

    // Truncate 到 LSN 5 (保留 5-10)
    wal.truncate(5);

    // 验证旧记录被删除
    let records = wal.get_tx_records(TxId::new(1));

    if records.is_empty() {
        info!("  ✅ Truncated records removed successfully");
        true
    } else {
        info!("  ❌ Truncated records still exist");
        false
    }
}

/// 测试 7: 事务 Rollback
pub fn test_transaction_rollback() -> bool {
    info!("\n🔬 Test 7: Transaction Rollback");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // Begin transaction
    let tx_id = wal.begin_tx();
    wal.write_file(tx_id, "/temp.txt", 0, b"temporary");

    // Rollback instead of commit
    wal.rollback_tx(tx_id);

    // Recover should show no committed transactions
    match wal.recover() {
        Ok(recovery) => {
            if recovery.committed.is_empty() && recovery.uncommitted.len() == 1 {
                info!("  ✅ Rolled back transaction not committed");
                true
            } else {
                info!("  ❌ Rollback state incorrect");
                false
            }
        }
        Err(_) => {
            info!("  ❌ Recovery failed");
            false
        }
    }
}

/// 测试 8: LSN 顺序性
pub fn test_lsn_sequencing() -> bool {
    info!("\n🔬 Test 8: LSN Sequencing");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    let tx1 = wal.begin_tx();
    wal.write_file(tx1, "/file1.txt", 0, b"data1");

    let tx2 = wal.begin_tx();
    wal.write_file(tx2, "/file2.txt", 0, b"data2");

    // 获取所有记录并验证 LSN 严格递增
    let records1 = wal.get_tx_records(tx1);
    let records2 = wal.get_tx_records(tx2);

    if records1.len() > 0 && records2.len() > 0 {
        let last_lsn_tx1 = records1.last().unwrap().lsn;
        let first_lsn_tx2 = records2.first().unwrap().lsn;

        if last_lsn_tx1 < first_lsn_tx2 {
            info!("  ✅ LSNs are strictly increasing");
            info!("     TX1 last LSN: {}, TX2 first LSN: {}", last_lsn_tx1, first_lsn_tx2);
            true
        } else {
            info!("  ❌ LSN ordering violated");
            false
        }
    } else {
        info!("  ❌ No records found");
        false
    }
}

/// 测试 9: 空 WAL 恢复
pub fn test_empty_wal_recovery() -> bool {
    info!("\n🔬 Test 9: Empty WAL Recovery");

    let wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let wal = wal.unwrap();

    match wal.recover() {
        Ok(recovery) => {
            if recovery.committed.is_empty() && recovery.uncommitted.is_empty() {
                info!("  ✅ Empty WAL recovery successful");
                true
            } else {
                info!("  ❌ Empty WAL has transactions");
                false
            }
        }
        Err(_) => {
            info!("  ❌ Recovery failed");
            false
        }
    }
}

/// 测试 10: 大文件写入
pub fn test_large_file_write() -> bool {
    info!("\n🔬 Test 10: Large File Write");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    let tx_id = wal.begin_tx();

    // 写入 10KB 数据
    let large_data: Vec<u8> = alloc::vec![0u8; 10 * 1024];
    wal.write_file(tx_id, "/large.bin", 0, &large_data);

    match wal.commit_tx(tx_id) {
        Ok(_) => {
            let records = wal.get_tx_records(tx_id);
            if records.len() > 0 {
                info!("  ✅ Large file (10KB) written successfully");
                true
            } else {
                info!("  ❌ No records found for large file");
                false
            }
        }
        Err(e) => {
            info!("  ❌ Commit failed: {:?}", e);
            false
        }
    }
}

/// 测试 11: 深度嵌套目录
pub fn test_deep_nested_directories() -> bool {
    info!("\n🔬 Test 11: Deep Nested Directories");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    let tx_id = wal.begin_tx();

    // 创建深度嵌套目录
    let deep_path = "/a/b/c/d/e/f/g/h/i/j";
    wal.mkdir(tx_id, deep_path);

    match wal.commit_tx(tx_id) {
        Ok(_) => {
            info!("  ✅ Deep nested directory (10 levels) created");
            true
        }
        Err(e) => {
            info!("  ❌ Commit failed: {:?}", e);
            false
        }
    }
}

/// 测试 12: 混合操作序列
pub fn test_mixed_operations() -> bool {
    info!("\n🔬 Test 12: Mixed Operations Sequence");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    let tx_id = wal.begin_tx();

    // 混合操作: 创建文件,写入,删除,创建目录
    wal.create_file(tx_id, "/file1.txt");
    wal.write_file(tx_id, "/file1.txt", 0, b"data1");
    wal.mkdir(tx_id, "/dir1");
    wal.write_file(tx_id, "/file1.txt", 100, b"data2"); // 追加写入
    wal.create_file(tx_id, "/dir1/file2.txt");

    match wal.commit_tx(tx_id) {
        Ok(_) => {
            let records = wal.get_tx_records(tx_id);
            info!("  ✅ Mixed operations (6 ops) committed");
            info!("     Total records: {}", records.len());
            true
        }
        Err(e) => {
            info!("  ❌ Commit failed: {:?}", e);
            false
        }
    }
}

/// 测试 13: 多次 Rollback 和 Commit
pub fn test_rollback_commit_cycles() -> bool {
    info!("\n🔬 Test 13: Rollback/Commit Cycles");

    let mut wal = Wal::new(String::from("/test/wal"));
    if wal.is_err() {
        info!("  ❌ Failed to create WAL");
        return false;
    }
    let mut wal = wal.unwrap();

    // 循环: commit, rollback, commit
    for i in 1..=3 {
        let tx_commit = wal.begin_tx();
        wal.write_file(tx_commit, &format!("/commit{}.txt", i), 0, b"keep");
        wal.commit_tx(tx_commit).unwrap();

        let tx_rollback = wal.begin_tx();
        wal.write_file(tx_rollback, &format!("/rollback{}.txt", i), 0, b"discard");
        wal.rollback_tx(tx_rollback);
    }

    // 恢复: 应该有 3 个已提交,3 个未提交
    match wal.recover() {
        Ok(recovery) => {
            if recovery.committed.len() == 3 && recovery.uncommitted.len() == 3 {
                info!("  ✅ Mixed commit/rollback cycles successful");
                info!("     Committed: {}, Rolled back: {}",
                      recovery.committed.len(), recovery.uncommitted.len());
                true
            } else {
                info!("  ❌ Recovery count incorrect");
                false
            }
        }
        Err(_) => false
    }
}

// ==================== 运行所有测试 ====================

/// 运行所有测试 (增强版)
pub fn run_all_tests() -> (usize, usize) {
    info!("========================================");
    info!("DBFS 增强测试套件");
    info!("========================================");

    let mut passed = 0;
    let mut total = 0;

    let tests: &[(&str, fn() -> bool)] = &[
        // 原有测试
        ("WAL Serialization", test_wal_serialize),
        ("Transaction Begin/Commit", test_transaction_begin_commit),
        ("File Operations", test_file_operations),
        ("Crash Recovery", test_crash_recovery),
        ("Multiple Transactions", test_multiple_transactions),

        // 新增测试
        ("Checkpoint and Truncate", test_checkpoint_truncate),
        ("Transaction Rollback", test_transaction_rollback),
        ("LSN Sequencing", test_lsn_sequencing),
        ("Empty WAL Recovery", test_empty_wal_recovery),
        ("Large File Write", test_large_file_write),
        ("Deep Nested Directories", test_deep_nested_directories),
        ("Mixed Operations", test_mixed_operations),
        ("Rollback/Commit Cycles", test_rollback_commit_cycles),
    ];

    for (name, test_fn) in tests.iter() {
        total += 1;
        info!("\nRunning: {}", name);
        if test_fn() {
            passed += 1;
        }
    }

    info!("\n========================================");
    info!("🏁 测试结果: {}/{} 通过", passed, total);
    if passed == total {
        info!("🎉 所有测试通过!");
    } else {
        info!("⚠️  {} 个测试失败", total - passed);
    }
    info!("========================================");

    (passed, total)
}

/// 运行基础测试 (5个)
pub fn run_basic_tests() -> (usize, usize) {
    info!("========================================");
    info!("DBFS 基础测试套件");
    info!("========================================");

    let mut passed = 0;
    let mut total = 0;

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
    info!("🏁 测试结果: {}/{} 通过", passed, total);
    info!("========================================");

    (passed, total)
}
# DBFS Architecture Design

## Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│  (dbfs_test, user applications)                               │
└────────────────────────┬────────────────────────────────────┘
                         │ POSIX syscalls
                         ↓
┌─────────────────────────────────────────────────────────────┐
│                    VFS Layer (vfscore)                       │
│  VfsPath / VfsFile / VfsDentry / VfsInode / VfsSuperBlock    │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ↓
┌─────────────────────────────────────────────────────────────┐
│                    DBFS Transaction Layer                    │
│                                                               │
│  ┌────────────────────────────────────────────────────────┐ │
│  │           TransactionManager                             │ │
│  │  - begin_transaction()                                   │ │
│  │  - commit_transaction()                                  │ │
│  │  - rollback_transaction()                                │ │
│  └──────────────────────┬─────────────────────────────────┘ │
│                         ↓                                     │
│  ┌────────────────────────────────────────────────────────┐ │
│  │           WriteAheadLog (WAL)                            │ │
│  │  - Begin/Commit/Rollback entries                        │ │
│  │  - Operation logging                                    │ │
│  │  - Crash recovery                                        │ │
│  └──────────────────────┬─────────────────────────────────┘ │
│                         ↓                                     │
│  ┌────────────────────────────────────────────────────────┐ │
│  │           Transaction State                             │ │
│  │  - Active / Committed / RolledBack                       │ │
│  │  - Operation tracking                                    │ │
│  │  - Undo/Redo support                                     │ │
│  └────────────────────────────────────────────────────────┘ │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ↓
┌─────────────────────────────────────────────────────────────┐
│              Underlying Filesystem Layer                     │
│  FatFs / ExtFs / Raw Block Device                            │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ↓
┌─────────────────────────────────────────────────────────────┐
│                    Block Device Layer                        │
│  /dev/sda / /dev/vda                                        │
└─────────────────────────────────────────────────────────────┘
```

## Transaction Flow

### Begin Transaction

```
1. Application calls dbfs::begin_transaction()
2. TransactionManager generates unique TransactionId
3. WriteAheadLog writes Begin entry
4. Transaction created in Active state
5. Return TransactionId to application
```

### Perform Operations

```
1. Application performs file operations (read/write/create/delete)
2. Each operation is wrapped in TransactionOperation
3. Operation is added to transaction's operation list
4. WriteAheadLog writes Data entry for each operation
5. Original operations are NOT executed yet (deferred)
```

### Commit Transaction

```
1. Application calls dbfs::commit_transaction(tx_id)
2. TransactionManager validates transaction
3. WriteAheadLog writes Commit entry
4. WAL is flushed to disk (fsync)
5. All operations are applied to underlying filesystem
6. Transaction marked as Committed
7. Return success
```

### Rollback Transaction

```
1. Application calls dbfs::rollback_transaction(tx_id)
2. TransactionManager finds transaction
3. WriteAheadLog writes Rollback entry
4. Undo all operations in reverse order
5. Transaction marked as RolledBack
6. Return success
```

## Crash Recovery

### After System Crash

```
1. System restarts
2. DBFS init() is called
3. WriteAheadLog.recover() is invoked
4. WAL is analyzed from beginning:
   - Find all Begin entries
   - Match with Commit entries
   - Identify uncommitted transactions
5. For each uncommitted transaction:
   - Rollback all operations
   - Mark as RolledBack
6. System proceeds with consistent state
```

## ACID Guarantees

### Atomicity
- All operations in a transaction are atomic
- Either all succeed or all fail
- Achieved via:
  - Deferred execution (operations only applied at commit)
  - WAL logging (replay capability)
  - Rollback mechanism (undo capability)

### Consistency
- Filesystem state is always valid
- No orphaned files or partial writes
- Achieved via:
  - Transaction validation
  - Atomic operation application
  - Rollback of incomplete transactions

### Isolation
- Concurrent transactions don't interfere
- Each transaction sees consistent snapshot
- Achieved via (future implementation):
  - MVCC (Multi-Version Concurrency Control)
  - Snapshot isolation
  - Lock management

### Durability
- Committed transactions survive crashes
- Achieved via:
  - Write-Ahead Logging
  - WAL flush to disk before commit returns
  - Crash recovery mechanism

## Data Structures

### TransactionManager
```rust
pub struct TransactionManager {
    active_transactions: Mutex<Vec<Transaction>>,
    wal: Mutex<WriteAheadLog>,
}
```

### Transaction
```rust
pub struct Transaction {
    id: TransactionId,
    operations: Vec<TransactionOperation>,
    state: TransactionState,
}
```

### WriteAheadLog
```rust
pub struct WriteAheadLog {
    log_file: Option<alloc::string::String>,
    log_entries: Mutex<Vec<WalEntry>>,
}
```

### TransactionOperation
```rust
pub enum TransactionOperation {
    Write { path: String, offset: u64, data: Vec<u8> },
    Create { path: String },
    Delete { path: String },
}
```

## Integration Points

### With VFS
```rust
// In subsystems/vfs/src/lib.rs
pub fn mount_dbfs(
    device: Arc<dyn VfsInode>,
    target: &VfsPath,
) -> AlienResult<Arc<dyn VfsDentry>> {
    // Create DBFS layer
    let dbfs = dbfs::DbFsType::new();
    dbfs.mount(target)
}
```

### With Applications
```rust
// User application
use dbfs::{TransactionManager, TransactionOperation};

let manager = TransactionManager::new();
let tx_id = manager.begin_transaction()?;

manager.add_operation(tx_id, TransactionOperation::Write {
    path: "/test.txt".into(),
    offset: 0,
    data: b"Hello, DBFS!".to_vec(),
});

manager.commit_transaction(tx_id)?;
```

## Performance Considerations

### WAL Optimization
- Group commit: Batch multiple transactions
- Checkpointing: Periodic WAL truncation
- Compression: Compress WAL entries

### Transaction Optimization
- Early lock release
- Operation coalescing
- Lazy evaluation

### Recovery Optimization
- Parallel recovery
- Incremental checkpointing
- Dirty page tracking

## Future Enhancements

1. **Persistent WAL**
   - Store WAL on disk instead of memory
   - Implement WAL rotation
   - Add WAL checksums

2. **Concurrency Control**
   - Implement locking mechanisms
   - Add deadlock detection
   - Support for parallel transactions

3. **Performance**
   - Add caching layer
   - Implement operation batching
   - Optimize WAL flush frequency

4. **Advanced Features**
   - Savepoints within transactions
   - Nested transactions
   - Distributed transactions

## Testing Strategy

### Unit Tests
- TransactionManager tests
- WAL tests
- Recovery tests

### Integration Tests
- VFS integration
- Crash recovery
- Concurrent transactions

### Stress Tests
- Large transactions
- Many concurrent transactions
- Crash during commit

## References

- [SQLite Transaction Management](https://www.sqlite.org/transactionintro.html)
- [PostgreSQL WAL](https://www.postgresql.org/docs/current/wal.html)
- [MySQL InnoDB Transactions](https://dev.mysql.com/doc/refman/8.0/en/innodb-transaction-model.html)
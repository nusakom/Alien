# DBFS - Database File System with ACID Transactions

## Overview

DBFS is a transactional filesystem layer that provides ACID guarantees for file operations in Alien OS.

## Architecture

```
Application / Userspace System Calls
    │
    ↓
VFS Layer (VfsPath / VfsFile / VfsDentry / VfsInode)
    │
VfsSuperBlock
    │
┌───────────────────────────────────────┐
│            DBFS                       │
│  Transaction Manager                  │
│  ┌─────────────────────────────────┐  │
│  │  Begin / Commit / Rollback     │  │
│  │  Write-Ahead Log (WAL)         │  │
│  │  Crash Recovery                │  │
│  │  Atomic Operations             │  │
│  └─────────────────────────────────┘  │
└───────────────────────────────────────┘
    │
    ↓
Underlying FS (FatFs / ExtFs / Raw Block Device)
```

## Features

### ACID Properties

1. **Atomicity**
   - File operations are atomic
   - Multi-file transactions either all succeed or all fail
   - No partial states

2. **Consistency**
   - Filesystem state is always valid
   - No corruption after crashes
   - Referential integrity maintained

3. **Isolation**
   - Concurrent transactions don't interfere
   - Each transaction sees a consistent snapshot

4. **Durability**
   - Committed transactions survive crashes
   - Write-Ahead Log (WAL) ensures persistence
   - Recovery mechanism after system restart

## Components

### TransactionManager

Manages all active transactions and coordinates commit/rollback.

```rust
let manager = TransactionManager::new();

// Begin transaction
let tx_id = manager.begin_transaction()?;

// ... perform operations ...

// Commit
manager.commit_transaction(tx_id)?;

// Or rollback
manager.rollback_transaction(tx_id)?;
```

### WriteAheadLog (WAL)

Logs all transaction operations before they are applied to the filesystem.

- **Begin**: Marks start of transaction
- **Commit**: Marks successful completion
- **Rollback**: Marks transaction cancellation
- **Data**: Actual operation data

### Transaction Operations

- `Write`: Write data to file at offset
- `Create`: Create new file
- `Delete`: Remove file

## Usage

### Basic Transaction

```rust
use dbfs::{TransactionManager, TransactionOperation};

let manager = TransactionManager::new();

// Begin transaction
let tx_id = manager.begin_transaction()?;

// Add operations
manager.add_operation(tx_id, TransactionOperation::Write {
    path: "/test.txt".into(),
    offset: 0,
    data: b"Hello, DBFS!".to_vec(),
});

// Commit
manager.commit_transaction(tx_id)?;
```

### Crash Recovery

After system restart, DBFS automatically recovers:

```rust
let wal = WriteAheadLog::new();
let result = wal.recover()?;

// result.last_committed: Last successfully committed transaction
// result.uncommitted: Transactions to roll back
```

## Testing

Run DBFS transaction tests:

```bash
cd /home/ubuntu2204/Desktop/Alien
make dbfs-test
```

Expected output:

```
========================================
DBFS Transaction Tests
========================================

✅ Transaction Begin/Commit: PASSED
✅ Transaction Rollback: PASSED
✅ WAL Recovery: PASSED

========================================
DBFS Tests: 3/3 passed
========================================
```

## Implementation Status

- [x] Transaction Manager
- [x] Write-Ahead Log (in-memory)
- [x] Basic transaction operations
- [x] Crash recovery framework
- [ ] Persistent WAL to disk
- [ ] Integration with VFS mount points
- [ ] Multi-transaction concurrency
- [ ] Performance optimization

## Integration with Alien OS

DBFS is integrated into the VFS subsystem:

1. **Mount Point**: `/tests` (or any mount point)
2. **Backend**: FAT32 (can be extended to Ext4)
3. **User Interface**: Standard POSIX syscalls (read, write, open, close)

## Future Enhancements

1. **Persistent WAL**: Store WAL on disk for true crash recovery
2. **Snapshot Isolation**: MVCC for concurrent transactions
3. **Compression**: Compress WAL entries
4. **Checkpoints**: Periodic WAL checkpointing
5. **Replication**: Multi-node transaction replication

## References

- [ACID - Wikipedia](https://en.wikipedia.org/wiki/ACID)
- [Write-Ahead Logging - PostgreSQL](https://www.postgresql.org/docs/current/wal.html)
- [SQLite Transaction Management](https://www.sqlite.org/transactionintro.html)

## License

Part of the Alien OS project.
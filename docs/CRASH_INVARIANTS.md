# Alien OS Crash Consistency Invariants

为了验证 Alien OS (基于 DBFS) 的正确性，我们定义了以下必须在任何崩溃场景下始终满足的不变量 (Invariants)。

## 1. 基础事务属性 (Transactional Properties)

### Invariant 1.1: 原子性 (Atomicity)
**定义**: 对于任意事务 $T$，其包含的所有写操作集合 $W_T = \{w_1, w_2, ..., w_n\}$。在系统崩溃并恢复后，持久化存储中的状态必须满足：要么 $W_T$ 中的所有更新全部可见，要么全部不可见。
**验证标准**:
- 检查数据库中是否存在 partial update（例如：key A 更新了，但同一事务中的 key B 还是旧值）。

### Invariant 1.2: 持久性 (Durability / Committed-means-Persisted)
**定义**: 一旦 `commit()` 操作向用户返回“成功”，则该事务 $T$ 的所有变更必须永久存储在介质上。
**验证标准**:
- 如果测试脚本在收到 `commit` 成功响应后立即切断电源，重启后必须能读到 $T$ 的数据。

---

## 2. WAL (Write-Ahead Log) 结构属性

### Invariant 2.1: 日志前缀完整性 (Log Prefix Integrity)
**定义**: 恢复程序读取 WAL 时，必须能够通过 Checksum 验证所有条目的完整性。如果发现某个条目损坏（checksum mismatch），则该条目及之后的所有条目必须被丢弃。
**验证标准**:
- 有效的日志条目序列必须是连续的，中间不能有损坏的“空洞”。

### Invariant 2.2: 幂等重放 (Idempotent Replay)
**定义**: 对同一个 WAL 文件进行多次重放 (Replay)，系统的最终状态必须完全一致。
**验证标准**:
- $State_{replay1} == State_{replay2}$。这保证了哪怕在恢复过程中再次崩溃，系统也不会损坏。

---

## 3. 文件系统语义属性 (Filesystem Semantics)

这些属性是 DBFS 特有的，映射到 POSIX 语义：

### Invariant 3.1: 目录项一致性 (Dentry Consistency)
**定义**: 如果一个文件 $F$ 被创建并链接到目录 $D$，则在事务提交后：
1. $D$ 的子项列表中必须包含 $F$。
2. $F$ 的 inode 数据必须存在。
**反例**: 目录里看到了文件名，但读取时提示 "Inode Not Found"（这是 ext4 在某些非 journaling 模式下的经典 bug，Alien OS 必须避免）。

### Invariant 3.2: 跨目录移动原子性 (Rename Atomicity)
**定义**: `rename(A/file, B/file)` 操作是一个原子事务。
**验证标准**:
- 崩溃后，文件必须出现在 $A$ 或 $B$ 中的某一个，**绝不能同时出现在两个目录中，也绝不能同时消失**。

## 4. 验证策略 (Simulator Logic)

我们将编写 `FaultyBlockDevice` 来模拟：
1.  **Bit-perfect writes**: 写入完全成功。
2.  **Flying writes**: 写入在飞行中丢失。
3.  **Partial sector**: (可选) 扇区写了一半断电（依赖 checksum 识别）。

测试流程：
```rust
loop {
    let trace = run_workload_on_instrumented_device();
    for i in 0..trace.len() {
        let damaged_img = apply_writes(trace[0..i]); // Cut power at step i
        let recovered_db = Dbfs::mount(damaged_img);
        check_invariants(recovered_db); // Must pass ALL 1.1 - 3.2
    }
}
```

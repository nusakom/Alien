# DBFS2 / Alien 修复记录

这次主要修了 3 个文件系统相关问题，另外对测试环境的 `namegen` 做了一处修改。

## 1. 修复 symlink 对悬空链接和自环链接处理不正确的问题

文件：

`_dbfs2_alien/vendor/vfscore-patched/src/path.rs`

修改：

```rust
self.open(None)
```

改成：

```rust
self.open_nofollow(None)
```

原来的实现会跟随 newpath 最后一级的符号链接，导致 `symlink()` 在目标是悬空符号链接或者自环符号链接时行为不符合 POSIX。

修复前：

* 普通文件：`EEXIST`
* 目录：`EEXIST`
* FIFO：`EEXIST`
* 有效符号链接：`EEXIST`
* 悬空符号链接：返回 `0`，并把原来的链接替换掉
* 自环符号链接：返回 `ELOOP`

修复后，以上 6 种情况全部返回 `EEXIST`，而且原来的 inode 没有发生变化。

这说明问题确实来自原来的末级路径解析会跟随符号链接。

---

## 2. 修复 rename(x, x) 行为错误

文件：

`_dbfs2_alien/dbfs2/src/inode.rs`

函数：

`dbfs_common_rename()`

修改：

增加同一 inode 判断：

```rust
if new_number == Some(old_number) {
    return Ok(());
}
```

原来 `rename(x, x)` 会出现两类问题：

* 普通文件、目录、符号链接直接返回 `EIO`
* 两个硬链接指向同一个 inode 时，虽然返回成功，但会错误删除其中一个名字，同时导致 `nlink` 减少

修复后：

* `rename(x, x)` 返回 `0`
* `rename(f, ./f)` 返回 `0`
* `rename(f, ../Q/f)` 等价情况也返回 `0`
* 同一个 inode 的两个硬链接不会再被错误删除
* inode、mode、nlink、size 等信息保持不变

这个判断放在 DBFS2 层，是因为当前适配层没有真正的 dentry 缓存，最终能够可靠判断两个路径是否指向同一个 inode 的信息就是 inode number。

---

## 3. 修复 rename 到 `.` / `..` 导致内核 panic

文件：

`_dbfs2_alien/vendor/vfscore-patched/src/path.rs`

函数：

`VfsPath::rename_to()`

增加了对最后一级路径的判断，如果目标是：

```text
.
..
```

直接返回 `EBUSY`。

之前的情况比较严重：

```text
rename A/B/. C
```

可能返回成功，并导致目录出现两个名字。

随后执行：

```text
rename A/B/.. C
```

会触发 JammDB 的错误：

```text
Cannot delete data from a deleted bucket
```

最终导致 Alien 内核 panic。

修复后：

```text
rename A/B/. C
rename A/B/.. C
```

都会返回 `EBUSY`，不会创建错误的目录项，也不会再触发内核 panic。

这次修复比较重要，因为之前跑 `rename` 测试时，整个测试批次会因为这个问题直接中断。

---

## 4. 测试环境的 namegen 问题

这一项不是 DBFS2 文件系统代码修改，只是测试环境修复。

Alien 目前的 `/dev/urandom` 实现实际上只取了：

```c
read_timer() as u8
```

所以只有 8 bit，有效空间最多 256 种。

pjdfstest 的 `namegen` 又依赖这个随机数生成测试文件名，导致大量测试之间出现重名，产生了一些假失败。

因此测试环境中的 `namegen` 改成了：

```text
单调计数器 + PID
```

从而避免测试文件名重复。

这项修改不作为 DBFS2 功能修改统计。

---

## 5. 当前结果

目前这 3 个 DBFS2 / vfscore 问题都已经通过独立探针验证：

* `symlink()` 对悬空链接和自环链接能够正确返回 `EEXIST`
* `rename(x, x)` 能够正确返回成功，并保持 inode 信息不变
* `rename` 到 `.` / `..` 不再导致 Alien 内核 panic

`rename` 测试之前会因为 panic 导致整个测试批次中断，修复后已经能够正常跑完整个测试目录。

目前不再继续扩大这部分修改，后面准备把重点转到 DBFS2 的性能测试和 I/O 优化上。

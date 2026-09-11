# 供应链补丁明细（vendor/）

根 `Cargo.toml` 用 `[patch]` 把 dbfs2 依赖树里的 git 源重定向到本仓库 `vendor/`。下面是每个补丁
的 why / what / 位置，便于审计与在别处复现。**不要用上游裸版替换这些文件**（core2 会 yank 装不上、
rvfs/jammdb 会在 riscv64 / 写负载下报错）。

---

## 1. `vendor/core2-0.4.0`

- **why**：`core2` 在 crates.io 已**全量 yank（含 0.4.0）**；而 `jammdb` 和 Alien `vfs` 的
  `initrd` feature 都硬依赖 `core2="0.4"`。不 patch 则解析失败装不上。
- **what**：从 `.crate` 原样 vendored，**无代码改动**，仅作为可用的 yank 替代源。

---

## 2. `vendor/rvfs-patched`（Godones/rvfs；dbfs2 的 `rvfs_old`）

- **why**：dbfs2 把旧 rvfs 作为 `rvfs_old` 依赖。Godones/rvfs 在 **riscv64**（`c_char = u8`）
  下有个移植 bug，Alien 上编译报 `E0308`。
- **what**：`src/dentry/define.rs` 里把 `name` 由 cast 成 `*const i8` 改为 cast 成
  `*const core::ffi::c_char`（riscv64 下 c_char 是 u8）。
- **注意**：macOS host 上 `c_char = i8`，所以这 bug 在 host 编译被掩盖；只有 riscv64 target 才暴露。

---

## 3. `vendor/jammdb-patched`（Godones/jammdb memfile 后端，两处对齐 bug）

jammdb 的**内存后端**（`src/fs/memfile.rs`）是 DBFS2 用的存储。两个 bug 只有**真实写负载**
才暴露（简单单 bucket 探针即可复现，host 与 QEMU 同受影响、后端无关）：

### 3a. `src/freelist.rs` — `TxFreelist::allocate` 未对齐分配
- **症状**：首次写页分配即 `misaligned pointer dereference` panic。
- **根因**：`Bump::alloc_layout(Layout::array::<u8>(bytes))`（`align=1`）分配后，cast 成
  `align=8` 的 `*mut Page` → 地址不 8 对齐。
- **修**：改用 `Layout::from_size_align(bytes, core::mem::align_of::<Page>())`。

### 3b. `src/bucket.rs` — `From<&[u8]> for BucketMeta` 未对齐反序列化
- **症状**：DBFS2 建多个 inode bucket 时，bucket meta 落到叶内非对齐偏移，读回 `BucketMeta`
  panic（`bucket.rs` 里 `*(ptr as *const BucketMeta)`）。
- **根因**：`BucketMeta` 是 `repr(C)` 两个 `u64`（align 8），但存的是打包 `&[u8]`，起点未必 8 对齐。
- **修**：用 `copy_nonoverlapping` + `MaybeUninit<BucketMeta>::assume_init` 做未对齐读
  （`read_unaligned` 等价；`BucketMeta: Copy`）。
- **验证**：`memfile` base 实测 4096 对齐，故 page.rs/db.rs/tx.rs 里基于整页起点的 cast 无需改。

---

## 4. `vendor/dbop-a4d2b58`（os-module/dbop）

- **why**：dbfs2 无条件 git 依赖 `dbop`（`git+https://github.com/os-module/dbop.git#a4d2b58`）。
- **what**：无 bug、无改动，仅为**离线 / 可复现** vendored（避免 VM 首次 build 联网抓取，且钉住 commit）。
- **注意**：它自身也依赖 `Godones/jammdb`（git），会随根 `[patch]` 一并重定向到 3。

---

## 复现/更新建议

- 若上游修了 bug，可升级对应 vendor 子目录并去掉对应 `[patch]` 条目；但升级**前**请重跑
  `cargo +nightly-2025-05-20 run -p a-harness` 确证写负载不再 panic。
- 保持 vendor 内**无 `.git`、无 `.cargo-ok`/`.cargo_vcs_info.json`** 等 cargo 内部标记，避免污染提交。

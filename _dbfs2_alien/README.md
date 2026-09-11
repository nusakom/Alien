# DBFS2-alien

把 **DBFS2 数据库文件系统**（`dbfs2`）以**薄适配层**方式接进 **Alien OS**（`os-module/rvfs` 新版 VFS 框架）的完整可移植工程。

> 本仓库是「**数据层自足**」形态：克隆后无需 Alien 即可独立编译并跑通 DBFS2 的真实
> create / write / read / readdir / unlink 数据通路。把 DBFS2 挂进完整 Alien 内核并在
> QEMU 里真 boot，按 [`docs/ALIEN_INTEGRATION.md`](docs/ALIEN_INTEGRATION.md) 在 Alien fork 里做几处手改即可。

---

## 1. 这是什么（背景）

- **DBFS2**（`dbfs2/` v0.2.0）：一个**数据库文件系统**，用 JammDB 做底层 KV 存储，把文件/目录存成键值。老 API 是函数指针 `InodeOps/FileOps/FileSystemType`（耦合 `Godones/rvfs` 旧版）。
- **Alien OS**：一个 Rust 写的教学用 OS，其 VFS（`subsystems/vfs`）依赖 **`os-module/rvfs`** 新版 trait 式 API：`VfsInode / VfsFile / VfsFsType / VfsDentry / VfsSuperBlock`。
- **问题**：两套 rvfs API 完全不同代、互不兼容，DBFS2 老 API 没法直接被 Alien 用。
- **解法（本仓库）**：DBFS2 的 `dbfs_common_*` 后端函数全部 `pub` 且只接收 `usize` inode 号 + `&[u8]`，**完全不依赖旧 rvfs 类型**。于是建一层**薄适配器**（`adapter/`），把新版 `Vfs*` trait 直接薄转接到 `dbfs_common_*`，**完全不碰 JammDB 与 DBFS2 存储逻辑**。

### 分层验证表

| 验证层 | 内容 | 结果 | 怎么跑 |
|--------|------|------|--------|
| **数据层** | DBFS2 `dbfs_common_*` 真实读写 | ✅ | `cargo run -p a-harness` |
| **adapter → vfscore** | 薄适配层编译通过 | ✅ | `cargo check -p dbfs2-adapter` |
| **Alien → mount → dbfs2** | 集成进 Alien 内核并 mount | ✅ 编译级 | 见集成手册 |
| **完整 QEMU boot** | Alien 启动挂 dbfs | ⏳ 需配 QEMU 环境 | 见集成手册 |

---

## 2. 目录结构

```
dbfs2-alien/
├── Cargo.toml            # workspace 根；统一持有全部供应链 [patch]（相对路径）
├── rust-toolchain.toml   # 固定 nightly-2025-05-20（与 Alien 内核对齐）
├── dbfs2/                # 改造后的 DBFS2 后端（数据层）
│   └── src/
├── adapter/              # dbfs2-adapter：vfscore(Vfs*) → dbfs_common_* 薄适配层
│   ├── src/  (fstype/convert/dentry/inode/superblock/error/lib)
│   └── 适配分析.md        # 全量 API 对照表 / 适配表 / 构建级阻塞 / 改动清单
├── harness/              # a-harness：宿主数据通路测试（无需内核）
│   └── src/main.rs
├── vendor/               # 供应链补丁 / 副本（全部经根 [patch] 生效）
│   ├── core2-0.4.0/      # crates.io 已全量 yank，jammdb/vfscore 硬依赖
│   ├── rvfs-patched/     # Godones/rvfs：riscv64 c_char 移植 bug 补丁
│   ├── jammdb-patched/   # Godones/jammdb：memfile 后端两个对齐 bug 补丁
│   └── dbop-a4d2b58/     # os-module/dbop（离线可复现 vendored）
└── docs/
    ├── ALIEN_INTEGRATION.md   # 把 dbfs2 接进 Alien fork 的手改手册（含 QEMU）
    └── SUPPLY_CHAIN_PATCHES.md# 每个补丁的 why/what/哪里
```

---

## 3. 快速开始（数据层，无需 Alien / QEMU）

> **首次构建联网说明**：三个有 bug / yank 的供应链依赖（core2/rvfs/jammdb/dbop）已 vendored 到
> `vendor/`，不需联网。但 `adapter` 依赖的 `vfscore`（`os-module/rvfs`）及其依赖 `pconst`/`pod`
> 是正常 git 依赖，**首次 build 需要联网抓取一次**（之后走 cargo 缓存）。若需完全离线，见
> [`docs/SUPPLY_CHAIN_PATCHES.md`](docs/SUPPLY_CHAIN_PATCHES.md) 的离线建议。

前置：安装 `nightly-2025-05-20`（`rust-toolchain.toml` 会自动切换）：

```bash
rustup toolchain install nightly-2025-05-20 --profile minimal --component rust-src,rustfmt,clippy,llvm-tools
```

编译 + 运行宿主数据通路测试（走 `init_dbfs_mem` + 与 adapter 完全一致的 `dbfs_common_*`）：

```bash
cargo +nightly-2025-05-20 check --workspace      # 编译全仓（dbfs2 + adapter + harness）
cargo +nightly-2025-05-20 run -p a-harness       # 真实数据读写 ALL PASS
```

期望输出（节选）：
```
[OK] init_dbfs_mem
[OK] root ino = 1
[OK] mkdir subdir -> ino 2
[OK] create file1 -> ino 3
[OK] write file1 20 bytes
[OK] read file1 -> [104, 101, ...]
[OK] readdir subdir count=3   ( .  ..  file1 )
[OK] unlink file1
[OK] lookup file1 after unlink -> NotFound
===== ALL PASS: DBFS2 data path OK (mem backend) =====
```

> `cargo` 会自动按根 `rust-toolchain.toml` 用 nightly-2025-05-20；也可显式 `cargo +nightly-2025-05-20`。

---

## 4. 三个 crate 速览

| crate | 角色 | 关键点 |
|-------|------|--------|
| `dbfs2` | 数据库文件系统后端 | `init_dbfs_mem` 用 JammDB **内存后端**，无块设备；`dbfs_common_*` 为薄转发目标 |
| `dbfs2-adapter` | 把 `dbfs2` 接成 Alien 的 `VfsFsType` | 把 `VfsInode/VfsFile/VfsDentry/VfsFsType` 直接转发到 `dbfs_common_*` |
| `a-harness` | 宿主数据通路测试 | 独立验证数据层真实读写，不引入 vfscore/内核 |

---

## 5. 供应链补丁为什么必须存在（不能删 vendor/）

`[patch]` 只能写在**正在构建的 workspace 的根 manifest**。所以本仓库把三个子工程收进一个
workspace，由根 `Cargo.toml` 统一把 dbfs2 的 git 依赖重定向到 `vendor/`：

1. **core2**：crates.io 已**全量 yank**（含 0.4.0），而 `jammdb` / `vfscore(initrd)` 硬依赖 `core2="0.4"`。
2. **Godones/rvfs**（dbfs2 的 `rvfs_old`）：riscv64（`c_char=u8`）下把 `name` cast 成 `*const i8` → E0308。
3. **Godones/jammdb**：memfile 后端**两个对齐 bug**（只有真实写负载才暴露）：
   - `freelist.rs` 用 `align=1` 分配再 cast 成 `align=8` 的 `*mut Page` → 首次写页 misaligned；
   - `bucket.rs` 打包叶值反序列化 `BucketMeta` 未对齐读。
4. **dbop**：无 bug，仅为离线可复现而 vendored。

详见 [`docs/SUPPLY_CHAIN_PATCHES.md`](docs/SUPPLY_CHAIN_PATCHES.md)。

---

## 6. 为什么能“换机器 / 上传 GitHub / 放 VM”直接用

- **全部路径相对**：根 `[patch]`、`adapter` 与 `harness` 对 `dbfs2` 的 `path` 依赖都是仓库内相对路径，不写死本机绝对路径。
- **单 workspace + 单根 [patch]**：供应链一次性在此解决，子工程各自不重复。
- **工具链固定**：`rust-toolchain.toml` 钉住 nightly-2025-05-20，避免不同 nightly 注入幻影编译错。
- **已实测**：在交付副本里 `check --workspace` 0 error、`run -p a-harness` ALL PASS，且 jammdb/dbop 都解析到 `vendor/`，证明补丁链路在相对路径下真实生效。

---

## 7. License / 上游

- 上游：DBFS2 / Alien 均保留各自 License（见各自目录）。
- 本仓库内的供应链 vendored 副本来自各上游开源项目，仅做补丁承载。

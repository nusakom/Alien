# 把 DBFS2 接进 Alien OS（fork）—— 手改手册

> 目标：在**你自己 fork 的 Godones/Alien** 里，让内核能 `mount("dbfs", ...)` 并用 DBFS2。
> 本仓库（dbfs2-alien）已把 DBFS2 / adapter / 供应链补丁整理成可上传形态；Alien 是**独立仓库**，
> 你需要在自己那份 Alien 上按下面步骤手改 **4 处**，并让它指向本仓库（或把本仓库拷进 Alien 工作区）。
>
> 分层：下面步骤做完并 `cargo check` 通过 = **第 3 层（Alien→mount→dbfs2）编译级坐实**；
> 要走第 4 层（QEMU 真 boot 看到 selftest 输出）另见 §「QEMU 运行」。

---

## 0. 前提核对（Alien 侧环境，先跑一次确认）

```bash
cd <你的 Alien 根>
rustup toolchain install nightly-2025-05-20 --profile minimal \
  --component rust-src,llvm-tools,rustfmt,clippy
rustup target add riscv64gc-unknown-none-elf --toolchain nightly-2025-05-20
# 权威 check（等价 Makefile check target）：
cargo +nightly-2025-05-20 check -p kernel \
  --target riscv64gc-unknown-none-elf --features qemu,talloc,fat
```

干净基线应 0 error。若过不了，先修环境（多为缺 target / 工具链版本错）。

---

## 1. 把本仓库放进 Alien 的构建范围（二选一）

本仓库的 `dbfs2/`、`adapter/`、`vendor/` 需要被 Alien 的 cargo 图看到。

- **A. 直接把本仓库拷/链接进 Alien 工作区**（最简单，推荐）：
  ```
  cp -r dbfs2-alien <你的 Alien 根>/_dbfs2_alien
  ```
  于是路径为 `<Alien>/_dbfs2_alien/{dbfs2,adapter,vendor}`。以下都以 `_dbfs2_alien` 为例。

- **B. 保持独立并用绝对/相对 path**：不推荐（换机器会挂），除非你把两仓库放固定相邻目录。

---

## 2. 四处手改

### 2.1 根 Cargo.toml —— 加供应链 [patch]（指向 _dbfs2_alien/vendor）

在 Alien 根 `Cargo.toml` 的 `[workspace]` 段后新增：

```toml
# DBFS2 供应链补丁：把 dbfs2 的 git 依赖重定向到本工作区 _dbfs2_alien/vendor
[patch.crates-io]
core2 = { path = "_dbfs2_alien/vendor/core2-0.4.0" }

[patch."https://github.com/os-module/dbop.git"]
dbop = { path = "_dbfs2_alien/vendor/dbop-a4d2b58" }

[patch."https://github.com/Godones/rvfs.git"]
rvfs = { path = "_dbfs2_alien/vendor/rvfs-patched" }

[patch."https://github.com/Godones/jammdb"]
jammdb = { path = "_dbfs2_alien/vendor/jammdb-patched" }
```

> 这些路径相对 Alien 根。`[patch]` 只作用于构建目标 workspace 的根 manifest，所以必须放 Alien 根。

### 2.2 subsystems/vfs/Cargo.toml —— 加 adapter 依赖

```toml
[dependencies]
dbfs2-adapter = { path = "../../_dbfs2_alien/adapter" }
```

（若你只是要 mount dbfs 而**不做内核内 selftest**，可跳过 2.3/2.4 的 selftest 部分，只需 2.2 + 2.3 的 register。）

### 2.3 subsystems/vfs/src/lib.rs —— 注册 "dbfs" 文件系统类型

在 `register_all_fs()` 里，把 `dbfs` 像其它 fs 一样插进 `FS` map（放在 tmpfs/pipefs insert 之后即可）：

```rust
use dbfs2_adapter::DbfsFs;   // 顶部或函数内
// ...
let dbfs = Arc::new(DbfsFs::new());
FS.lock().insert("dbfs".to_string(), dbfs);
```

> mount 由 adapter 的 `DbfsFs::mount` 内部调用 `init_dbfs_mem` 构造 JammDB **内存后端**，
> 无需块设备；`dev=None` 即可。

### 2.4 kernel/src/fs/basic.rs —— sys_mount 认 "dbfs"

把 "dbfs" 加进 `sys_mount` 的文件系统名 match 臂：

```rust
name @ ("tmpfs" | "ramfs" | "fat32" | "dbfs") => {
    // ... 走 i_mount(0, &dir, dev, &[])
}
```

---

## 3. （可选）内核内最小运行验证 selftest

不加完整 syscall 设计，只在 Alien vfs 里放一个 **feature 门控、可整删** 的测试入口，
boot 时自动 `mount("dbfs")` + create/write/read/readdir/unlink 并 println。

### 3.1 vfs feature + module

`subsystems/vfs/Cargo.toml` `[features]`：
```toml
dbfs_selftest = []
```

新建 `subsystems/vfs/src/dbfs_selftest.rs`：**直接拷本仓库 [`docs/dbfs_selftest.reference.rs`](dbfs_selftest.reference.rs)**，
即一份已编译通过的最小实现。逻辑：从 `crate::FS` 取 `"dbfs"` → `fs_type.i_mount(0, "/", None, &[])`
→ `root.inode()` → `create("hello.txt")` → `write_at`/`read_at` 校验 → `readdir` → `unlink`
→ `lookup` 确认 NotFound。

`subsystems/vfs/src/lib.rs`：
```rust
#[cfg(feature = "dbfs_selftest")]
pub mod dbfs_selftest;
```
并在 `init_filesystem()` 末尾、`SYSTEM_ROOT_FS.call_once(...)` 之前调用：
```rust
#[cfg(feature = "dbfs_selftest")]
if let Err(e) = dbfs_selftest::dbfs_selftest() {
    println!("[dbfs-selftest] FAILED: {:?}", e);
}
```

### 3.2 kernel feature 穿透

`kernel/Cargo.toml` `[features]`：
```toml
dbfs_selftest = ["vfs/dbfs_selftest"]
```

### 3.3 编译验证（必须走 kernel 图，feature 开启）

```bash
cargo +nightly-2025-05-20 check -p kernel \
  --target riscv64gc-unknown-none-elf --features qemu,talloc,fat,dbfs_selftest
```

> 不要用 `-p vfs --features dbfs_selftest,fat,initrd` 单独查 —— 会因缺 `qemu` platform
> feature 报 `basic_machine_info`/`platform_dtb_ptr`/`console_putchar` 等已知平台错误。

---

## 4. QEMU 真 boot（第 4 层，本机/VM 需配环境）

Alien 的 `make run` 需要：`qemu-system-riscv64`、交叉链接器（`riscv64-linux-gnu-ld` 或
`riscv64-unknown-elf-ld`，供 `make compile` 链接 libkernel.a→ELF）、`gen_ksym`
（`cargo install --git https://github.com/Starry-OS/ksym`）、`mkfs.fat`、`tools/sdcard.img`、
`boot/rustsbi-qemu.bin`。

在已配好这些的 VM/机器上：
```bash
cd <Alien 根>
# 让 selftest feature 在 make 里也开启（把 dbfs_selftest 并进 FEATURES，或用临时 make 变量）
make FEATURES="qemu talloc fat dbfs_selftest" run   # 视 Makefile 特征变量而定，自行对齐
```

boot 后 serial 上应看到 `[dbfs-selftest] ... PASS` 系列输出，即 DBFS2 在 Alien 内核里真实跑通。

> 提示：若只是想在轻量环境验证「数据层真读写」，不必 boot 内核 —— 直接在本仓库
> `cargo +nightly-2025-05-20 run -p a-harness`（宿主程序）即可，它走的就是 adapter mount
> 会薄转发的同一批 `dbfs_common_*`。

---

## 5. 常见坑

1. **rvfs 包名冲突**：dbfs2 的旧 rvfs 已在 dbfs2 内改名 `rvfs_old`（`package="rvfs"`），不要改回。
2. **`-p vfs` 单独查会报平台错误**：永远走 `-p kernel --features qemu,...`。
3. **`[patch]` 只认根 manifest**：供应链 patch 必须放 Alien 根 Cargo.toml，放 vfs/adapter 里不生效。
4. **memfile 对齐 bug 后端无关**：QEMU 与 host 用同一 memfile 后端，本仓库 vendor 里的 jammdb
   已含修复，别回退成上游裸 jammdb（会 misaligned panic）。

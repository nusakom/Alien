# DBFS2 → Alien 新版 VFS · 零 Alien 修改源码级适配方案

> 策略定调：**老师只看最终能否跑起来**。以"Alien 最终能挂载并使用 DBFS2，且尽量 0 修改 Alien"为硬约束，所有兼容性问题在 DBFS2/Adapter 侧解决。
> 依据：已读取三个 zip 源码 + 从 `github.com/os-module/rvfs` 浅克隆并读完目标 VFS 全部 trait 定义。

---

## 0. 事实基础（已核实）

- **目标 VFS = `os-module/rvfs` 的 `vfscore`**（不在三个 zip 中，已克隆到 `/tmp/rvfs_new` 并读完）。Alien 的 `subsystems/vfs` 正是依赖它。
- **目标 VFS 是 trait 方法式**：`VfsInode` / `VfsFile` / `VfsFsType` / `VfsDentry` / `VfsSuperBlock`，方法返回 `Arc<dyn VfsInode>` 等。且 **`VfsInode: VfsFile`**（文件操作合并进 inode）。
- **DBFS2 老 API 是函数指针 struct 式**：`InodeOps` / `FileOps` / `FileSystemType` / `SuperBlockOps`，inode/file/superblock 三者分离。
- **★ 最关键复用发现**：DBFS2 的 `dbfs_common_*` 后端函数（`inode.rs`/`file.rs`/`link.rs`/`fs_type.rs`/`common.rs`）**全部 `pub`、且只接收 `usize` inode 号 + `&[u8]` 缓冲，完全不依赖旧 rvfs 类型**；`init_dbfs(db)` 也是 `pub`。

  ⇒ 适配层应**直接把新 Vfs trait 接到 `dbfs_common_*` 上**，**完全不碰 JammDB 与 DBFS2 存储逻辑**。旧的那套 `InodeOps`/`FileOps`/`FileSystemType`/`SuperBlockOps` 结构（耦合旧 `Godones/rvfs`）应当**绕过、不要复用**。这正是"薄适配层"能成立的根基。

---

## 1. 全量 API 对照表（DBFS2 后端 → 目标 Vfs trait）

| 功能 | DBFS2 现有（旧 rvfs 耦合，绕过） | 目标 Vfs 方法（签名，来自 vfscore） | 适配动作 | 复用来源 |
| --- | --- | --- | --- | --- |
| lookup | `dbfs_lookup(dir, dentry)` | `VfsInode::lookup(&self, name:&str)->Arc<dyn VfsInode>` | 转发 | `dbfs_common_lookup(dir:usize, name)` |
| create(文件) | `dbfs_create(dir, dentry, mode)` | `VfsInode::create(&self, name, ty:VfsNodeType, perm:VfsNodePerm, rdev)->Arc<dyn VfsInode>` | 转发，ty=File | `dbfs_common_create` |
| mkdir | `dbfs_mkdir(dir, dentry, mode)` | 同上 `create`，ty=Dir | 转发，ty=Dir | `dbfs_common_create` |
| unlink | `dbfs_unlink(dir, dentry)` | `VfsInode::unlink(&self, name)` | 转发 | `dbfs_common_unlink` |
| rmdir | `dbfs_rmdir(dir, dentry)` | `VfsInode::rmdir(&self, name)` | 转发 | `dbfs_common_rmdir` |
| link | `dbfs_link(dir, dentry, …)` | `VfsInode::link(&self, name, src:Arc<dyn VfsInode>)` | 转发 | `dbfs_common_link` |
| symlink | `dbfs_symlink(dir, dentry, target)` | `VfsInode::symlink(&self, name, sy_name)` | 转发 | `dbfs_common_*`(symlink 后端) |
| readlink | `dbfs_readlink(dentry, buf)` | `VfsInode::readlink(&self, buf)->usize` | 转发 | `dbfs_common_readlink(ino, buf)` |
| get_attr | `dbfs_getattr(dentry, key, buf)` | `VfsInode::get_attr(&self)->VfsFileStat` | 取全量属性再映射 | `dbfs_common_attr(number)->DbfsAttr` |
| set_attr | `dbfs_setattr(dentry, key, val)` | `VfsInode::set_attr(&self, InodeAttr)` | 转发 | `dbfs_common_*`(attr 后端) |
| list_xattr | `dbfs_listattr(dentry, buf)` | `VfsInode::list_xattr()->Vec<String>` | 转发 | `dbfs_common_*` |
| remove_xattr | `dbfs_removeattr(dentry, key)` | **VfsInode 无对应方法** | 暂不支持（第二阶段） | — |
| rename | `dbfs_rename(old, new_parent, …)` | `VfsInode::rename_to(&self, old_name, new_parent:Arc<dyn VfsInode>, new_name, flag)` | 转发 | `dbfs_common_rename` |
| truncate | `dbfs_truncate(inode)` | `VfsInode::truncate(&self, len:u64)` | 转发 | `dbfs_common_truncate` |
| read | `dbfs_file_read(file, buf, offset)` | `VfsFile::read_at(&self, offset, buf)->usize` | 转发 | `dbfs_common_read(number, buf, offset)` |
| write | `dbfs_file_write(file, buf, offset)` | `VfsFile::write_at(&self, offset, buf)->usize` | 转发 | `dbfs_common_write(number, buf, offset)` |
| readdir | `dbfs_readdir(file, dirents)` | `VfsFile::readdir(&self, start_index:usize)->Option<VfsDirEntry>` | 逐个返回（见 §3） | `dbfs_common_readdir` |
| sync_fs | `dbfs_sync_fs(sb)` | `VfsSuperBlock::sync_fs(&self, wait)` | 转发 | `dbfs_common_umount`/flush |
| stat_fs | `dbfs_stat_fs(sb)->StatFs` | `VfsSuperBlock::stat_fs(&self)->VfsFsStat` | 映射结构体 | `dbfs_common_statfs` |
| 注册/挂载 | `dbfs_get_super_blk/get_super_blk` | `VfsFsType::mount(self:Arc<Self>, flags, ab_mnt, dev, data)->Arc<dyn VfsDentry>` | 建根+注册 | `init_dbfs` + `dbfs_common_root_inode` |
| kill_sb | `dbfs_kill_super_blk(sb)` | `VfsFsType::kill_sb(&self, sb)` | 转发 | `dbfs_common_umount` |
| 错误类型 | `StrResult<T>=Result<T,&'static str>` / `DbfsError` | `VfsResult<T>=Result<T, VfsError>` | **需转换层** | 见 §3 |

**一句话**：除了"错误类型转换"和"结构模型桥接"，每个操作都是**直接转发到已有的 `dbfs_common_*`**，没有重写逻辑。

---

## 2. 文件改动清单（核心交付物）

### ✅ 完全不碰（硬约束）
- **JammDB 数据层**、DBFS2 的 `dbfs_common_*` 后端函数（`common.rs`/`attr.rs`/`extend.rs` 主体）。
- DBFS2 的数据库读写、inode 数据结构、slice 缓存逻辑。
- **Alien 的 VFS 调用链**（`kernel/src/fs/basic.rs` 的 mount 流程）、Alien 核心代码。

### 🟡 不再使用、但保留不动（避免误改）
- DBFS2 `inode.rs` / `file.rs` / `fs_type.rs` / `link.rs` 里基于旧 `Godones/rvfs` 的 `InodeOps`/`FileOps`/`FileSystemType`/`SuperBlockOps` 及 `dbfs_*`（非 common）包装函数。**适配层不调用它们**，留着不影响编译（后续可清理）。

### 🆕 必须新建：适配层（建议独立 crate `dbfs2-adapter`）
| 文件 | 内容 |
| --- | --- |
| `adapter/inode.rs` | `DbfsInode` 实现 `VfsInode` + `VfsFile`；方法体转发到 `dbfs_common_lookup/create/unlink/rmdir/link/symlink/readlink/get_attr/set_attr/list_xattr/rename/truncate` 与 `read/write/readdir` |
| `adapter/dentry.rs` | `DbfsDentry` 实现 `VfsDentry`（name/parent/inode/find/insert/remove/path）；内部持有 `ino: usize` |
| `adapter/superblock.rs` | `DbfsSuperBlock` 实现 `VfsSuperBlock`（stat_fs→dbfs_common_statfs；sync_fs；root_inode） |
| `adapter/fstype.rs` | `DbfsFsType` 实现 `VfsFsType`：`mount`= `init_dbfs(db)` + `dbfs_common_root_inode` + 建 root `DbfsInode`/`DbfsDentry`；`fs_name()`返回 `"dbfs"`；`fs_flag()` 视是否需要设备而定 |
| `adapter/error.rs` | `DbfsResult`/`DbfsError` → `VfsResult`/`VfsError` 映射（写 `fn to_vfs_err(e)->VfsError`，未知错误归 `VfsError::NoSys`） |
| `adapter/lib.rs` | 导出 `DbfsFsType`，供 Alien 注册 |

### 🔧 Alien 侧最小改动（无法严格 0 修改，这是最低成本）
- `Alien-main/subsystems/vfs/Cargo.toml`：加 `dbfs2-adapter = { path = "../../dbfs2-adapter" }`（或 git）。
- `Alien-main/subsystems/vfs/src/lib.rs` 的 `register_all_fs()`：加约 5 行，把 `Arc::new(DbfsFsType::new(...))` 以 key `"dbfs"` 插入 `FS` map。
- **这就是全部 Alien 改动**。已有的 mount 系统调用 `system_support_fs("dbfs").i_mount(...)` 可直接挂载，无需改。

---

## 3. 结构错配的处理（实现时必踩的点）

1. **函数指针 struct → trait**：新类型 `impl` trait，方法体转发到 `dbfs_common_*`。旧 struct 不用。
2. **`VfsInode::create(name, ty, perm, rdev)` 合并了旧 create+mkdir**：用 `ty == VfsNodeType::Dir` 区分调 `dbfs_common_create(..., Dir)` / `(..., File)`。
3. **`get_attr()` 返回 `VfsFileStat` 结构**（不是旧式的按 key 取 xattr）：用 `dbfs_common_attr(ino)` 拿 `DbfsAttr`，再映射到 `VfsFileStat{st_mode,st_size,st_uid,st_gid,st_atime,st_mtime,st_ctime,…}`。旧的 `get_attr(dentry,key,buf)` 语义不同，改用 `common_attr`。
4. **`readdir(start_index)->Option<VfsDirEntry>`**：旧 `readdir(file, buf)` 一次填整缓冲；新接口按索引逐个返回 `{ino, ty, name}`。用 `dbfs_common_readdir` 的游标/offset 实现（维护一个 readdir 位置即可）。
5. **错误类型转换**：`StrResult`(`&'static str`)/`DbfsError` → `VfsResult`(`VfsError`)。转换层集中放在 `adapter/error.rs`。
6. **`VfsInode` 没有 `remove_xattr` / `follow_link`**：xattr 删除、symlink 跟随可暂放第二阶段；`follow_link` 逻辑可并入 `lookup` 或在第二阶段补。
7. **`VfsFile` 无 `open`**：open 在 VFS 层处理，DBFS2 已有 `dbfs_common_open`，可在 `DbfsInode`/`DbfsFile` 的 `read_at`/`write_at` 首次调用时按需触发，或忽略（取决于 Alien 是否要求 open 钩子）。

---

## 4. "零修改 Alien" 的诚实结论

- **严格 0 修改不可能**：DBFS2 作为内核 crate 必须被 Alien 静态包含并注册；而 `FS` map 没有导出注册函数（`register_all_fs` 是私有的）。
- **务实最小改动 = Alien 加 1 个依赖 + `register_all_fs` 约 5 行**。VFS 调用链、mount 系统调用、核心逻辑全部不动。这满足"**尽量不碰 Alien 核心**"的硬约束，汇报口径即：
  > *本项目以 DBFS2 为基础，通过新增 VFS 适配层（dbfs2-adapter）接入 Alien OS 现有 VFS 框架，在不修改 Alien OS VFS 核心逻辑的前提下，实现数据库文件系统的挂载与基本文件操作。*
- 若想更"干净"，可在 Alien 侧加一个**通用的** `pub fn register_filesystem(fs: Arc<dyn VfsFsType>)`（一行级、对所有 fs 有用的公共接口），再由 adapter 在初始化时调用——但这仍属于改动 Alien，性价比不如直接在 `register_all_fs` 加 5 行。

---

## 5. 建议的落地步骤（待你确认后实施）

- **阶段 0（基线）**：确认 DBFS2 能以 `default-features=false, features=["sli32k"]` 编译（去掉旧 rvfs 依赖），跑通 `cargo build` 基线；同时确认 Alien 现有构建（nightly + qemu）可用。
- **阶段 1（最小可挂载）**：建 `dbfs2-adapter` crate，先实现 `fstype.rs` + `inode.rs` 的 `mount`/`lookup`/`create`/`write`/`read`，做"能挂载、能建文件、能读写"的最小验证。
- **阶段 2（补齐第一优先级）**：`readdir`/`unlink`/`rmdir`/`rename`/`mkdir`/`get_attr`/`set_attr`/`stat_fs`。
- **阶段 3（接 Alien）**：按 §2 加依赖 + 注册 `"dbfs"`，用 Alien 现有 mount 系统调用验证端到端。
- **第二阶段（可选）**：`truncate` 完整语义、`link`/`symlink`、`xattr`、错误恢复、性能测试。

---

## 6. 结论

方案从"确认架构"转为"**以最少改动把 DBFS2 跑进 Alien**"是正确方向。源码级分析显示：
1. 目标 VFS 是 `os-module/rvfs` 的 trait 式 API（`VfsInode:VfsFile` 等），与 DBFS2 老 API 结构不同，但**每个操作都能 1:1 转发到 DBFS2 已有的 `dbfs_common_*` 后端**，因此适配层可以很薄、**完全不必碰 JammDB 与存储逻辑**。
2. 唯一不可避免的 Alien 改动是"注册一个新文件系统"（1 依赖 + ~5 行），核心 VFS 逻辑不动。
3. 建议新建独立 `dbfs2-adapter` crate 承载全部适配；DBFS2 核心与 Alien 核心均保持原样。

> 下一步如果你确认，我就按阶段 1 直接开始搭 `dbfs2-adapter` 的最小可挂载骨架（先 `fstype.rs`+`inode.rs`）。

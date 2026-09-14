# DBFS2 是怎么接进 Alien 的

**日期**：2026-09-15  
**范围**：`_dbfs2_alien/`（DBFS2 核心 + 适配层 + vendor 依赖）

这篇主要记录四件事：

1. 这个 DBFS2 内部是什么结构
2. 原来的接口对不上，我是怎么改到能用的
3. 怎么照着 rvfs 的做法把它并进 Alien 内核
4. 凭什么说它真的挂载成功了

写的时候尽量只写做过和验证过的。没验证完的、没查清的都放在最后一节，不在正文里当成果讲。

---

## 0. 先说结论

DBFS2 本身**一行没重写**，它那套存储逻辑（JammDB 上面的文件/目录 KV 模型）是原样保留的。  
我做的是在它和 Alien 的 VFS 中间**加了一层很薄的适配器**，把两套对不上的 rvfs API 接起来，  
再把它的存储后端换成 Alien 的块设备（`/dev/dbfs`），最后按 Alien 已有的方式挂到 `/dbfs`。

验证过的部分：QEMU 里 boot，内核打印 `[dbfs] step5: mounted at /dbfs`，之后 `/dbfs` 能读写、能建目录。

---

## 1. 起点：两个 rvfs，名字一样但不是一代东西

这是整件事最麻烦的地方，先讲清楚。

|         | DBFS2 原本依赖的                                     | Alien 用的                                                                   |
| ------- | ----------------------------------------------- | -------------------------------------------------------------------------- |
| 仓库      | `Godones/rvfs`                                  | `os-module/rvfs`                                                           |
| crate 名 | `rvfs`                                          | `rvfs`（在 Alien 里对外叫 `vfscore`）                                             |
| API 风格  | 函数指针表 `InodeOps` / `FileOps` / `FileSystemType` | trait `VfsInode` / `VfsFile` / `VfsFsType` / `VfsDentry` / `VfsSuperBlock` |

两者同名不同代，互相不认。而且 crate 名都叫 `rvfs`，放进同一个构建图会直接冲突。

所以 DBFS2 原本那套按老 API 写的文件系统实现，Alien 拿过去是没法直接用的。

---

## 2. 为什么可以只换壳、不重写

翻 DBFS2 后端函数的时候发现一件事，这是后面所有工作的前提：

```rust
// _dbfs2_alien/dbfs2/src/inode.rs —— 后端函数签名
pub fn dbfs_common_lookup(dir: usize, name: &str) -> DbfsResult<DbfsAttr>
pub fn dbfs_common_read(ino: usize, buf: &mut [u8], offset: usize) -> DbfsResult<usize>
pub fn dbfs_common_write(ino: usize, buf: &[u8], offset: usize) -> DbfsResult<usize>
pub fn dbfs_common_create(...) -> DbfsResult<DbfsAttr>
pub fn dbfs_common_rename(r_uid: u32, r_gid: u32, old_dir: usize, old_name: &str, ...) -> DbfsResult<()>
```

它们只吃 `usize` 的 inode 号、`&[u8]` 缓冲和几个标量参数，跟 rvfs 的类型一点关系都没有。

这意味着在 DBFS2 里 inode 就是个整数，不存在什么 `Inode` 结构体要继承。那适配器只要把新版 trait  
的调用翻译成 `dbfs_common_*` 就行，JammDB 和 DBFS2 的存储逻辑一个字都不用动。

所以接口不兼容的是外面那层壳，不是里面的存储逻辑，换壳就够了。

---

## 3. 内部结构和块设备后端

### 3.1 分层大概长这样

```text
  用户程序 / pjdfstest
        │  open / read / write / mkdir / rename ...
        ▼
  Alien VFS（vfscore，trait 式）
        │  VfsInode / VfsFile / VfsDentry
        ▼
  adapter（dbfs2-adapter）          ← 这次新写的，只做翻译
        │  dbfs_common_*(ino, buf, ...)
        ▼
  DBFS2 语义层（dbfs2/src/{inode,file,link,attr,fs_type}.rs）
        │  文件和目录 = JammDB 里的 bucket + KV 记录
        ▼
  JammDB（事务型 KV，vendor/jammdb-patched）
        │  tx 事务：读页 / 改页 / commit
        ▼
  块设备后端（adapter/src/blockdev.rs，BlockDevFile）
        │  VfsInode::read_at / write_at
        ▼
  Alien 块设备层（GenericBlockDevice + 页缓存）
        │  read_block / write_block（512 B 扇区）
        ▼
  /dev/dbfs  =  RAMDISK（默认）或第二块 virtio-blk（开 dbfs_persist）
```

关键点在于 DBFS2 既不直接碰内存也不直接碰磁盘。它以为自己在读写一个"文件"，也就是 JammDB 的数据库文件，  
而这个文件是我们用块设备假装出来的，物理上落在 Alien 的块设备层。跟 fat32 坐在 `/dev/sda` 上是同一个路子。

### 3.2 DBFS2 自己那一层在干什么

- inode 号就是 bucket id。DBFS2 里没有 inode 表，一个文件或者一个目录就是 JammDB 里的一个 bucket，  
  目录项就是 bucket 里的 KV（名字 → 子 bucket id）。这也是上一节说"后端函数只吃 `usize`"的原因。
- 所有改动都走 JammDB 事务。写文件等于开一个 tx、改页、commit。JammDB 是 COW 的，提交时把动过的页写回去，  
  meta 页是双份交替写，所以掉电能回到上一个完整的 tx。
- DBFS2 的源文件一个字没改（`dbfs2/src/` 下面 9 个文件）。只动了 `Cargo.toml`（依赖改名）  
  和 `lib.rs`（把私有模块里的后端函数重导出，见 4.1）。

### 3.3 块设备后端：内存镜像加 write-through

JammDB 需要一个"DB 文件"，在 Alien 里我拿块设备来当这个文件。实现在 `_dbfs2_alien/adapter/src/blockdev.rs`：

```rust
pub struct BlockDevFile {
    pub data: Vec<u8>,              // 设备内容的内存镜像，长度恒等于 capacity
    pub dev: Arc<dyn VfsInode>,     // 底层块设备 inode（/dev/dbfs）
    pub capacity: usize,            // 设备容量，同时也是内存映射的合法范围
    pub logical_size: usize,        // DB 的逻辑大小，也就是有效数据长度
}
```

有三个地方是刻意这么设计的。

第一，`data` 是一整块预分配的内存镜像，长度恒等于 `capacity`，而且不能 realloc。因为 JammDB 通过  
`FileExt::addr()` 把裸指针拿走之后是原地访问的（`BlockDevMap::do_map` → `IndexByPageID`），指针必须稳定。

第二，两个"大小"得分开。`capacity` 给 JammDB 当映射上界（`FileExt::size()` 返回它），`logical_size`  
才是 DB 的真实长度（`metadata().len()` 返回它）。这两个如果混成一个，JammDB 的扩容判断会失效，  
mount 的时候也不得不一整盘读进来。所以 `open()` 的时候先读设备头部两页 meta，算出 `num_pages * 4096`  
得到有效长度，**只读这一段**，镜像剩下的部分保持为零。

第三，写是 write-through。`BlockDevFile::write` 先改内存镜像，紧接着调 `dev.write_at(..)` 落到块设备，  
返回就表示已经落盘了。崩溃一致性是建立在这个前提上的 —— 重启之后能读回上次写的东西。

### 3.4 为什么非要坐在块设备上

- 这是接进来的硬要求。文件系统得坐在块结构之上，不能自己 malloc 一块内存当磁盘使。
- 只有落在块设备上才有持久化可言。RAMDISK 模式下重启就丢（用来跑功能测试），开 `dbfs_persist`  
  之后换成第二块 virtio-blk，掉电重启数据还在。
- 跟 fat32/diskfs 同构，意味着块缓存、LRU、脏页淘汰这些现成的机制全都能复用，不用自己再写一套。

`/dev/dbfs` 这个设备节点是在 `subsystems/devices/src/block.rs` 里造的：默认在堆上划一块 16 MiB  
的零化缓冲区，包成 `MemoryFat32Img`；开了 `dbfs_persist` 就改由第二块 virtio-blk 填进去  
（用 `VIRTIO_BLK_INDEX` 计数来区分第一块盘和第二块盘）。

### 3.5 为什么是同步的，没上 future 也没搞绿色线程

存储这块我最后是按原作者的同步设计来做的，没有引入 future，也没上协程。理由有六条，前两条是决定性的。

**第一条，JammDB 的接口本来就是全同步的，改 async 等于重写 jammdb。**

```rust
// vendor/jammdb-patched/src/fs/mod.rs:74
pub trait DbFile: Seek + Write + Read + FileExt + Any {}
```

`core2::io` 的 `Read` / `Write` / `Seek` 全是同步签名，`FileExt` 也是（`metadata()`、`allocate()`、  
`sync_all()`、`addr()`）。要把这些改成 async，`db.rs` / `tx.rs` / `bucket.rs` / `page.rs` 整条链都得跟着动，  
`&self` 要变 `&mut self`，返回值要套 `Future`，还得处理 `Send`。而我给自己定的硬规矩是  
**不改 `vendor/jammdb-patched/**`** —— 一改就得长期维护一个 fork，上游升一次级就冲突一次。  
这个代价比多等一会儿大太多了。

**第二条，JammDB 读页走的是借用，不是 I/O，它压根没有"读一页"这个动作。**

JammDB 是 mmap 风格的，按 `page_id` 直接取页：

```rust
impl IndexByPageID for IndexByPageIDImpl {
    fn index(&self, page_id: u64, page_size: usize) -> IOResult<&[u8]> {
        // ... from_raw_parts 直接返回切片
    }
}
```

返回的是 `&[u8]` 借用，不是 owned 数据。同步返回是它的命根子：一旦换成 `async fn -> &[u8]`，  
await 期间这块内存可能已经被别的事务改写或者回收了，借用活不过 await 点，**类型上就写不出来**。  
硬要写只能改成返回 owned buffer，那等于给每一次页访问加一次 4 KiB 拷贝加一次堆分配，  
比同步等一次内存访问贵得多。

**第三条，Alien 的 VFS 全栈是同步的。** `VfsInode` / `VfsFile` / `VfsFsType` 这些 trait 里没有 `async fn`  
（内核里搜不到 `async fn`，只有块设备驱动里有个 `read_block_async`）。要让文件系统异步化只有两条路：  
要么把 VFS + syscall + 用户库整条链改成 async，等于重做 syscall 的返回路径，超出这次的范围；  
要么在中间加个 executor 配 `block_on` 把 async 压回同步，那等于什么都没得到，只是白白多一层调度开销。

**第四条，Alien 现有的异步能力 DBFS2 现在用不上，不是不想用，是路径对不上。**  
Alien 的块设备确实有异步路径，但它不是 Rust 的 `Future`，而是"提交 + 让出 CPU + IRQ 唤醒"这一套：

```text
read_blocks_nb() 提交 → 入 wait_queue → 让出 CPU
    → virtio IRQ → handle_irq() → wake_head() 唤醒 → complete_read_blocks() 取数据
```

它有前置条件，必须处在 task 上下文（`shim::take_current_task()` 得返回 `Some`，否则降级成同步读）。  
驱动里的注释也写得很明白：

> "仅供 QD=1 async 自检使用，**不参与** `read/write` 主路径（缓存层接入异步属于 Phase 8.3，  
> 不在本阶段改动范围）。"

也就是说，带页缓存的 `GenericBlockDevice::read/write`（DBFS2 实际走的那条）现在就是同步的，  
就算我想把 DBFS2 接到异步路径上，接口也不在那儿。

这一点在 boot 日志里也有旁证。2026-09-15 那次 boot 的中断自检打了 8 行：

```text
[blk-irq] #1 ack=true woke=false irq_enter=1 irq_wake=0 in_flight=0
...
[blk-irq] #8 ack=true woke=false irq_enter=8 irq_wake=0 in_flight=0
```

`irq_enter` 一路涨到 8，说明中断确实进来了；但 `irq_wake` 和 `in_flight` 始终是 0 ——
没有任何任务真的发过异步块请求，也没有人被唤醒过。文件系统这边走的完全是同步路径。

**第五条，write-through 要求"返回即落盘"，异步提交会把它破坏掉。**  
`BlockDevFile::write` 写完镜像立刻 `dev.write_at(..)`，靠这一点保证崩溃一致性。  
如果改成异步提交就返回，那"写成功"就不再是"数据到盘了"，持久化判断全部失效，  
还得再补一套 barrier / flush 协议才能救回来。

**第六条，绿色线程在裸内核里要自带栈，而 DBFS2 的写都是小批量。**  
Alien 的 task 有独立内核栈（栈大小这件事我吃过亏，已经从 8 KiB 调到 32 KiB）。  
起一个协程就要分配栈、参与调度。DBFS2 的写多是几百字节到几十 KiB 的 write-through，  
为它进一次调度器的成本比直接同步写完还高，而且会在文件系统路径里引入调度点，带来重入和锁序的风险。

所以我的判断是：JammDB 是 mmap 型的库，真正的 I/O 只有两处，一是"打开时把有效区间读进来"，  
二是"事务提交时把脏页写出去"，这两处都在 DBFS2 自己的同步函数里，天生就是批处理的。  
在每一次页访问上挂异步既没收益（页访问根本不产生 I/O），也不可能（借用跨不过 await）。  
**沿用作者原本的同步设计是对的，没有理由去改 future 或者绿色线程。**

> 说明一下：第一、二、四、五、六条是基于代码事实的推理，**没有做过同步 vs 异步的实测对比** ——  
> 我没有实现异步版本，所以拿不出两者的数据。第三条和第四条里关于 Alien 现状的部分是查代码确认的事实。

---

## 4. 具体改了哪些地方

改动是 **DBFS2 侧 3 处 + Alien 侧 3 处 + 新建 adapter**。不是"零改动"，但每一处都不大。

### 4.1 DBFS2 侧（3 处）

**包名冲突必须先化解。** DBFS2 的 `Cargo.toml` 里 `rvfs = { git = "Godones/rvfs" }` 跟 Alien 的  
`vfscore` 同名（包名也是 `rvfs`），放同一个构建图里必冲突。我的做法是把 DBFS2 那边的依赖改名，而不是删掉：

```toml
# _dbfs2_alien/dbfs2/Cargo.toml
rvfs_old = { package = "rvfs", git = "https://github.com/Godones/rvfs.git" }
```

然后 `common.rs` 里两处 `use rvfs::...` 改成 `use rvfs_old::...`。

**后端函数在外部调不到。** DBFS2 的 `lib.rs` 里这些模块是私有的（`mod file; mod inode; ...`，没写 `pub`），  
适配器在另一个 crate 里根本看不见 `dbfs_common_*`。加一层定向重导出就行：

```rust
// _dbfs2_alien/dbfs2/src/lib.rs
pub use file::{dbfs_common_open, dbfs_common_read, dbfs_common_readdir, dbfs_common_write};
pub use fs_type::{dbfs_common_root_inode, dbfs_common_statfs, dbfs_common_umount};
pub use inode::{dbfs_common_access, dbfs_common_create, dbfs_common_link,
                dbfs_common_lookup, dbfs_common_attr, dbfs_common_rename,
                dbfs_common_rmdir, dbfs_common_truncate};
pub use link::{dbfs_common_readlink, dbfs_common_unlink};
```

**供应链重定向。** DBFS2 依赖的几个 git 库在裸机/离线环境下有问题（`core2` 0.4 在 crates.io 上  
已经被全量 yank 了），在根 `Cargo.toml` 用 `[patch]` 指到仓库内的 vendor 副本：

```toml
[patch.crates-io]
core2 = { path = "_dbfs2_alien/vendor/core2-0.4.0" }

[patch."https://github.com/Godones/rvfs.git"]
rvfs = { path = "_dbfs2_alien/vendor/rvfs-patched" }

[patch."https://github.com/Godones/jammdb"]
jammdb = { path = "_dbfs2_alien/vendor/jammdb-patched" }
```

### 4.2 Alien 侧（3 处，都很小）

注册文件系统类型（`subsystems/vfs/src/lib.rs`）：

```rust
let dbfs = Arc::new(DbfsFs::new());
FS.lock().insert("dbfs".to_string(), dbfs);
```

块设备要在 devfs 扫描之前造好，顺序错了 devfs 扫描时就找不到 `/dev/dbfs` 这个节点（同一个文件的 `init_filesystem()`）：

```rust
devices::init_dbfs_ramdisk();   // 必须在 scan_system_devices 之前
register_all_fs();
```

挂载到 `/dbfs` 的部分见第 5 节，跟 fat32 完全同构。

### 4.3 新建的 adapter 层（`_dbfs2_alien/adapter/`）

主要工作量在这：

| 文件            | 干什么                                                                  |
| ------------- | -------------------------------------------------------------------- |
| `fstype.rs`   | `impl VfsFsType for DbfsFs` —— mount 入口，接块设备、开 JammDB、构造 root dentry |
| `inode.rs`    | `impl VfsInode / VfsFile for DbfsInode` —— 26 个方法转发到 `dbfs_common_*` |
| `blockdev.rs` | 块设备后端：`BlockDevFile` / `BlockDevMap` / `BlockDevOpenOptions`（见 3.3）  |
| `dentry.rs`   | `impl VfsDentry for DbfsDentry`                                      |
| `convert.rs`  | 类型/错误码转换（`DbfsError → VfsError`、`DbfsAttr → VfsFileStat` 等）          |

`DbfsInode` 很轻，只存一个 inode 号：

```rust
pub struct DbfsInode { pub ino: usize }
```

每次 `lookup` / `create` 返回的时候 `Arc::new` 一个新实例就行，不需要全局的 inode 注册表。

### 4.4 踩过的坑：root inode 号不能用返回值

`dbfs_common_root_inode()` 返回的**不是 inode 号**，是根目录 bucket 的 `size` 字段，也就是目录项计数。  
刚巧第一次挂载的时候根目录 size 是 1，所以看起来是对的；一旦复用一个已经有 N 个目录项的库（比如重启之后），  
root ino 就变成 1+N，`/dbfs` 下面所有 lookup / readdir / create 都会寻址到错误的 bucket。

所以代码里直接写死：

```rust
// _dbfs2_alien/adapter/src/fstype.rs
/// DBFS2 根 inode 号（= 根目录 bucket 的 id，恒为 1）。
pub const DBFS_ROOT_INODE: usize = 1;
```

（依据是上游 rvfs_old 的 `dbfs_create_root_inode` 里同样把 inode 号写死成 1，返回值只当 `file_size` 用。）

### 4.5 另一个坑：readdir 的游标语义对不上

DBFS2 的 `dbfs_common_readdir` 原本是给 FUSE 写的，内部用全局游标，而且带一个  
`assert!(offset == save_offset + 1)`，要求调用方连续递增地喂 offset。新版 VFS 的  
`readdir(start_index)` 语义不一样，是"每次返回一个 `Option`，按索引取"。

直接对接会 assert 失败，所以我在 adapter 里加了个缓存绕开：

```rust
// _dbfs2_alien/adapter/src/inode.rs
/// 只在 start_index==0 时整拉一次 dbfs_common_readdir，之后按索引 serve，
/// 绕开 DBFS2 内部 global cursor + assert。
static READDIR_CACHE: Mutex<BTreeMap<usize, Vec<VfsDirEntry>>> = Mutex::new(BTreeMap::new());
```

也就是第一次调用把整个目录拉进本地缓存，之后按 `start_index` 取 `vec[start_index]`，越界返回 `None`。  
不再二次喂 offset 给它。


## 6. 凭什么说挂载成功了

五层证据，从"挂上了"到"能用"。

### 证据 1：boot 时内核自己打印的挂载流程

`subsystems/vfs/src/lib.rs` 里给挂载加了分步打印（失败会打 ERROR 并返回 Err）。2026-09-15 04:20 那次 boot 的输出：

```text
[0] Init dbfs block device (RAMDISK) success
[0] [dbfs] step1: lookup /dev/dbfs ...
[0] [dbfs] step2: /dev/dbfs FOUND
[0] [dbfs] step3: got dev inode, i_mount ...
[0] [dbfs] step4: i_mount OK
[0] [dbfs] step5: mounted at /dbfs
[0] mount fs success
drwxr-xr-x      4KB tests
drwxr-xr-x      4KB dbfs        ← 紧接着打印的根目录列表里 /dbfs 出现了
```

这是"挂载成功"最直接的证据。step4 过了说明 DBFS2 内部初始化通过，step5 说明 dentry 已经挂到 `/dbfs`。
原始输出在同目录 `evidence/boot-full-selftest-2026-09-15.serial.txt`（那次 boot 的完整串口日志）。

### 证据 2：挂载后 `/dbfs` 可写

探针 `rc20.sh` 实测：

```text
--- 5) 直接写在 /dbfs 根下 ---
write /dbfs/.rc20f rc=0
```

### 证据 3：挂载后可建目录，而且目录是真的存在

探针 `rc22.sh` 实测：

```text
--- A) 单个 mkdir（无 -p）---
  A1 mkdir /dbfs/.rc22            rc=0  (期望 0)
  A2 mkdir /dbfs/.rc22 再来一次    rc=1  (期望 1/EEXIST)
  A2/A4 的 stderr 原文:
mkdir: can't create directory '/dbfs/.rc22': File exists
--- C) 对照：pjdfstest 的 mkdir（C 断言引擎，不走 busybox）---
  C1 pjdfstest mkdir 一级 rc=0
  C2 pjdfstest mkdir 二级(父已存在) rc=0
  C 组取证: [ -d /dbfs/rc22c/d1 ] = yes
```

`A1=0` 说明能建，`A2=rc1` 且报 `File exists` 说明重复建会被正确拒绝，  
`[ -d /dbfs/rc22c/d1 ] = yes` 说明目录确实在那儿，不是假的。

### 证据 4：在 `/dbfs` 上跑过标准 POSIX 一致性测试套件

pjdfstest（17 个目录 / 238 个 `.t` 文件）在 `/dbfs` 上实测过。以 `rename` 目录为例：

```text
ok 4407 / not ok 450   （共 4857 条断言，完整跑完）
```

能在 `/dbfs` 上跑几千条断言，前提就是它挂载可用，这本身就是对挂载的一种验证。

### 证据 5：内核自带的 DBFS2 自检四部分全过

开 `dbfs_selftest` feature 编译，boot 时内核会在挂载之后立刻跑一遍自检。2026-09-15 04:20 那次的结果：

```text
[dbfs-selftest] ============ DBFS2 selftest begin ============
part 1/4  transaction: atomicity & durability
  step2: tx dropped WITHOUT commit -> k is NOT visible        [Atomicity OK]
  step4: after commit, a NEW tx reads k=v_commit              [Durability OK]
part 1 PASS
part 2/4  file CRUD: create / write / read / readdir / unlink
  [C] create /dbfs/hello.txt -> OK
  [U] write  21 bytes -> OK
  [R] read   21 bytes = "hello from alien dbfs" -> OK
  [Q] readdir [".", "hello.txt"]
  [D] unlink /dbfs/hello.txt -> OK ; lookup after unlink -> Err (expected) -> OK
part 2 PASS
part 3/4  dir CRUD: mkdir / readdir / rmdir
  [C] mkdir /dbfs/selftest_dir -> OK
  [Q] readdir [".", ".."] (empty dir)
  [D] rmdir -> OK ; lookup after rmdir -> Err (expected) -> OK
part 3 PASS
part 4/4  write amplification: logical vs physical write
  [WA] logical write: 37689 bytes, physical write calls: 4 -> amp ratio ~1.00x
  [WA] sync_all (full-device rewrite): 0 calls / 0 bytes
  [WA] amplification(bytes) = 1.00x ; full-device rewrite share = 0.0%
  [WA] elapsed for 1 write tx (4096B) = 0 ms
part 4 PASS
[dbfs-selftest] ============ DBFS2 selftest PASS ============
[0] Init filesystem success
...
Init process is running
Alien:/#
```

这份自检说明了四件事：事务是原子的（没 commit 的写入不生效）、commit 之后是持久的（新事务能读到）、
文件和目录的增删改查都通、以及写放大是 1.00 倍（全盘回写那次冗余确实是 0）。
最后 `Alien:/#` 说明 init 进程起来了、shell 正常。

需要说明的是：这是**内核自带的自检**，不是第三方标准测试，它只能说明我列出来的这些用例是过的。

---

## 7. 还没做到的部分

挂载成功不等于功能完备。目前明确没做到的：

1. **pjdfstest 在 `/dbfs` 上还有失败项。** 上面那个 `not ok 450` 就是。其中一部分是平台层或者测试仪器的问题，  
   一部分是 DBFS2 真实缺陷，还没有逐条定性完。
2. **针对已发现缺陷的补丁没做完，这篇不写。** 之前在另一个分支上改过 vfscore 和 DBFS2 的几处缺陷  
   （符号链接、rename 相关），这些改动在当前仓库里没有重新编译加重新验证过，  
   所以正文一律不把它们算作成果，也不在这里列什么"已修复清单"。
3. **四臂对照矩阵没跑完。** 原计划是「FAT32/DBFS2 × 修复前/修复后」四格对照，实际只跑了两格就中断了，  
   没有完整的对照数据。
4. **标准性能工具没跑。** lmbench / iozone 都没跑过，没有吞吐数据。目前只有自检里那两个数：
   写放大 1.00 倍、一次 4096 B 写事务 0 ms（0 ms 说明时钟分辨率不够，不能当真值用）。
   3.5 节关于"同步 vs 异步"的判断同样没有实测支撑，只是设计上的推理。  
   也因此没有实测支撑，只是设计上的推理。
5. **崩溃一致性只做到"有脚本"。** `run_crash.sh` 是有的，但没有跑出过完整的  
   「写入 → 断电 → 重启 → 校验」通过记录。3.3 节说的 write-through 是代码层面的保证，不是端到端验证过的结论。
6. **Alien 上 `busybox mkdir -p` 恒失败**（报 `can't create directory '/': Invalid argument`，多级目录实际没建出来）。  
   这是平台层的缺陷，不是 DBFS2 的问题，但会影响任何用 `mkdir -p` 搭目录的测试流程，上面证据 3 的 A 组就撞上了。

---

## 附：怎么自己复现

```bash
cd ~/Alien
cargo build --release -p kernel \
    --target riscv64gc-unknown-none-elf \
    --features qemu,talloc,fat,dbfs_selftest,dbfs_persist

make run    # 或走 tests/third_party/runner/run_suite.sh
```

在串口输出里找 `[dbfs] step5: mounted at /dbfs`。

想进一步验证可写，进 shell 之后：

```sh
echo hello > /dbfs/testfile
cat /dbfs/testfile
mkdir /dbfs/testdir
ls /dbfs
```

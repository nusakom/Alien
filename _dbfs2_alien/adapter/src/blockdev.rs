//! DBFS2 块设备后端：把 JammDB 的 `DbFile` / `MemoryMap` 接到 Alien 的块设备 inode（`vfscore::inode::VfsInode`）。
//!
//! 设计目标（满足老师「DBFS2 必须坐在块结构之上」的反馈）：
//!   DBFS2 不再用内存裸指针旁路块设备，而是以 Alien 的块设备 inode（/dev/dbfs）作为
//!   它的存储后端，与 diskfs/fat32 同构——挂载时由 `DbfsFs::mount` 传入块设备 inode，
//!   所有页读写都经由 `VfsInode::read_at` / `write_at` 落到块设备层（RAMDISK / virtio-blk）。
//!
//! 实现完全镜像 jammdb 自带的 `memfile.rs`，仅把「堆裸指针」换成「块设备内存镜像 + 设备 inode」：
//! - `BlockDevFile`：持有块设备的内存镜像 `data: Vec<u8>`（长度恒 = `capacity`，`addr()` 指向它）
//!   与设备 inode `dev`。`open` 只把**有效区间**读入 `data`；`write` 同时写进 `data` 并
//!   write-through 到设备（持久化）；`read` 从 `data` 读；`flush` / `sync_all` 是 **no-op**
//!   ——因为 write 已逐扇区 write-through，且 JammDB 对映射只读、改动经 arena 后 `write_all`，
//!   映射中不存在待回写增量（原「整盘回写」是纯冗余，见 `sync_all` 注释）。
//!   这样 JammDB 的数据库物理上就落在 Alien 块设备层上。
//!
//! ★ **两个「大小」必须分开**（Commit 1 的修复核心，见 `BlockDevFile` 字段注释）：
//!   - `capacity`：设备容量，**同时是内存映射的合法范围**（`FileExt::size()` 返回它 ⇒
//!     决定 `IndexByPageID::index()` 的越界判据）。**不可改小**。
//!   - `logical_size`：DB 的**逻辑大小**（决定 `metadata().len()` / `Read` 的 EOF /
//!     `SeekFrom::End` 基准，以及 JammDB 是否需要 `allocate()` 扩容）。
//!   修复前二者恒等（`logical_size = capacity`），后果有两条：
//!   ① `metadata().len()` 恒 = 设备容量 ⇒ 扩容判据（`tx.rs:302-309`）永不成立；
//!   ② `open()` 必须整盘读入 `data` ⇒ mount 读 = **1.00 × 设备容量**
//!      （16 MiB = 4,096 页 × 8 扇区 = 32,768 次 512 B 串行请求），而有效数据仅 0.66%。
//!
//! - `BlockDevMap::do_map`：通过 `dyn DbFile` 的 `FileExt::addr()` / `size()` 取出稳定内存镜像的
//!   裸指针与长度，交给 JammDB 的页索引器（与 `FakeMap::do_map` 完全等价）。
//! - `BlockDevOpenOptions`：在 `open` 时从全局设备持有者取出 dev，构造 `BlockDevFile`。
//!
//! 挂载入口见 `crate::fstype::DbfsFs::mount`：
//!   `DB::open::<BlockDevOpenOptions, _>(Arc::new(BlockDevMap), "dbfs.db")` → `dbfs2::init_dbfs_with(db)`。

use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;

use core::fmt::{self, Debug, Display};

use core2::io::{ErrorKind, Read, Seek, SeekFrom, Write};

use vfscore::inode::VfsInode;

use jammdb::{
    DbFile, File, FileExt, IOResult, IndexByPageID, MemoryMap, MetaData, OpenOption, PathLike,
};

use spin::Mutex;

/// jammdb 的 MAGIC_VALUE（`db.rs` 私有常量，此处复制一份用于持久化判断）。
/// 首次 `init_file` 格式化时写入 meta 页；重启后据此判断「块设备是否已初始化」。
const JAMMDB_MAGIC: u32 = 0x00AB_CDEF;
/// meta 页 `page_type` 值（`Page::TYPE_META`）。
const PAGE_TYPE_META: u8 = 0x03;
/// jammdb 固定 4096 字节页（`db.rs::get_page_size`）。
const JAMMDB_PAGESIZE: usize = 4096;
/// meta 页数量：page 0 / page 1 交替写入，取二者 `num_pages` 的较大值作为安全上界。
const META_PAGES: usize = 2;

/// `Meta` 结构体在 meta 页内的字段偏移（`#[repr(C)]`；`Meta` 起于页内偏移 32 = `Page.ptr` 处）。
///
/// ```text
/// 页内偏移: 32 meta_page(u32) | 36 magic(u32)          | 40 version(u32)
///           48 pagesize(u64)  | 56 root.root_page(u64)  | 64 root.next_int(u64)
///           72 num_pages(u64) | 80 freelist_page(u64)   | 88 tx_id(u64) | 96 hash[32]
/// ```
///
/// ⚠️ 这些偏移与 `Meta` 的结构体布局**硬绑定、无编译期耦合**，因此 `probe_logical_size`
/// 必须做多重校验（页大小 / `num_pages` 区间 / `page_type` + `magic` 双条件），
/// 任一不满足即回退到保守路径。若将来升级 jammdb 并改动 `Meta` 布局，此处必须同步核对。
const META_OFF_PAGESIZE: usize = 48;
const META_OFF_NUM_PAGES: usize = 72;
/// `init_file` 至少写 4 页（`db.rs:350` `m.num_pages = 4`）⇒ 合法的 `num_pages` 下界。
const MIN_VALID_NUM_PAGES: u64 = 4;

#[inline]
fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&buf[off..off + 4]);
    u32::from_le_bytes(b)
}

#[inline]
fn read_u64_le(buf: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(b)
}

/// 写放大计数器（论文 M2 性能测试用）：
/// - `logical_write_bytes`：块设备层收到的「逻辑写」总字节数（即 jammdb 请求写出的字节）。
/// - `physical_page_writes`：write-through 到块设备 inode 的调用次数（每次调用近似一次物理页写扇出）。
///   二者结合可拆出「数据放大 vs 元数据放大」：COW 时同一逻辑页会被多次写出，计数次数即放大倍数。
/// 注意：write 是 write-through，每次 `BlockDevFile::write` 都会真实调 `dev.write_at`，
///       因此 physical_page_writes 直接对应「触发的块设备写扇出次数」。
pub static WRITE_AMPLIFY_STATS: Mutex<WriteAmplifyStats> = Mutex::new(WriteAmplifyStats {
    logical_write_bytes: 0,
    physical_write_calls: 0,
    physical_write_bytes: 0,
    sync_all_calls: 0,
    sync_all_bytes: 0,
});

#[derive(Debug, Clone, Copy, Default)]
pub struct WriteAmplifyStats {
    pub logical_write_bytes: usize,
    pub physical_write_calls: usize,
    /// 实际交给 `dev.write_at` 的字节总数。
    /// 移除整盘冗余回写后 = 逻辑写字节数（不再包含全设备回写）。
    pub physical_write_bytes: usize,
    /// `sync_all()`（全设备镜像回写）调用次数。
    /// **此后恒为 0**：`sync_all()` 已降级为 no-op（见该方法的注释）。
    /// 字段保留是因为 `crate::lib` 对外导出、`dbfs_selftest` / `dbfs_perf` 仍在读取，
    /// 删除会破坏调用方与打印格式；`0` 本身就是「冗余已消除」的可观测证据。
    pub sync_all_calls: usize,
    /// `sync_all()` 累计写出的字节数。**此后恒为 0**（同上）。
    pub sync_all_bytes: usize,
}

/// 读取并清零写放大统计（供性能测试在单次操作前后采样）。
pub fn take_write_amplify_stats() -> WriteAmplifyStats {
    let mut g = WRITE_AMPLIFY_STATS.lock();
    let s = *g;
    g.logical_write_bytes = 0;
    g.physical_write_calls = 0;
    g.physical_write_bytes = 0;
    g.sync_all_calls = 0;
    g.sync_all_bytes = 0;
    s
}

/// 设备持有者：mount 时由 `DbfsFs::mount` 登记块设备 inode；`BlockDevOpenOptions::open` 取走。
///
/// 用 `Mutex<Option<..>>` 而非 `Once`，便于（测试 / 重建时）重新指定设备。
static DBFS_BLOCK_DEV: Mutex<Option<Arc<dyn VfsInode>>> = Mutex::new(None);

/// mount 时调用：登记块设备 inode（/dev/dbfs 对应的 `Arc<dyn VfsInode>`）。
pub fn set_dbfs_block_device(dev: Arc<dyn VfsInode>) {
    *DBFS_BLOCK_DEV.lock() = Some(dev);
}

/// 取走登记的设备 inode；未登记时返回 None。
fn get_dbfs_block_device() -> Option<Arc<dyn VfsInode>> {
    DBFS_BLOCK_DEV.lock().clone()
}

/// 块设备镜像文件：data 是设备内容的内存副本（长度 = capacity），dev 是底层块设备 inode。
pub struct BlockDevFile {
    pub name: String,
    pub pos: usize,
    /// 设备内容镜像。**长度恒 = `capacity`**，且不可 realloc —— JammDB 经 `addr()` 裸指针
    /// 原地访问，指针必须稳定且页对齐。
    pub data: Vec<u8>,
    pub dev: Arc<dyn VfsInode>,
    /// 设备容量（字节）。**同时是内存映射的合法范围**：`FileExt::size()` 返回它，
    /// 进而决定 `IndexByPageIDImpl::size`（`index()` 的上界）。**不可改小**。
    pub capacity: usize,
    /// DB 的**逻辑大小**（字节，<= `capacity`）。决定 `metadata().len()`、`Read` 的 EOF、
    /// `SeekFrom::End` 的基准，以及 JammDB 的扩容判据（`tx.rs:302-309`）。
    ///
    /// 修复前此字段与 `capacity` 恒等，见模块头注释。
    pub logical_size: usize,
}

impl Seek for BlockDevFile {
    fn seek(&mut self, pos: SeekFrom) -> IOResult<u64> {
        match pos {
            SeekFrom::Start(l) => self.pos = l as usize,
            SeekFrom::Current(l) => {
                let new = self.pos as i64 + l;
                if new < 0 {
                    return Err(core2::io::Error::new(ErrorKind::Other, "seek error"));
                }
                self.pos = new as usize;
            }
            SeekFrom::End(l) => {
                let new = self.logical_size as i64 + l;
                if new < 0 {
                    return Err(core2::io::Error::new(ErrorKind::Other, "seek error"));
                }
                self.pos = new as usize;
            }
        };
        Ok(self.pos as u64)
    }
}

impl Read for BlockDevFile {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        // EOF 以**逻辑大小**为界（不是设备容量）。
        // 注：JammDB 从不通过 `Read` 读数据库（页读取全走 `IndexByPageID`），
        // 这里只是为了契约正确。
        if self.pos >= self.logical_size {
            return Ok(0);
        }
        let remain = self.logical_size - self.pos;
        let act_size = if remain > buf.len() { buf.len() } else { remain };
        let start = self.pos;
        buf[..act_size].copy_from_slice(&self.data[start..start + act_size]);
        self.pos += act_size;
        Ok(act_size)
    }
}

impl Write for BlockDevFile {
    fn write(&mut self, buf: &[u8]) -> IOResult<usize> {
        let end = self.pos + buf.len();
        // 硬上限是**设备容量**（内存镜像只有这么大）。
        if end > self.capacity {
            return Err(core2::io::Error::new(
                ErrorKind::Other,
                "dbfs write beyond block device capacity",
            ));
        }
        // 逻辑大小随写入增长（与 `memfile::MemoryFile::write` 的 realloc 分支等价，
        // 只是我们的缓冲已预分配为 capacity，无需 realloc）。
        if end > self.logical_size {
            self.logical_size = end;
        }
        let start = self.pos;
        // 1) 写进内存镜像
        self.data[start..start + buf.len()].copy_from_slice(buf);
        // 2) write-through 到块设备，保证 DB 物理落在块设备层（满足「块结构」约束）
        self.dev
            .write_at(start as u64, buf)
            .map_err(|_| core2::io::Error::new(ErrorKind::Other, "write to block device failed"))?;
        // 埋点：统计写放大（逻辑写字节 + 物理写调用次数）
        {
            let mut st = WRITE_AMPLIFY_STATS.lock();
            st.logical_write_bytes += buf.len();
            st.physical_write_calls += 1;
            st.physical_write_bytes += buf.len();
        }
        self.pos += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> IOResult<()> {
        // 所有写已在 `Write::write` 中 write-through 到块设备
        // （`drivers/src/block_device.rs:220-231`）；JammDB 对内存映射只做**只读**访问、
        // 改动一律经 arena 暂存后 `write_all` 落盘 ⇒ 不存在待回写增量。
        // 证据：`docs/phase8.3-c`、`docs/phase8.3-d`、`docs/phase8.3-e-step5`（两臂 CRASH_CONSISTENCY,PASS）。
        Ok(())
    }
}

impl FileExt for BlockDevFile {
    fn lock_exclusive(&self) -> IOResult<()> {
        Ok(())
    }

    fn allocate(&mut self, new_size: u64) -> IOResult<()> {
        // 硬上限仍是设备容量：`new_size > capacity` 表示 DB 已超出设备，必须**响亮报错**
        // 而不是静默夹断（夹断会让后续 `write` 在更远的地方失败，更难定位）。
        if new_size as usize > self.capacity {
            return Err(core2::io::Error::new(
                ErrorKind::Other,
                "dbfs allocate beyond block device capacity",
            ));
        }
        if self.logical_size < new_size as usize {
            // data 已被预分配为 capacity 长度，无需 realloc，仅调整逻辑大小即可（裸指针稳定）。
            //
            // ⚠️ 这里**不**回读 `[old_logical_size, new_size)` 区间在设备上的字节，保持 0。
            //    依据（不变量 I-SIZE）：JammDB 只索引 `page_id < meta.num_pages` 的页，
            //    而新页号正是从 `meta.num_pages` 开始分配的（`freelist.rs:52-54`），
            //    且新页先经 arena 暂存写满、再 `write_all` 落盘（`tx.rs:312-320`）
            //    —— 因此该区间内的页**只会被写、不会被读**。
            //    回读会引入最多 `MIN_ALLOC_SIZE`(8 MiB) 的无谓 I/O。
            self.logical_size = new_size as usize;
        }
        Ok(())
    }

    fn unlock(&self) -> IOResult<()> {
        Ok(())
    }

    fn metadata(&self) -> IOResult<MetaData> {
        // ★ 关键修复点：报告**逻辑大小**（不是设备容量）。
        // JammDB 的 `tx.rs:302-309` 用 `metadata().len()` 与 `required_size = num_pages * pagesize`
        // 比较来决定是否扩容。此前恒等容量 ⇒ 该分支永不进入 ⇒ 扩容逻辑形同虚设。
        Ok(MetaData {
            len: self.logical_size as u64,
        })
    }

    fn sync_all(&self) -> IOResult<()> {
        // 原实现把整盘镜像（= `capacity`）再写一遍：每次 commit 2 次 ⇒ 2 × 16 MiB = 33,554,432 B，
        // 占设备写入 99.89–99.99%，是 891.30× 写放大的唯一来源。它是**纯冗余**：
        //   ① `Write::write` 已逐扇区 write-through（`drivers/src/block_device.rs:220-231`）⇒ 返回即落盘；
        //   ② JammDB 对内存映射只读、改动经 arena 暂存后 `write_all`
        //      （`page.rs:36-39` 返回 `&Page`；`tx.rs:312-320`/`:348`）⇒ 映射里不存在「未回写的增量」；
        //   ③ ⑤ 端到端实证：no-op 臂与 ON 臂 boot#2 日志逐行一致，两臂均 `CRASH_CONSISTENCY,PASS`。
        //
        // 方法体保留（`DbFile` trait 契约；调用点 `tx.rs:350-351`、`db.rs:365-366` 一律不动），
        // 但降级为 no-op。**不要再在这里补回写**：那会立刻恢复 891× 写放大。
        Ok(())
    }

    /// 返回**内存映射的合法范围**（= `capacity`），**不是**逻辑文件大小。
    ///
    /// `BlockDevMap::do_map` 用它构造 `IndexByPageIDImpl`，进而决定
    /// `IndexByPageID::index()` 的越界判据。保持 = `capacity`（宽松上界）是**刻意**的：
    /// JammDB 以 mmap 风格按 `page_id` 直接取页，若这里收紧到 `logical_size`，
    /// 任何「页号 ≥ 逻辑大小」的访问都会变成 panic，可能把原本正确的路径打坏。
    /// 逻辑文件大小请用 `metadata().len()`。
    fn size(&self) -> usize {
        self.capacity
    }

    fn addr(&self) -> usize {
        self.data.as_ptr() as usize
    }
}

impl DbFile for BlockDevFile {}

/// 持久化判定用 `PathLike`：`exists()` 读块设备头部，判断 jammdb 是否已初始化。
///
/// 背景：`jammdb::DB::open` 通过 `path.exists()` 决定「新建格式化」还是「复用已有数据」。
/// 内置的 `PathLike for &str` 检查的是 memfile 的 `FILE_S` 注册表（内存后端专用），
/// 我们的块设备后端从不注册，导致 `exists()` 恒为 `false` —— 每次 boot 都重新格式化，
/// 覆盖掉上一次写盘的数据（这就是 crash_verify 全 MISSING 的根因）。
///
/// 本类型直接读块设备前 4KB：若 meta 页的 `page_type == TYPE_META` 且 `magic == JAMMDB_MAGIC`，
/// 说明已格式化，返回 `true`（复用）；否则返回 `false`（首次格式化）。
pub struct DbfsPathLike;

impl Display for DbfsPathLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dbfs.db")
    }
}

impl Debug for DbfsPathLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dbfs.db")
    }
}

impl PathLike for DbfsPathLike {
    fn exists(&self) -> bool {
        // 读块设备头部 4KB，检查 meta 页 magic。
        // 布局（jammdb Page/Meta，#[repr(C)]）：
        //   Page { id:u64(0), page_type:u8(8), pad(9..16), count:u64(16), overflow:u64(24), ptr:u64(32) }
        //   Meta { meta_page:u32(32), magic:u32(36), ... }
        // 因此 page_type 在 offset 8，magic 在 offset 36。
        let dev = match get_dbfs_block_device() {
            Some(d) => d,
            None => return false,
        };
        let mut head = [0u8; 4096];
        if dev.read_at(0, &mut head).is_err() {
            return false;
        }
        // page_type == TYPE_META(0x03) 且 magic == 0x00AB_CDEF
        head.get(8) == Some(&PAGE_TYPE_META) && {
            // 端序说明（D1 修复）：页镜像里存的是 `#[repr(C)] struct Meta` 的**原生内存布局**
            // （`Page::meta()` 直接把 offset 32 处的 `ptr` reinterpret 成 `*const Meta`，
            //  `tx.rs` 用 `from_raw_parts` 把结构体原样写盘，**没有做任何字节序序列化**）。
            // 因此 `magic: u32` 在镜像中的字节序 = 目标机原生字节序。Alien 目标为
            // riscv64gc（小端），`0x00AB_CDEF` 落盘为 `EF CD AB 00`，必须用 `from_le_bytes`。
            // （原先用 `from_be_bytes` 会得到 0xEFCDAB00，恒失配 ⇒ exists() 恒 false ⇒ 每次
            //  boot 都重新格式化，这是 8.3-D 已确证的 D1 缺陷。）
            // 若将来目标改为大端，此处须同步改为 `from_ne_bytes`。
            let magic = u32::from_le_bytes([head[36], head[37], head[38], head[39]]);
            magic == JAMMDB_MAGIC
        }
    }
}

/// `probe_logical_size` 的三态结果。
enum SizeProbe {
    /// 设备上**没有**有效 JammDB meta ⇒ 首次格式化：设备内容无意义，无需读取任何字节。
    Fresh,
    /// 复用：设备上 DB 的逻辑大小（已校验并落在合法区间内）。
    Extent(usize),
    /// 有 meta 但字段异常（页大小不符 / `num_pages` 越界）⇒ 保守回退到旧行为（整盘读）。
    Fallback,
}

/// 离线探测设备上 JammDB 的逻辑 extent（= `meta.num_pages * pagesize`）。
///
/// **为什么必须探测**：`do_map` 只在 `DBInner::open` 里被调用一次（`db.rs:226`），
/// 之后 `IndexByPageID` 就以那一刻的 size 为准；而「DB 到底有多大」只有 meta 页知道。
/// 修复前把逻辑大小直接设成设备容量，于是 `metadata().len()` 恒等于容量，
/// 既让扩容逻辑失效，又迫使 `open()` 整盘读。
///
/// **判定依据**：`page_type == TYPE_META` 且 `magic == JAMMDB_MAGIC`（与 `DbfsPathLike::exists` 一致）。
/// 两个 meta 页都读，取 `num_pages` 的**较大值**作为安全上界
/// （覆盖掉电时两页 `tx_id` 不一致、其中一页偏旧的情况）。
fn probe_logical_size(dev: &Arc<dyn VfsInode>, capacity: usize) -> SizeProbe {
    let mut head = [0u8; JAMMDB_PAGESIZE * META_PAGES];
    if dev.read_at(0, &mut head).is_err() {
        // meta 都读不出来：按空设备处理（后续 `DB::open` 会格式化，或对损坏库响亮报错）。
        return SizeProbe::Fresh;
    }

    let pagesize = read_u64_le(&head, META_OFF_PAGESIZE) as usize;

    let mut max_num_pages: u64 = 0;
    let mut found_valid_meta = false;
    for page_idx in 0..META_PAGES {
        let base = page_idx * JAMMDB_PAGESIZE;
        // page_type 在页内偏移 8；magic 在页内偏移 36（= Meta 内偏移 4，Meta 起于页内 32）。
        if head[base + 8] != PAGE_TYPE_META {
            continue;
        }
        if read_u32_le(&head, base + 36) != JAMMDB_MAGIC {
            continue;
        }
        found_valid_meta = true;
        let num_pages = read_u64_le(&head, base + META_OFF_NUM_PAGES);
        if num_pages > max_num_pages {
            max_num_pages = num_pages;
        }
    }

    if !found_valid_meta {
        return SizeProbe::Fresh;
    }

    // 三重校验（offset 硬编码、无编译期耦合 ⇒ 必须防错）：
    // ① 页大小必须是 jammdb 固定的 4096（adapter 其余部分同样按 4096 假设布局）；
    // ② `num_pages` 下界 = 4（`init_file` 至少写 4 页，`db.rs:350`）；
    // ③ `num_pages` 上界 = 设备可容纳的页数（DB 不可能大于设备，否则写不下去）。
    if pagesize != JAMMDB_PAGESIZE {
        return SizeProbe::Fallback;
    }
    if max_num_pages < MIN_VALID_NUM_PAGES || max_num_pages as usize > capacity / JAMMDB_PAGESIZE {
        return SizeProbe::Fallback;
    }

    SizeProbe::Extent(max_num_pages as usize * JAMMDB_PAGESIZE)
}

/// 与 memfile::FileOpenOptions 等价：open 时从全局设备持有者取出块设备，构造 BlockDevFile。
pub struct BlockDevOpenOptions;

impl OpenOption for BlockDevOpenOptions {
    fn new() -> Self {
        BlockDevOpenOptions
    }

    fn read(&mut self, _: bool) -> &mut Self {
        self
    }

    fn write(&mut self, _: bool) -> &mut Self {
        self
    }

    fn open<T: ToString + PathLike>(&mut self, path: &T) -> IOResult<File> {
        let dev = get_dbfs_block_device().ok_or_else(|| {
            core2::io::Error::new(
                ErrorKind::Other,
                "dbfs block device not set (call set_dbfs_block_device first)",
            )
        })?;

        // 设备容量：通过 VfsInode::get_attr().st_size 取得。
        let capacity = dev
            .get_attr()
            .map_err(|_| core2::io::Error::new(ErrorKind::Other, "block device get_attr failed"))?
            .st_size as usize;

        // ── Commit 1 核心改动 ────────────────────────────────────────────────────
        // 先离线探测 DB 的逻辑 extent，由它同时决定「读多少字节」与「报告多大的逻辑大小」。
        //
        // 修复前：逻辑大小直接取 `capacity`，于是
        //   ① `metadata().len()` 恒 = 设备容量 ⇒ 扩容判据（`tx.rs:302-309`）永不成立；
        //   ② 下面必须 `read_at(0, capacity)` 整盘读入 ⇒ mount 读 = 1.00 × 设备容量
        //      （16 MiB = 4,096 页 × 8 扇区 = 32,768 次 512 B 串行请求），而有效数据仅 0.66%。
        // 注意：`data` 缓冲**仍然**是 `capacity` 长度（`addr()` 必须稳定，见 `BlockDevFile` 注释），
        // 只是不再把整盘内容都读进来。
        let (logical_size, read_len) = match probe_logical_size(&dev, capacity) {
            // 空设备（首次格式化）：设备上没有有意义的内容，一个字节都不用读。
            // 逻辑大小取 `capacity` 是**刻意保守**的：与修复前逐字节一致，
            // 不引入「首次 mount 就走 allocate/extend 隐藏路径」的新变量。
            SizeProbe::Fresh => (capacity, 0usize),
            // 复用已有 DB：只读有效区间，并把真实逻辑大小报给 JammDB。
            SizeProbe::Extent(extent) => (extent, extent),
            // meta 字段异常：保守沿用旧行为（整盘读 + 逻辑大小 = 容量）。
            SizeProbe::Fallback => (capacity, capacity),
        };

        // 分配内存镜像（长度 = capacity）；未读到的部分保持 0。
        let mut data = Vec::new();
        data.resize(capacity, 0u8);
        if read_len > 0 {
            dev.read_at(0, &mut data[..read_len])
                .map_err(|_| core2::io::Error::new(ErrorKind::Other, "read block device failed"))?;
        }

        // 关于「逻辑大小不能为 0」的历史修正（保留结论，语义已分离）：
        //   若逻辑大小为 0，`BlockDevMap::do_map` 用 `file.size()` 构造 `IndexByPageID`，
        //   其 `index()` 边界检查会在复用路径第一次 `db.meta()` 读 page 0 时越界失败
        //   ⇒ `DB::open` 出错、整库被重新格式化。
        //   现在这条由 `FileExt::size() = capacity`（映射范围）独立保证，与逻辑大小解耦；
        //   逻辑大小只影响 `metadata().len()` / `Read` EOF / 扩容判据。
        let file = BlockDevFile {
            name: path.to_string(),
            pos: 0,
            data,
            dev,
            capacity,
            logical_size,
        };
        Ok(File::new(Box::new(file)))
    }

    fn create(&mut self, _: bool) -> &mut Self {
        self
    }
}

/// 与 memfile::FakeMap 等价：do_map 通过 dyn DbFile 的 FileExt::addr/size 取出稳定内存镜像，
/// 交给 JammDB 页索引器（与 FakeMap 完全等价——地址来自块设备镜像而非堆）。
pub struct BlockDevMap;

impl MemoryMap for BlockDevMap {
    fn do_map(&self, file: &mut File) -> IOResult<Arc<dyn IndexByPageID>> {
        let addr = file.file.addr();
        // `FileExt::size()` 返回的是**内存映射的合法范围**（= `capacity`），
        // 不是逻辑文件大小 —— 两个概念刻意分离（见模块头注释与 `BlockDevFile` 字段注释）。
        let size = file.file.size();
        Ok(Arc::new(IndexByPageIDImpl { size, addr }))
    }
}

/// 页索引器：`size` 是**内存映射的合法范围**（`capacity`），因此 `index()` 是**宽松上界**。
///
/// 保持宽松是刻意的：JammDB 以 mmap 风格按 `page_id` 直接取页（`page.rs:36-39` 返回 `&Page`），
/// 若把上界收紧到逻辑大小，任何「页号 ≥ 逻辑大小」的访问都会 panic，可能打坏原本正确的路径。
/// 逻辑大小只管 `metadata().len()` / `Read` EOF / 扩容判据。
struct IndexByPageIDImpl {
    size: usize,
    addr: usize,
}

impl IndexByPageID for IndexByPageIDImpl {
    fn index(&self, page_id: u64, page_size: usize) -> IOResult<&[u8]> {
        let start = page_id as usize * page_size;
        if start + page_size > self.size {
            return Err(core2::io::Error::new(
                ErrorKind::Other,
                "dbfs page index out of range",
            ));
        }
        let addr = self.addr + start;
        let data = unsafe { core::slice::from_raw_parts(addr as *const u8, page_size) };
        Ok(data)
    }

    fn len(&self) -> usize {
        self.size
    }
}

//! DBFS2 块设备后端：把 JammDB 的 `DbFile` / `MemoryMap` 接到 Alien 的块设备 inode（`vfscore::inode::VfsInode`）。
//!
//! 设计目标（满足老师「DBFS2 必须坐在块结构之上」的反馈）：
//!   DBFS2 不再用内存裸指针旁路块设备，而是以 Alien 的块设备 inode（/dev/dbfs）作为
//!   它的存储后端，与 diskfs/fat32 同构——挂载时由 `DbfsFs::mount` 传入块设备 inode，
//!   所有页读写都经由 `VfsInode::read_at` / `write_at` 落到块设备层（RAMDISK / virtio-blk）。
//!
//! 实现完全镜像 jammdb 自带的 `memfile.rs`，仅把「堆裸指针」换成「块设备内存镜像 + 设备 inode」：
//! - `BlockDevFile`：持有块设备的整块内存镜像 `data: Vec<u8>` 与设备 inode `dev`。
//!   `open` 时把整块设备读入 `data`；`write` 同时写进 `data` 并 write-through 到设备（持久化）；
//!   `read` 从 `data` 读；`sync_all` 兜底把 `data` 整体刷回设备。
//!   这样 JammDB 的数据库物理上就落在 Alien 块设备层上。
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
    /// 实际交给 `dev.write_at` 的字节总数（= 逻辑写 + 全设备回写）。
    pub physical_write_bytes: usize,
    /// `sync_all()`（全设备镜像回写）调用次数。
    pub sync_all_calls: usize,
    /// `sync_all()` 累计写出的字节数。
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
    pub data: Vec<u8>, // 设备内容镜像（长度 = capacity）
    pub dev: Arc<dyn VfsInode>,
    pub capacity: usize, // 设备容量（字节）
    pub size: usize,     // 逻辑文件大小（<= capacity）
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
                let new = self.size as i64 + l;
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
        if self.pos >= self.size {
            return Ok(0);
        }
        let remain = self.size - self.pos;
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
        if end > self.capacity {
            return Err(core2::io::Error::new(
                ErrorKind::Other,
                "dbfs write beyond block device capacity",
            ));
        }
        if end > self.size {
            self.size = end;
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
        // 所有写都已 write-through，这里仅做兜底全量刷回。
        self.sync_all()
    }
}

impl FileExt for BlockDevFile {
    fn lock_exclusive(&self) -> IOResult<()> {
        Ok(())
    }

    fn allocate(&mut self, new_size: u64) -> IOResult<()> {
        if new_size as usize > self.capacity {
            return Err(core2::io::Error::new(
                ErrorKind::Other,
                "dbfs allocate beyond block device capacity",
            ));
        }
        if self.size < new_size as usize {
            // data 已被预分配为 capacity 长度，无需 realloc，仅调整逻辑大小即可（裸指针稳定）。
            self.size = new_size as usize;
        }
        Ok(())
    }

    fn unlock(&self) -> IOResult<()> {
        Ok(())
    }

    fn metadata(&self) -> IOResult<MetaData> {
        Ok(MetaData {
            len: self.size as u64,
        })
    }

    fn sync_all(&self) -> IOResult<()> {
        // 兜底：把内存镜像整体刷回块设备，实现持久化。
        let n = self.size;
        self.dev
            .write_at(0, &self.data[..n])
            .map_err(|_| core2::io::Error::new(ErrorKind::Other, "sync to block device failed"))?;
        // 埋点（Phase 8.3-B）：全设备回写此前**完全未被统计**，而它才是每次 commit 的
        // 主要物理写入来源。不埋这里会得到严重偏低的放大比，实验结论失真。
        {
            let mut st = WRITE_AMPLIFY_STATS.lock();
            st.physical_write_calls += 1;
            st.physical_write_bytes += n;
            st.sync_all_calls += 1;
            st.sync_all_bytes += n;
        }
        Ok(())
    }

    fn size(&self) -> usize {
        self.size
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

        // 把整块设备读进内存镜像。
        let mut data = Vec::new();
        data.resize(capacity, 0u8);
        dev.read_at(0, &mut data)
            .map_err(|_| core2::io::Error::new(ErrorKind::Other, "read block device failed"))?;

        // 逻辑大小 = 设备容量（整块设备已读入 data，addr 裸指针指向完整 buffer）。
        //
        // 关键修正（crash_verify 全 MISSING 的第二个根因）：若 size=0，
        // BlockDevMap::do_map 用 file.size() 构造 IndexByPageID，其 index() 边界检查
        // `start + page_size > size` 会在复用路径（重启后 DB::open 读已持久化数据）第一次
        // db.meta() 读 page 0 时就越界失败，导致 DB::open 出错、整库被重新格式化。
        //
        // 而 memfile 后端之所以 size 能从 0 开始，是因为它靠 FILE_S 全局表跨 open 保持 size；
        // 块设备后端没有这张表，必须用设备容量作为稳定 size（与 addr 裸指针稳定一致）。
        let file = BlockDevFile {
            name: path.to_string(),
            pos: 0,
            data,
            dev,
            capacity,
            size: capacity,
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
        let size = file.file.size();
        Ok(Arc::new(IndexByPageIDImpl { size, addr }))
    }
}

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

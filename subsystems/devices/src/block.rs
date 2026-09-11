use alloc::sync::Arc;
#[cfg(not(feature = "dbfs_persist"))]
use alloc::{boxed::Box, vec::Vec};
use platform::println;

use constants::DeviceId;
use device_interface::BlockDevice;
use drivers::block_device::GenericBlockDevice;
use spin::Once;
use vfscore::{
    error::VfsError,
    file::VfsFile,
    inode::{InodeAttr, VfsInode},
    utils::{VfsFileStat, VfsNodeType, VfsPollEvents},
    VfsResult,
};
pub static BLOCK_DEVICE: Once<Arc<GenericBlockDevice>> = Once::new();

/// DBFS2 专用的独立块设备（与 /dev/sda 解耦，避免互相干扰）。
/// 由 `init_dbfs_ramdisk` 在内存中造一块零初始化的 RAMDISK，作为 DBFS2 的存储后端，
/// 使 DBFS2 像 diskfs/fat32 一样坐在块设备层之上（满足「块结构」约束）。
/// 当启用 `dbfs_persist` feature 时，则改为由第二块 virtio-blk 盘填充（见 init_block_device）。
pub static DBFS_BLOCK_DEVICE: Once<Arc<GenericBlockDevice>> = Once::new();

/// virtio-blk 盘序号计数器：第一块给 /dev/sda（fat32/diskfs），
/// 第二块（当 `dbfs_persist` 开启时）给 /dev/dbfs（DBFS2）。
/// 用原子计数区分两块盘，避免依赖 base_addr 的硬编码。
static VIRTIO_BLK_INDEX: spin::Mutex<usize> = spin::Mutex::new(0);

/// 登记一块 virtio-blk 设备：第一块 -> BLOCK_DEVICE(/dev/sda)，第二块 -> DBFS_BLOCK_DEVICE(/dev/dbfs)。
/// `dbfs_persist` feature 关闭时，DBFS2 仍走 RAMDISK（init_dbfs_ramdisk），第二块盘被忽略。
pub fn init_block_device(block_device: Arc<GenericBlockDevice>) {
    let mut idx = VIRTIO_BLK_INDEX.lock();
    let n = *idx;
    *idx += 1;
    drop(idx);

    if n == 0 {
        // 第一块：sda（fat32/diskfs 的持久化后端）
        BLOCK_DEVICE.call_once(|| block_device);
    } else if n == 1 {
        // 第二块：dbfs（DBFS2 的持久化后端）
        // 仅在 dbfs_persist feature 开启时接管；否则忽略（DBFS2 用 RAMDISK）。
        #[cfg(feature = "dbfs_persist")]
        DBFS_BLOCK_DEVICE.call_once(|| block_device);
        #[cfg(not(feature = "dbfs_persist"))]
        {
            let _ = block_device;
            println!("Ignoring 2nd virtio-blk device (dbfs_persist feature disabled)");
        }
    } else {
        println!("Ignoring virtio-blk device index {}", n);
    }
}

/// 构造 DBFS2 的块设备。
///
/// - `dbfs_persist` 关闭（默认）：在堆上分配一块零初始化的 RAMDISK，包装成 GenericBlockDevice。
///   镜像 init_ramdisk 的做法，但使用独立缓冲区，且零初始化（供 JammDB 首次格式化）。
/// - `dbfs_persist` 开启：不造 RAMDISK，DBFS2 改用第二块 virtio-blk 盘（由 init_block_device
///   在 init_device 阶段填充 DBFS_BLOCK_DEVICE），实现真正的持久化（崩溃一致性测试的前提）。
///
/// 必须在 vfs 的 scan_system_devices（注册 /dev/dbfs）之前调用。
pub fn init_dbfs_ramdisk() {
    #[cfg(feature = "dbfs_persist")]
    {
        // 持久化模式：DBFS_BLOCK_DEVICE 已由第二块 virtio-blk 填充。
        // 若未填充（qemu 没挂第二块盘），这里 panic 提示配置错误。
        if DBFS_BLOCK_DEVICE.get().is_none() {
            panic!("dbfs_persist enabled but no 2nd virtio-blk device found (add -drive file=tools/dbfs.img -device virtio-blk-device)");
        }
        println!("Init dbfs block device (persistent virtio-blk) success");
    }

    #[cfg(not(feature = "dbfs_persist"))]
    {
        use drivers::block_device::MemoryFat32Img;

        const DBFS_DISK_SIZE: usize = 16 * 1024 * 1024; // 16MB，对齐 dbfs2 super_blk 的 disk_size

        // 在已初始化的内核堆上分配零化缓冲区，并泄漏为 'static，供 MemoryFat32Img 持有。
        let mut buf: Vec<u8> = Vec::new();
        buf.resize(DBFS_DISK_SIZE, 0u8);
        let buf: &'static mut [u8] = Box::leak(buf.into_boxed_slice());

        let block_device = GenericBlockDevice::new(Box::new(MemoryFat32Img::new(buf)));
        let block_device = Arc::new(block_device);
        DBFS_BLOCK_DEVICE.call_once(|| block_device);
        println!("Init dbfs block device (RAMDISK) success");
    }
}

pub struct BLKDevice {
    device_id: DeviceId,
    device: Arc<GenericBlockDevice>,
}

impl BLKDevice {
    pub fn new(device_id: DeviceId, device: Arc<GenericBlockDevice>) -> Self {
        Self { device_id, device }
    }
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }
}

impl VfsFile for BLKDevice {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        self.device
            .read(buf, offset as usize)
            .map_err(|_| VfsError::IoError)
    }
    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        self.device
            .write(buf, offset as usize)
            .map_err(|_| VfsError::IoError)
    }
    fn poll(&self, _event: VfsPollEvents) -> VfsResult<VfsPollEvents> {
        unimplemented!()
    }
    fn ioctl(&self, _cmd: u32, _arg: usize) -> VfsResult<usize> {
        unimplemented!()
    }
    fn flush(&self) -> VfsResult<()> {
        Ok(())
    }
    fn fsync(&self) -> VfsResult<()> {
        Ok(())
    }
}

impl VfsInode for BLKDevice {
    fn set_attr(&self, _attr: InodeAttr) -> VfsResult<()> {
        Ok(())
    }
    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        Ok(VfsFileStat {
            st_rdev: self.device_id.id(),
            st_size: self.device.size() as u64,
            st_blksize: 512,
            ..Default::default()
        })
    }
    fn inode_type(&self) -> VfsNodeType {
        VfsNodeType::BlockDevice
    }
}

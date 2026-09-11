#![feature(c_variadic)]
#![no_std]

extern crate alloc;
#[macro_use]
extern crate platform;
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
};
use core::ops::Index;

use constants::AlienResult;
use dynfs::DynFsKernelProvider;
use ksync::Mutex;
use spin::{Lazy, Once};
#[cfg(feature = "ext")]
use vfscore::inode::VfsInode;
use vfscore::{dentry::VfsDentry, fstype::VfsFsType, path::VfsPath, utils::VfsTimeSpec};

use crate::dev::DevFsProviderImpl;
use dbfs2_adapter::DbfsFs;
pub mod dev;
#[cfg(feature = "dbfs_selftest")]
pub mod dbfs_selftest;
pub mod epoll;
pub mod eventfd;
#[cfg(feature = "ext")]
mod extffi;
mod initrd;
pub mod kfile;
pub mod pipefs;
pub mod proc;
pub mod ram;
pub mod sys;
pub mod timerfd;

pub static FS: Lazy<Mutex<BTreeMap<String, Arc<dyn VfsFsType>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

static SYSTEM_ROOT_FS: Once<Arc<dyn VfsDentry>> = Once::new();

type SysFs = dynfs::DynFs<CommonFsProviderImpl, spin::Mutex<()>>;
type ProcFs = dynfs::DynFs<CommonFsProviderImpl, spin::Mutex<()>>;
type RamFs = ramfs::RamFs<CommonFsProviderImpl, spin::Mutex<()>>;
type DevFs = devfs::DevFs<DevFsProviderImpl, spin::Mutex<()>>;
type TmpFs = ramfs::RamFs<CommonFsProviderImpl, spin::Mutex<()>>;
type PipeFs = dynfs::DynFs<CommonFsProviderImpl, spin::Mutex<()>>;

#[cfg(feature = "fat")]
type DiskFs = fat_vfs::FatFs<CommonFsProviderImpl, spin::Mutex<()>>;

#[cfg(feature = "ext")]
type DiskFs = lwext4_vfs::ExtFs<CommonFsProviderImpl, spin::Mutex<()>>;

#[derive(Clone)]
pub struct CommonFsProviderImpl;

impl DynFsKernelProvider for CommonFsProviderImpl {
    fn current_time(&self) -> VfsTimeSpec {
        VfsTimeSpec::new(0, 0)
    }
}

impl ramfs::RamFsProvider for CommonFsProviderImpl {
    fn current_time(&self) -> VfsTimeSpec {
        DynFsKernelProvider::current_time(self)
    }
}

#[cfg(feature = "fat")]
impl fat_vfs::FatFsProvider for CommonFsProviderImpl {
    fn current_time(&self) -> VfsTimeSpec {
        DynFsKernelProvider::current_time(self)
    }
}

#[cfg(feature = "ext")]
impl lwext4_vfs::ExtDevProvider for CommonFsProviderImpl {
    fn rdev2device(&self, rdev: u64) -> Option<Arc<dyn VfsInode>> {
        use constants::DeviceId;
        use dev::DEVICES;
        let device_id = DeviceId::from(rdev);
        DEVICES.lock().get(&device_id).cloned()
    }
}

fn register_all_fs() {
    let procfs = Arc::new(ProcFs::new(CommonFsProviderImpl, "procfs"));
    let sysfs = Arc::new(SysFs::new(CommonFsProviderImpl, "sysfs"));
    let ramfs = Arc::new(RamFs::new(CommonFsProviderImpl));
    let devfs = Arc::new(DevFs::new(DevFsProviderImpl));
    let tmpfs = Arc::new(TmpFs::new(CommonFsProviderImpl));
    let pipefs = Arc::new(PipeFs::new(CommonFsProviderImpl, "pipefs"));

    // DBFS2：数据库文件系统，坐在块设备层之上（/dev/dbfs 为其存储后端）。
    // 与 diskfs/fat32 同构：挂载时由 VfsFsType::mount 接收块设备 inode，所有页读写
    // 经由 VfsInode::read_at / write_at 落到块设备层（满足「块结构」约束）。
    let dbfs = Arc::new(DbfsFs::new());

    FS.lock().insert("procfs".to_string(), procfs);
    FS.lock().insert("sysfs".to_string(), sysfs);
    FS.lock().insert("ramfs".to_string(), ramfs);
    FS.lock().insert("devfs".to_string(), devfs);
    FS.lock().insert("tmpfs".to_string(), tmpfs);
    FS.lock().insert("pipefs".to_string(), pipefs);
    FS.lock().insert("dbfs".to_string(), dbfs);

    #[cfg(feature = "fat")]
    let diskfs = Arc::new(DiskFs::new(CommonFsProviderImpl));
    #[cfg(feature = "ext")]
    let diskfs = Arc::new(DiskFs::new(
        lwext4_vfs::ExtFsType::Ext4,
        CommonFsProviderImpl,
    ));

    FS.lock().insert("diskfs".to_string(), diskfs);

    println!("register fs success");
}

/// Init the filesystem
pub fn init_filesystem() -> AlienResult<()> {
    // 在 devfs 扫描设备之前，先造好 DBFS2 的块设备（独立 RAMDISK），
    // 这样 scan_system_devices 才能注册 /dev/dbfs 节点。
    devices::init_dbfs_ramdisk();
    register_all_fs();
    let ramfs_root = ram::init_ramfs(FS.lock().index("ramfs").clone());
    let procfs = FS.lock().index("procfs").clone();
    let procfs_root = proc::init_procfs(procfs);
    let devfs_root = dev::init_devfs(FS.lock().index("devfs").clone());
    let sysfs_root = sys::init_sysfs(FS.lock().index("sysfs").clone());
    let tmpfs_root = FS
        .lock()
        .index("tmpfs")
        .clone()
        .i_mount(0, "/tmp", None, &[])?;

    pipefs::init_pipefs(FS.lock().index("pipefs").clone());

    let path = VfsPath::new(ramfs_root.clone(), ramfs_root.clone());
    path.join("proc")?.mount(procfs_root, 0)?;
    path.join("sys")?.mount(sysfs_root, 0)?;
    path.join("dev")?.mount(devfs_root, 0)?;
    path.join("tmp")?.mount(tmpfs_root.clone(), 0)?;

    let shm_ramfs = FS
        .lock()
        .index("ramfs")
        .clone()
        .i_mount(0, "/dev/shm", None, &[])?;
    path.join("dev/shm")?.mount(shm_ramfs, 0)?;

    let diskfs = FS.lock().index("diskfs").clone();
    let blk_inode = path
        .join("/dev/sda")?
        .open(None)
        .expect("open /dev/sda failed")
        .inode()?;

    let diskfs_root = diskfs.i_mount(0, "/tests", Some(blk_inode), &[])?;
    path.join("tests")?.mount(diskfs_root, 0)?;

    // DBFS2 自动挂载到 /dbfs，使用 /dev/dbfs 块设备作为存储后端（坐在块结构之上）。
    let dbfs = FS.lock().index("dbfs").clone();
    println!("[dbfs] step1: lookup /dev/dbfs ...");
    let dbfs_blk_inode = match path.join("/dev/dbfs") {
        Ok(p) => {
            println!("[dbfs] step2: /dev/dbfs FOUND");
            p.open(None).expect("open /dev/dbfs failed").inode()?
        }
        Err(e) => {
            println!("[dbfs] ERROR step2: /dev/dbfs NOT FOUND (devfs node missing)");
            return Err(e.into());
        }
    };
    println!("[dbfs] step3: got dev inode, i_mount ...");
    let dbfs_root = match dbfs.i_mount(0, "/dbfs", Some(dbfs_blk_inode), &[]) {
        Ok(r) => {
            println!("[dbfs] step4: i_mount OK");
            r
        }
        Err(e) => {
            println!("[dbfs] ERROR step4: i_mount FAILED (dbfs2 internal)");
            return Err(e.into());
        }
    };
    path.join("dbfs")?.mount(dbfs_root.clone(), 0)?;
    println!("[dbfs] step5: mounted at /dbfs");

    println!("mount fs success");

    vfscore::path::print_fs_tree(&mut VfsOutPut, ramfs_root.clone(), "".to_string(), false)
        .unwrap();

    initrd::populate_initrd(ramfs_root.clone())?;

    SYSTEM_ROOT_FS.call_once(|| ramfs_root);

    // DBFS2 自检（事务性 + 增删改查），仅在 `dbfs_selftest` feature 打开时编译。
    // 复用上面已挂载的 /dbfs 根 dentry：绝不重复 i_mount，否则块设备后端会重新格式化清空数据。
    #[cfg(feature = "dbfs_selftest")]
    dbfs_selftest::run(dbfs_root);
    #[cfg(not(feature = "dbfs_selftest"))]
    let _ = dbfs_root;

    println!("Init filesystem success");
    Ok(())
}

struct VfsOutPut;
impl core::fmt::Write for VfsOutPut {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        platform::console::console_write(s);
        Ok(())
    }
}

/// Get the root filesystem of the system
#[inline]
pub fn system_root_fs() -> Arc<dyn VfsDentry> {
    SYSTEM_ROOT_FS.get().unwrap().clone()
}

/// Get the filesystem by name
#[inline]
pub fn system_support_fs(fs_name: &str) -> Option<Arc<dyn VfsFsType>> {
    FS.lock().iter().find_map(|(name, fs)| {
        if name == fs_name {
            Some(fs.clone())
        } else {
            None
        }
    })
}

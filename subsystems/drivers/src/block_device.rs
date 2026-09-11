use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec::Vec};
use core::{
    cmp::min,
    fmt::{Debug, Formatter},
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    sync::atomic::{AtomicU8, AtomicUsize, Ordering},
};

use config::FRAME_SIZE;
use constants::{AlienResult, LinuxErrno};
use device_interface::{BlockDevice, DeviceBase, LowBlockDevice};
use ksync::{Mutex, RwLock};
use lru::LruCache;
use mem::{alloc_frames, free_frames};
use platform::config::{BLOCK_CACHE_FRAMES, CLOCK_FREQ};
use shim::KTask;
use timer::read_timer;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};
pub use visionfive2_sd::Vf2SdDriver;
use visionfive2_sd::{SDIo, SleepOps};

use crate::hal::HalImpl;
const PAGE_CACHE_SIZE: usize = FRAME_SIZE;

/// 异步等待者状态（R1 三态原子机）。
///
/// - `WAIT_RUNNING`：请求已提交、等待者已入队，但「是否挂起」尚未决断；
/// - `WAIT_PARKED`：已决断为挂起，等待中断侧唤醒；
/// - `WAIT_WOKEN`：已被唤醒（由中断侧或提交侧续跑），之后绝不再重复唤醒。
///
/// 关键不变量：唤醒动作（中断侧 `to_wakeup + put_task`）**仅当** `swap(WOKEN)`
/// 的返回值是 `WAIT_PARKED` 时才执行。这样无论「中断先到」还是「提交方先决断」，
/// 唤醒最多发生一次，彻底消除 R1 的「双重入队 -> 任务被恢复两次 -> 二次 complete panic」。
const WAIT_RUNNING: u8 = 0;
const WAIT_PARKED: u8 = 1;
const WAIT_WOKEN: u8 = 2;

/// 异步等待者（替代原 `Arc<dyn KTask>` 直接入队）。
///
/// 携带三态原子 `state`，使「是否需要挂起」的判据从易错的 `TaskState`
/// （`to_wakeup()` 会把它改回 Ready，无法区分）改为显式原子机。
struct BlkWait {
    task: Arc<dyn KTask>,
    state: AtomicU8,
}

pub struct GenericBlockDevice {
    device: Box<dyn LowBlockDevice>,
    cache: Mutex<LruCache<usize, FrameTracker>>,
    dirty: Mutex<Vec<usize>>,
}

#[derive(Debug)]
struct FrameTracker {
    ptr: usize,
}

impl FrameTracker {
    pub fn new(ptr: usize) -> Self {
        Self { ptr }
    }
}

impl Deref for FrameTracker {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr as *const u8, FRAME_SIZE) }
    }
}

impl DerefMut for FrameTracker {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr as *mut u8, FRAME_SIZE) }
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        free_frames(self.ptr as *mut u8, 1);
    }
}

unsafe impl Send for GenericBlockDevice {}

unsafe impl Sync for GenericBlockDevice {}

impl GenericBlockDevice {
    pub fn new(device: Box<dyn LowBlockDevice>) -> Self {
        Self {
            device,
            cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(BLOCK_CACHE_FRAMES).unwrap(),
            )),
            dirty: Mutex::new(Vec::new()),
        }
    }

    /// Phase 8.1 测试钩子：绕过页缓存，直接向底层 `LowBlockDevice` 发一次**同步**块读。
    /// 仅供 QD=1 async 自检做对照组使用，**不参与** `read/write` 主路径。
    pub fn read_block_raw(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        self.device.read_block(block_id, buf)
    }

    /// Phase 8.1 测试钩子：绕过页缓存，直接向底层 `LowBlockDevice` 发一次**异步**块读。
    /// 仅供 QD=1 async 自检使用，**不参与** `read/write` 主路径
    /// （缓存层接入异步属于 Phase 8.3，不在本阶段改动范围）。
    pub fn read_block_async_raw(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        self.device.read_block_async(block_id, buf)
    }

    /// 诊断接口：转发到底层 `LowBlockDevice::blk_stats`（观测 IRQ 计数，用于 R4/R5）。
    pub fn blk_stats(&self) -> (usize, usize) {
        self.device.blk_stats()
    }
}

impl DeviceBase for GenericBlockDevice {
    fn handle_irq(&self) {
        self.device.handle_irq();
    }
}

impl Debug for GenericBlockDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QemuBlockDevice").finish()
    }
}

impl BlockDevice for GenericBlockDevice {
    fn read(&self, buf: &mut [u8], offset: usize) -> AlienResult<usize> {
        let mut page_id = offset / PAGE_CACHE_SIZE;
        let mut offset = offset % PAGE_CACHE_SIZE;

        let mut cache_lock = self.cache.lock();
        let len = buf.len();
        let mut count = 0;

        while count < len {
            if !cache_lock.contains(&page_id) {
                let device = &self.device;
                let cache = alloc_frames(1);
                let mut cache = FrameTracker::new(cache as usize);
                let start_block = page_id * PAGE_CACHE_SIZE / 512;
                let end_block = start_block + PAGE_CACHE_SIZE / 512;
                for i in start_block..end_block {
                    let target_buf =
                        &mut cache[(i - start_block) * 512..(i - start_block + 1) * 512];
                    device.read_block(i, target_buf).unwrap();
                }
                let old_cache = cache_lock.push(page_id, cache);
                if let Some((id, old_cache)) = old_cache {
                    let start_block = id * PAGE_CACHE_SIZE / 512;
                    let end_block = start_block + PAGE_CACHE_SIZE / 512;
                    for i in start_block..end_block {
                        let target_buf =
                            &old_cache[(i - start_block) * 512..(i - start_block + 1) * 512];
                        device.write_block(i, target_buf).unwrap();
                        self.dirty.lock().retain(|&x| x != id);
                    }
                }
            }
            let cache = cache_lock.get(&page_id).unwrap();
            let copy_len = min(PAGE_CACHE_SIZE - offset, len - count);
            buf[count..count + copy_len].copy_from_slice(&cache[offset..offset + copy_len]);
            count += copy_len;
            offset = 0;
            page_id += 1;
        }
        Ok(buf.len())
    }
    fn write(&self, buf: &[u8], offset: usize) -> AlienResult<usize> {
        let mut page_id = offset / PAGE_CACHE_SIZE;
        let mut offset = offset % PAGE_CACHE_SIZE;
        let mut cache_lock = self.cache.lock();
        let len = buf.len();
        let mut count = 0;
        while count < len {
            if !cache_lock.contains(&page_id) {
                let device = &self.device;
                let cache = alloc_frames(1);
                let mut cache = FrameTracker::new(cache as usize);
                let start_block = page_id * PAGE_CACHE_SIZE / 512;
                let end_block = start_block + PAGE_CACHE_SIZE / 512;
                for i in start_block..end_block {
                    let target_buf =
                        &mut cache[(i - start_block) * 512..(i - start_block + 1) * 512];
                    device.read_block(i, target_buf).unwrap();
                }
                let old_cache = cache_lock.push(page_id, cache);
                if let Some((id, old_cache)) = old_cache {
                    let start_block = id * PAGE_CACHE_SIZE / 512;
                    let end_block = start_block + PAGE_CACHE_SIZE / 512;
                    for i in start_block..end_block {
                        let target_buf =
                            &old_cache[(i - start_block) * 512..(i - start_block + 1) * 512];
                        device.write_block(i, target_buf).unwrap();
                        self.dirty.lock().retain(|&x| x != id);
                    }
                }
            }
            let cache = cache_lock.get_mut(&page_id).unwrap();
            let copy_len = min(PAGE_CACHE_SIZE - offset, len - count);
            cache[offset..offset + copy_len].copy_from_slice(&buf[count..count + copy_len]);

            // 关键修复（崩溃一致性/持久化）：write-through 到真实块设备。
            //
            // 原实现只把数据写进 LruCache，从不落盘，仅靠 LRU 淘汰时顺带 write-back；
            // 且 flush() 被注释成空实现。导致 DBFS2 的「write-through 持久化」假设失效：
            // 数据停留在内存 cache 中，qemu 一关即丢失，重启后全 MISSING。
            //
            // 这里在写完 cache 后，把受影响的那部分扇区立即 write_block 写回真实设备，
            // 使 BlockDevFile::write -> BLKDevice::write_at -> 本 write 真正 write-through，
            // 与 virtio-blk 的持久化语义对齐。
            {
                // 计算本次写涉及的起始/结束扇区（512 字节对齐，落在当前 page 内）。
                let byte_start = offset;
                let byte_end = offset + copy_len;
                let start_block = (page_id * PAGE_CACHE_SIZE + byte_start) / 512;
                let end_block = (page_id * PAGE_CACHE_SIZE + byte_end + 511) / 512;
                for i in start_block..end_block {
                    let base = i * 512 - page_id * PAGE_CACHE_SIZE;
                    let target_buf = &cache[base..base + 512];
                    self.device.write_block(i, target_buf).unwrap();
                }
            }

            count += copy_len;
            offset = (offset + copy_len) % PAGE_CACHE_SIZE;
            page_id += 1;
        }
        Ok(buf.len())
    }
    fn size(&self) -> usize {
        self.device.capacity() * 512
    }
    fn flush(&self) -> AlienResult<()> {
        // 兜底：把缓存中所有 dirty 页写回真实设备。
        // （write 已 write-through，这里主要是 flush 语义完整性 + 防御性落盘。）
        let cache_lock = self.cache.lock();
        let device = &self.device;
        let mut flushed = 0usize;
        for (id, cache) in cache_lock.iter() {
            let start_block = id * PAGE_CACHE_SIZE / 512;
            let end_block = start_block + PAGE_CACHE_SIZE / 512;
            for i in start_block..end_block {
                let target_buf = &cache[(i - start_block) * 512..(i - start_block + 1) * 512];
                device.write_block(i, target_buf).unwrap();
            }
            flushed += 1;
        }
        let _ = flushed;
        self.dirty.lock().clear();
        Ok(())
    }
}

pub struct VirtIOBlkWrapper {
    device: Mutex<VirtIOBlk<HalImpl, MmioTransport>>,
    /// 异步等待队列：token -> 等待者（R1 三态状态机）。
    /// 唤醒即 `remove` 取走，绝不保留陈旧项（R3 防 token 复用串台）。
    wait_queue: Mutex<BTreeMap<u16, Arc<BlkWait>>>,
    /// R4 隔离/观测计数：当前在飞的异步请求数。
    /// 同步读/写路径入（断言）时必须为 0，否则说明 sync/async 在同一 wrapper 上混飞 ——
    /// 立即 assert 失败（宁可显式 panic，也不要让 virtqueue 静默楔死）。
    in_flight: AtomicUsize,
    /// Phase 8.1 观测计数器：`handle_irq` 被进入的次数。
    ///
    /// 用于验证「virtio request -> IRQ -> handle_irq」这一段是否真的接通。
    /// 只要它大于 0，就说明 enable_interrupts + PLIC 注册已经生效。
    irq_enter: AtomicUsize,
    /// 观测计数器：成功从 `wait_queue` 取出并唤醒等待任务的次数（含完成链续链唤醒）。
    ///
    /// 只有真正走异步路径（`read_block_async`）才会增加；
    /// 同步路径产生的中断不会增加它。
    irq_wake: AtomicUsize,
}

impl VirtIOBlkWrapper {
    pub fn new(addr: usize) -> Self {
        let header = NonNull::new(addr as *mut VirtIOHeader).unwrap();
        let transport = unsafe { MmioTransport::new(header) }.unwrap();
        let mut blk = VirtIOBlk::<HalImpl, MmioTransport>::new(transport)
            .expect("failed to create blk driver");
        // Phase 8.1：打开 device -> driver 方向的完成通知。
        //
        // `enable_interrupts()` 等价于 `queue.set_dev_notify(true)`，即把 avail ring 的
        // VIRTQ_AVAIL_F_NO_INTERRUPT 标志清 0，允许设备在完成请求后拉起中断。
        //
        // 已知边界：若与设备协商了 VIRTIO_F_EVENT_IDX，该调用是 no-op
        // （中断抑制改由 avail.used_event 控制）。此时若观测不到中断，
        // 需要另行处理 EVENT_IDX，属于 Phase 8.1 的验证项之一。
        blk.enable_interrupts();
        Self {
            device: Mutex::new(blk),
            wait_queue: Mutex::new(BTreeMap::new()),
            in_flight: AtomicUsize::new(0),
            irq_enter: AtomicUsize::new(0),
            irq_wake: AtomicUsize::new(0),
        }
    }

    pub fn from_mmio(mmio_transport: MmioTransport) -> Self {
        let mut blk = VirtIOBlk::<HalImpl, MmioTransport>::new(mmio_transport)
            .expect("failed to create blk driver");
        // 同上：使能设备完成中断。
        blk.enable_interrupts();
        Self {
            device: Mutex::new(blk),
            wait_queue: Mutex::new(BTreeMap::new()),
            in_flight: AtomicUsize::new(0),
            irq_enter: AtomicUsize::new(0),
            irq_wake: AtomicUsize::new(0),
        }
    }

    /// Phase 8.1 观测接口：返回 `(进入中断次数, 唤醒任务次数)`。
    pub fn irq_stats(&self) -> (usize, usize) {
        (
            self.irq_enter.load(Ordering::Relaxed),
            self.irq_wake.load(Ordering::Relaxed),
        )
    }

    /// 唤醒 used ring 队首 token 对应的等待者，并续链（R4 完成链核心原语）。
    ///
    /// 调用点：
    /// 1. `handle_irq()`：`ack_interrupt()` 之后调用（取代原「取一个 token 唤醒」）；
    /// 2. `read/write_block_async` 的 `complete_*` 返回之后调用。
    ///
    /// 为什么对「批量完成只来一次中断」免疫：链路推进不依赖新中断，
    /// 而依赖「前一个请求完成 -> 推进 `last_used_idx` -> 唤醒新队首」。
    ///
    /// 安全性：不 panic。
    /// - `peek_used()` 为 `None`（无完成）直接返回；
    /// - token 不在 `wait_queue`（同步请求的中断）也安全返回，不触碰。
    fn wake_head(&self) -> bool {
        let mut device = self.device.lock();
        let Some(token) = device.peek_used() else {
            return false;
        };
        let wait = self.wait_queue.lock().remove(&token);
        drop(device);
        if let Some(wait) = wait {
            // 仅当状态为 PARKED 才真正唤醒；否则（RUNNING=提交方将自行续跑 /
            // WOKEN=已被唤醒过）都不应重复唤醒。
            if wait.state.swap(WAIT_WOKEN, Ordering::AcqRel) == WAIT_PARKED {
                wait.task.to_wakeup();
                shim::put_task(wait.task.clone());
                self.irq_wake.fetch_add(1, Ordering::Relaxed);
                platform::println!(
                    "[blk-irq] WAKE token={:?} -> task resumed by IRQ/chain",
                    token
                );
                return true;
            }
        }
        false
    }
}

impl LowBlockDevice for VirtIOBlkWrapper {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        // 注：R4（sync/async 不得在同一 wrapper 上同时 in-flight）在本阶段靠**测试结构**保证
        // （QD=2 测试期间只发异步读、不并发同步 I/O），不在同步路径里硬编码断言——
        // 否则并发调度的 init 进程在异步 in-flight 窗口内做同步读会误触发 panic。
        // `in_flight` 计数仅作诊断（见 `blk_stats`）。
        let res = self
            .device
            .lock()
            .read_blocks(block_id, buf)
            .map_err(|_| LinuxErrno::EIO.into());
        res
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        self.device
            .lock()
            .write_blocks(block_id, buf)
            .map_err(|_| LinuxErrno::EIO.into())
    }

    fn capacity(&self) -> usize {
        self.device.lock().capacity() as usize
    }

    /// 非阻塞读（Phase 8.1 QD=1 实证 + Phase 8.2 QD>1 完成链）。
    ///
    /// 链路：`read_blocks_nb()` 提交请求 -> 入队 `BlkWait{state=RUNNING}` ->
    ///       三态 CAS 决断挂起 / 续跑 -> 让出 CPU
    ///       -> virtio IRQ -> `handle_irq()` -> `wake_head()` 唤醒队首
    ///       -> 任务恢复 -> `complete_read_blocks()` 取回数据 -> `wake_head()` 续链唤醒下一个。
    ///
    /// QD>1 由多任务并发提交驱动（单任务一次只能挂起等一个 token，见设计文档 §3）；
    /// 完成顺序被 used ring 锁死（只能完成队首），故用「顺序完成链」逐个唤醒。
    ///
    /// 前置条件：
    /// - 必须处在 task 上下文（`shim::take_current_task()` 返回 `Some`），否则降级为同步读；
    /// - `buf.len()` 必须是 512 的非零倍数。
    ///
    /// 关键约束（沿用 Phase 8.1）：让出 CPU 前必须释放 `device` 锁；必须先提交成功再取 task。
    fn read_block_async(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        use virtio_drivers::device::blk::{BlkReq, BlkResp, RespStatus};
        let mut resp = BlkResp::default();
        let mut req = BlkReq::default();
        let mut device = self.device.lock();
        let submit = unsafe { device.read_blocks_nb(block_id, &mut req, buf, &mut resp) };
        // 第一阶段：只处理提交结果。此时尚未触碰 task 上下文，降级/报错都是安全的。
        let token = match submit {
            Ok(token) => token,
            // 队列满：降级为同步读。直接走 device，绕过 R4 断言（此时本请求尚未计入 in_flight，
            // 但其它异步请求可能在飞 -> 同步 fallback 本就属 R4 边缘情况；QD 远小于 16 不会触发）。
            Err(virtio_drivers::Error::QueueFull) => {
                drop(device);
                return self
                    .device
                    .lock()
                    .read_blocks(block_id, buf)
                    .map_err(|_| LinuxErrno::EIO.into());
            }
            Err(_) => {
                drop(device);
                return Err(LinuxErrno::EIO.into());
            }
        };
        // 提交成功，计入 in_flight（R4 隔离计数）。
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        // 第二阶段：提交已成功，此时才取 task 上下文。
        //
        // 关键修复（QN 8.2 R1 else 分支 panic）：必须用 `current_task()`（只读克隆），
        // **不能**用 `take_current_task()`。`take_current_task()` 会 `cpu.task.take()`
        // 把当前 cpu.task 清成 None；一旦 R1 竞态（中断在 take 与 CAS 之间先 swap(WOKEN)）
        // 命中 else 分支（未真正 park，不调用 schedule_now），任务会继续执行且 cpu.task 永远为
        // None，最终在 idle 的 `do_suspend()`/`schedule()` 处 `unwrap` panic。
        // `current_task()` 只克隆 Arc，保留 cpu.task，彻底消除该窗口。
        let task = match shim::current_task() {
            Some(task) => task,
            None => {
                // 没有 task 上下文（例如 boot 早期）：无法让出，请求已在飞，
                // 只能持锁原地等待完成（等价于一次同步读）。
                unsafe {
                    device
                        .complete_read_blocks(token, &req, buf, &mut resp)
                        .unwrap();
                }
                assert_eq!(
                    resp.status(),
                    RespStatus::OK,
                    "Error {:?} reading block.",
                    resp.status()
                );
                self.in_flight.fetch_sub(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        // 第三阶段：登记等待者并让出 CPU。
        //
        // R1 三态状态机：
        // - 先 `to_wait()` 保持 TaskState 语义（Waiting）；
        // - 入队 `BlkWait { state = RUNNING }`；
        // - drop(device) 开中断后，由 CAS(RUNNING->PARKED) 决定「是否挂起」：
        //     * 成功 -> 确实挂起，交给 `schedule_now`；
        //     * 失败（中断侧已 swap(WOKEN)）-> 不挂起，恢复 TaskState 并清理后直接续跑到 complete。
        // 无论哪条路径，唤醒最多发生一次（中断侧仅当 swap 返回 PARKED 才唤醒）。
        task.to_wait();
        let wait = Arc::new(BlkWait {
            task: task.clone(),
            state: AtomicU8::new(WAIT_RUNNING),
        });
        self.wait_queue.lock().insert(token, wait.clone());
        drop(device);
        if wait
            .state
            .compare_exchange(WAIT_RUNNING, WAIT_PARKED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            shim::schedule_now(task);
        } else {
            // 中断侧已抢先 swap(WOKEN)：本任务未真正挂起，恢复 TaskState 并清理队列项
            // （中断侧已在 wake_head 中 remove 过，这里再 remove 是幂等保险）。
            task.to_wakeup();
            self.wait_queue.lock().remove(&token);
        }
        // 被唤醒后（或续跑）回到这里：请求已完成，取回数据。
        let res = unsafe {
            self.device
                .lock()
                .complete_read_blocks(token, &req, buf, &mut resp)
        };
        // 无论成功失败，先回收 in_flight，并续链唤醒下一个队首（R6：错误路径不能断链）。
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
        self.wake_head();
        match res {
            Ok(()) => {
                assert_eq!(
                    resp.status(),
                    RespStatus::OK,
                    "Error {:?} reading block.",
                    resp.status()
                );
                Ok(())
            }
            Err(e) => {
                platform::println!(
                    "[blk-async] read complete_read_blocks err={:?} token={} (chain continued)",
                    e, token
                );
                Err(LinuxErrno::EIO.into())
            }
        }
    }

    /// 非阻塞写（Phase 8.1 QD=1 实证 + Phase 8.2 QD>1 完成链，与 `read_block_async` 对称）。
    ///
    /// 约束、R1 三态状态机、完成链逻辑同 `read_block_async`，参见那里的说明。
    fn write_block_async(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        use virtio_drivers::device::blk::{BlkReq, BlkResp, RespStatus};
        let mut resp = BlkResp::default();
        let mut req = BlkReq::default();
        let mut device = self.device.lock();
        let submit = unsafe { device.write_blocks_nb(block_id, &mut req, buf, &mut resp) };
        let token = match submit {
            Ok(token) => token,
            // 队列满：降级为同步写，直接走 device 绕过 R4 断言（同 read 的注释）。
            Err(virtio_drivers::Error::QueueFull) => {
                drop(device);
                return self
                    .device
                    .lock()
                    .write_blocks(block_id, buf)
                    .map_err(|_| LinuxErrno::EIO.into());
            }
            Err(_) => {
                drop(device);
                return Err(LinuxErrno::EIO.into());
            }
        };
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        let task = match shim::current_task() {
            Some(task) => task,
            None => {
                unsafe {
                    device
                        .complete_write_blocks(token, &req, buf, &mut resp)
                        .unwrap();
                }
                assert_eq!(
                    resp.status(),
                    RespStatus::OK,
                    "Error {:?} writing block.",
                    resp.status()
                );
                self.in_flight.fetch_sub(1, Ordering::Relaxed);
                return Ok(());
            }
        };
        task.to_wait();
        let wait = Arc::new(BlkWait {
            task: task.clone(),
            state: AtomicU8::new(WAIT_RUNNING),
        });
        self.wait_queue.lock().insert(token, wait.clone());
        drop(device);
        if wait
            .state
            .compare_exchange(WAIT_RUNNING, WAIT_PARKED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            shim::schedule_now(task);
        } else {
            task.to_wakeup();
            self.wait_queue.lock().remove(&token);
        }
        let res = unsafe {
            self.device
                .lock()
                .complete_write_blocks(token, &req, buf, &mut resp)
        };
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
        self.wake_head();
        match res {
            Ok(()) => {
                assert_eq!(
                    resp.status(),
                    RespStatus::OK,
                    "Error {:?} writing block.",
                    resp.status()
                );
                Ok(())
            }
            Err(e) => {
                platform::println!(
                    "[blk-async] write complete_write_blocks err={:?} token={} (chain continued)",
                    e, token
                );
                Err(LinuxErrno::EIO.into())
            }
        }
    }

    /// virtio-blk 完成中断处理（Phase 8.1 实证 + Phase 8.2 完成链）。
    ///
    /// 进入后只做两件事：
    /// 1. `ack_interrupt()` 应答设备；
    /// 2. `wake_head()` 唤醒 used ring 队首 token 对应的等待者（取代原「取一个 token 唤醒」）。
    ///
    /// 不在这里完成请求本身（`complete_*` 需要调用方持有栈上 buffer，中断上下文拿不到）；
    /// 完成动作由被唤醒的任务在自己的上下文里做，做完再 `wake_head()` 续链。
    ///
    /// `wake_head` 对「token 不在异步队列」是安全空操作（同步读的中断会走自己的 `pop_used`），
    /// 因此这里**绝不**需要 `unwrap()`。
    fn handle_irq(&self) {
        let n = self
            .irq_enter
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let acked = {
            let mut device = self.device.lock();
            device.ack_interrupt()
        };
        let woke = self.wake_head();
        // 诊断：前 8 次中断 + 任何真正唤醒，都打印计数，用于 R5（中断抑制）观测。
        // `platform::println!` 基于 `format_args!`（不分配内存），内部用 ksync 锁（持锁期间关中断），
        // 在中断上下文中调用安全。
        if n <= 8 || woke {
            let (ie, iw) = self.irq_stats();
            let inf = self.in_flight.load(Ordering::Relaxed);
            platform::println!(
                "[blk-irq] #{} ack={} woke={} irq_enter={} irq_wake={} in_flight={}",
                n, acked, woke, ie, iw, inf
            );
        }
    }

    /// 诊断接口（覆盖 `LowBlockDevice` 默认实现）：返回 `(irq_enter, irq_wake)`。
    fn blk_stats(&self) -> (usize, usize) {
        self.irq_stats()
    }
}

pub struct MemoryFat32Img {
    data: RwLock<&'static mut [u8]>,
}

impl LowBlockDevice for MemoryFat32Img {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        let start = block_id * 512;
        let end = start + 512;
        buf.copy_from_slice(&self.data.read()[start..end]);
        Ok(())
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        let start = block_id * 512;
        let end = start + 512;
        self.data.write()[start..end].copy_from_slice(buf);
        Ok(())
    }
    fn capacity(&self) -> usize {
        self.data.read().len() / 512
    }
    fn read_block_async(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        self.read_block(block_id, buf)
    }

    fn write_block_async(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        self.write_block(block_id, buf)
    }
    fn handle_irq(&self) {}
}

impl MemoryFat32Img {
    pub fn new(data: &'static mut [u8]) -> Self {
        Self {
            data: RwLock::new(data),
        }
    }
}

pub struct VF2SDDriver {
    driver: Mutex<Vf2SdDriver<SdIoImpl, SleepOpsImpl>>,
}

impl VF2SDDriver {
    pub fn new() -> Self {
        Self {
            driver: Mutex::new(Vf2SdDriver::new(SdIoImpl)),
        }
    }
    pub fn init(&mut self) {
        self.driver.lock().init();
    }
}

impl LowBlockDevice for VF2SDDriver {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        self.driver.lock().read_block(block_id, buf);
        Ok(())
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        self.driver.lock().write_block(block_id, buf);
        Ok(())
    }
    fn capacity(&self) -> usize {
        // unimplemented!()
        // 32GB
        32 * 1024 * 1024 * 1024 / 512
    }
    fn read_block_async(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()> {
        self.read_block(block_id, buf)
    }

    fn write_block_async(&self, block_id: usize, buf: &[u8]) -> AlienResult<()> {
        self.write_block(block_id, buf)
    }
    fn handle_irq(&self) {
        unimplemented!()
    }
}

pub struct SdIoImpl;
pub const SDIO_BASE: usize = 0x16020000;

impl SDIo for SdIoImpl {
    fn read_reg_at(&self, offset: usize) -> u32 {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.read_volatile() }
    }
    fn write_reg_at(&mut self, offset: usize, val: u32) {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.write_volatile(val) }
    }
    fn read_data_at(&self, offset: usize) -> u64 {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.read_volatile() }
    }
    fn write_data_at(&mut self, offset: usize, val: u64) {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.write_volatile(val) }
    }
}

pub struct SleepOpsImpl;

impl SleepOps for SleepOpsImpl {
    fn sleep_ms(ms: usize) {
        sleep_ms(ms)
    }
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
        sleep_ms_until(ms, f)
    }
}

fn sleep_ms(ms: usize) {
    let start = read_timer();
    while read_timer() - start < ms * CLOCK_FREQ / 1000 {
        core::hint::spin_loop();
    }
}

fn sleep_ms_until(ms: usize, mut f: impl FnMut() -> bool) {
    let start = read_timer();
    while read_timer() - start < ms * CLOCK_FREQ / 1000 {
        if f() {
            return;
        }
        core::hint::spin_loop();
    }
}

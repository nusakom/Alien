//! BlockDevice 自检套件（Phase 8.1 QD=1 异步 + Step ③ 同步正确性矩阵）。
//!
//! 本模块整体由 `blk_async_test` feature 门控（默认关闭），挂在 `kthread_init`，
//! 必须在 **task 上下文**运行。
//!
//! 两个子测试职责分离：
//! - `sync_correctness()`（Step ③）：只验证**同步**块设备正确性。只读不写、不碰 async/QD>1。
//!   复用 `read_block_raw`（绕过页缓存、直达 virtio-blk 同步读）+ `BlockDevice::read`（多块路径）。
//! - `qd1_async_selftest()`（Phase 8.1）：QD=1 异步闭环自检，已实证，留作回归。
//!
//! 两者都走 `blk_async_test` feature 钩子，互不干扰。

// `devices` 内部的 `mod block` 是私有模块，只对外再导出了符号，
// 因此这里必须用 `devices::BLOCK_DEVICE` 而不是 `devices::block::BLOCK_DEVICE`。
use devices::BLOCK_DEVICE;
use device_interface::BlockDevice;
use platform::config::CLOCK_FREQ;
use platform::println;
use timer::read_timer;

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::task::kthread::ktread_create;
use crate::task::do_suspend;
use drivers::block_device::GenericBlockDevice;

/// 读哪一个块。512B/块，取 2MB 偏移处（FAT32 数据区，内容稳定）。
/// 只读不写，因此选块无需担心破坏文件系统。
const TEST_BLOCK_ID: usize = 4096;

/// 32-bit FNV-1a 校验和（no_std 可实现的极小哈希）。
///
/// 仅供 T6 主机交叉校验使用（本步 T6 暂不实施，仅打印供后续核对）：
/// mac 侧对 `tools/sdcard.img` 同一偏移用同算法重算，逐块比对即可证明
/// 「同步块设备返回的字节与后端镜像逐字节一致」。
fn fnv1a32(buf: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in buf {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

// ===== QD=N 并发异步自检（Phase 8.2 QD>1 完成链）=====
//
// 由 `QD_N` 个 kthread 各提交一个异步读并挂起，彼此并发在飞（QD=N）。
// 完成链（`wake_head` 顺序完成 + 链式续唤醒）把它们逐个拉起。验证目标（用户给定）：
//   QD=N 能稳定完成 N 个并发异步请求，全部任务被正确唤醒，
//   数据与 Sync 基线逐字节一致，无 WrongToken / 重复唤醒 / 死锁 / virtqueue wedge。
//
// R4 隔离由测试结构保证：自检构建下不启动 INIT（见 `task::init_task`），
// 本测试期间**只发异步读**，不并发同步 I/O；各 worker 读不同块，token 互不干扰。
// QD 矩阵 = 1/2/4/8/16（无 32）；逐级扩大验证。单 worker 块号 = QD_BLOCK_BASE + idx。
const QD_N: usize = 2; // 默认闭环级别（QD matrix 1/2/4/8/16 已全部 PASS，见 docs/step82-*）。逐级扩大验证：2->4->8->16。
const QD_BLOCK_BASE: usize = 8192;
fn qd_block_id(i: usize) -> usize {
    QD_BLOCK_BASE + i
}

static mut QD_BUF: [[u8; 512]; QD_N] = [[0u8; 512]; QD_N];
/// 同步基线：主线程 spawn 前对各块各做一次同步读，存这里供最后完成的 worker 比对。
static mut QD_SYNC: [[u8; 512]; QD_N] = [[0u8; 512]; QD_N];
static mut QD_OK: [bool; QD_N] = [false; QD_N];
static QD_DONE: AtomicUsize = AtomicUsize::new(0);
/// worker 领取自身编号的原子计数器（避免为每个 QD 写 N 个 wrapper 函数）。
static QD_NEXT_IDX: AtomicUsize = AtomicUsize::new(0);

/// 打印 QD=N 最终结论（由最后完成的 worker 调用）。
///
/// 判据（用户给定）：
/// - 所有 worker 的异步读结果必须与同步基线 **逐字节一致**；
/// - `irq_enter > 0` 证明「设备完成 -> 中断 -> handle_irq」链路已接通（否则说明异步降级成同步读）；
/// - `done == QD_N` 证明 N 个并发请求全部完成、全部被正确唤醒。
fn print_qd_result(device: &Arc<GenericBlockDevice>) {
    let (ie, iw) = device.blk_stats();
    let mut all_identical = true;
    for i in 0..QD_N {
        let ok = unsafe { QD_OK[i] && QD_BUF[i] == QD_SYNC[i] };
        if !ok {
            all_identical = false;
        }
    }
    println!(
        "[blk-qd{}] all {} workers async==sync: {} (irq_enter={} irq_wake={} done={}/{})",
        QD_N,
        QD_N,
        all_identical,
        ie,
        iw,
        QD_DONE.load(Ordering::SeqCst),
        QD_N
    );
    if all_identical && ie > 0 && QD_DONE.load(Ordering::SeqCst) == QD_N {
        println!(
            "[blk-qd{}] ==== RESULT: PASS (QD={} concurrent async, all {} woken, byte-identical, no wedge) ====",
            QD_N, QD_N, QD_N
        );
    } else {
        println!(
            "[blk-qd{}] ==== RESULT: FAIL (all_identical={} irq_enter={} done={}/{}) ====",
            QD_N, all_identical, ie, QD_DONE.load(Ordering::SeqCst), QD_N
        );
    }
    println!("[blk-qd{}] ==== QD={} concurrent async selftest end ====", QD_N, QD_N);
}

/// 工作者：领取唯一编号 `idx`，对 `qd_block_id(idx)` 做一次异步读，结果存静态缓冲。
///
/// 任务**不得返回**（Context 无返回地址 -> UB），完成后进入安全空闲循环（`do_suspend`）。
///
/// 最后完成的 worker（`fetch_add(1) == QD_N-1`）负责打印最终结论。
fn qd_worker() {
    let idx = QD_NEXT_IDX.fetch_add(1, Ordering::SeqCst);
    if idx >= QD_N {
        return;
    }
    let device = match BLOCK_DEVICE.get() {
        Some(d) => d.clone(),
        None => return,
    };
    let mut buf = [0u8; 512];
    let r = device.read_block_async_raw(qd_block_id(idx), &mut buf);
    unsafe {
        QD_BUF[idx].copy_from_slice(&buf);
        QD_OK[idx] = r.is_ok();
    }
    // 递增完成计数；返回值是「本 worker 之前已完成的个数」。
    // prev + 1 == QD_N => 我是最后一个完成者 => 由我打印结论。
    let prev = QD_DONE.fetch_add(1, Ordering::SeqCst);
    if prev + 1 == QD_N {
        print_qd_result(&device);
    }
    // 安全空闲：直接 `schedule()` 在某些调度路径下会触发 `cpu.task` 为 None 的 panic；
    // 改走 OS 既有的安全空闲原语 `do_suspend()`（与 kthread_init 的 `loop { do_suspend() }` 同款）。
    loop {
        do_suspend();
    }
}

/// QD=N 并发异步闭环自检。
///
/// 设计（用户给定、规避 `schedule()` panic + R4 隔离）：
/// 1. 主线程对各块做一次同步读作为基线（存 `QD_SYNC`）；
/// 2. spawn `QD_N` 个 kthread（各自提交一个异步读，QD=N 并发在飞）；
/// 3. **主线程直接返回**，由 `kthread_init` 的 `loop { do_suspend(); }` 承担安全空闲；
/// 4. 最后完成的 worker 打印终极结论。
pub fn qd_selftest() {
    println!("[blk-qd{}] ==== QD={} concurrent async selftest begin ====", QD_N, QD_N);
    let device = match BLOCK_DEVICE.get() {
        Some(d) => d.clone(),
        None => {
            println!("[blk-qd{}] BLOCK_DEVICE not initialized -> SKIP", QD_N);
            return;
        }
    };
    // Sync 基线：对各块各做一次同步读，存入 `QD_SYNC`。
    let mut all_ok = true;
    for i in 0..QD_N {
        let mut b = [0u8; 512];
        let ok = device.read_block_raw(qd_block_id(i), &mut b).is_ok();
        if !ok {
            all_ok = false;
        }
        unsafe {
            QD_SYNC[i].copy_from_slice(&b);
        }
    }
    if !all_ok {
        println!("[blk-qd{}] sync baseline read failed -> ABORT", QD_N);
        return;
    }
    unsafe {
        QD_OK = [false; QD_N];
    }
    QD_DONE.store(0, Ordering::SeqCst);
    QD_NEXT_IDX.store(0, Ordering::SeqCst);

    // 启动 QD_N 个并发 kthread，各自提交一个异步读（QD=N 并发在飞）。
    for i in 0..QD_N {
        if let Err(e) = ktread_create(qd_worker, "qdw") {
            println!("[blk-qd{}] spawn worker {} failed: {:?} -> ABORT", QD_N, i, e);
            return;
        }
    }
    // 不忙等：直接返回，交给 kthread_init 的安全空闲循环。最终结论由最后完成的 worker 打印。
    println!(
        "[blk-qd{}] spawned {} async workers (QD={} in flight); returning to idle, verdict by last finisher.",
        QD_N, QD_N, QD_N
    );
}

pub fn run() {
    // Step ③：先跑同步正确性矩阵（只读不写，不碰 async）。
    sync_correctness();
    // Step ④：同步块设备基准（SYNC BASELINE），只读不写。
    sync_benchmark();
    // Phase 8.1：QD=1 异步闭环自检（已实证，留作回归）。
    qd1_async_selftest();
    // Phase 8.2 QD>1 完成链：QD=N（由 `QD_N` 常量控制，逐级 2->4->8->16 扩大验证）。
    // QD matrix 1/2/4/8/16 已全部 PASS（见 docs/step82-qd*-qemu-bootlog.txt）。
    qd_selftest();
}

/// Step ③：同步块设备正确性矩阵（T1..T5）。
///
/// 设计约束（刻意最小化，遵守 Phase 8.2 两条硬约束）：
/// - **只验证同步路径**：不调用 `read_block_async` / `write_block_async` / 不碰 `handle_irq`；
/// - **只读不写**：只用 `read_block_raw` + `BlockDevice::read`，不调用 `write_block` / `flush`；
/// - **不构造 sync+async 并发**：因此 Phase 8.2-R4（virtqueue 混合楔死）在本步天然不触发。
pub fn sync_correctness() {
    println!("[blk-sync] ==== Sync BlockDevice correctness matrix begin ====");

    let device = match BLOCK_DEVICE.get() {
        Some(device) => device.clone(),
        None => {
            println!("[blk-sync] BLOCK_DEVICE not initialized -> SKIP");
            return;
        }
    };

    // 块设备容量（块数）。`size()` 返回字节数，除以 512 得块数。
    let cap = device.size() / 512;
    println!(
        "[blk-sync] capacity = {} blocks ({} bytes), TEST_BLOCK_ID = {}",
        cap,
        device.size(),
        TEST_BLOCK_ID
    );

    let mut passed = 0u32;
    let mut failed = 0u32;

    // ---- T1：单块 + 引导扇区签名 ----
    // 读 block 0，检查 FAT/MBR 引导签名 0x55AA @ [510..512]。
    // 这是对「offset 0 字节精确、未错位」的最强单点证据。
    {
        let mut buf = [0u8; 512];
        match device.read_block_raw(0, &mut buf) {
            Ok(()) => {
                let sig_ok = buf[510] == 0x55 && buf[511] == 0xAA;
                println!(
                    "[blk-sync] T1 block0 [510..512] = {:02x}{:02x} (expect 55aa) -> {}",
                    buf[510],
                    buf[511],
                    if sig_ok { "PASS" } else { "FAIL" }
                );
                if sig_ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            Err(e) => {
                println!("[blk-sync] T1 FAIL: read block0 err {:?}", e);
                failed += 1;
            }
        }
        // 打印校验和，供后续 T6 主机交叉校验使用（本步不比对）。
        let mut b0 = [0u8; 512];
        if device.read_block_raw(0, &mut b0).is_ok() {
            println!("[blk-sync] fnv block=0 {:08x} (for deferred T6)", fnv1a32(&b0));
        }
    }

    // ---- T2：读确定性 ----
    // 同一块读两次，必须逐字节相等（同步读可复现，无在飞状态污染 / 随机垃圾）。
    {
        let mut a = [0u8; 512];
        let mut b = [0u8; 512];
        let r1 = device.read_block_raw(TEST_BLOCK_ID, &mut a);
        let r2 = device.read_block_raw(TEST_BLOCK_ID, &mut b);
        match (r1, r2) {
            (Ok(()), Ok(())) => {
                let ok = a == b;
                println!(
                    "[blk-sync] T2 block{} read twice -> {}",
                    TEST_BLOCK_ID,
                    if ok { "PASS (identical)" } else { "FAIL (differs)" }
                );
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            _ => {
                println!("[blk-sync] T2 FAIL: read err {:?}/{:?}", r1, r2);
                failed += 1;
            }
        }
        println!(
            "[blk-sync] fnv block={} {:08x} (for deferred T6)",
            TEST_BLOCK_ID,
            fnv1a32(&a)
        );
    }

    // ---- T3：多块顺序 ----
    // 单块读 block 4096 / 4097，再用 `BlockDevice::read` 跨 2 块读 2048 字节，
    // 校验 big[0..512]==block4096 且 big[512..1024]==block4097
    // → 多块读路径返回顺序正确、未串块。
    {
        let mut k = [0u8; 512];
        let mut k1 = [0u8; 512];
        let rk = device.read_block_raw(TEST_BLOCK_ID, &mut k);
        let rk1 = device.read_block_raw(TEST_BLOCK_ID + 1, &mut k1);
        let mut big = [0u8; 2048];
        let rr = device.read(&mut big, TEST_BLOCK_ID * 512);
        match (rk, rk1, rr) {
            (Ok(()), Ok(()), Ok(_)) => {
                let ok = &big[0..512] == &k[..] && &big[512..1024] == &k1[..];
                println!(
                    "[blk-sync] T3 multi-block read ordering (block{}|{}) -> {}",
                    TEST_BLOCK_ID,
                    TEST_BLOCK_ID + 1,
                    if ok { "PASS" } else { "FAIL (mis-ordered)" }
                );
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
            }
            _ => {
                println!("[blk-sync] T3 FAIL: read err {:?}/{:?}/{:?}", rk, rk1, rr);
                failed += 1;
            }
        }
        println!(
            "[blk-sync] fnv block={} {:08x} (for deferred T6)",
            TEST_BLOCK_ID + 1,
            fnv1a32(&k1)
        );
    }

    // ---- T4：首 / 末块边界 ----
    // block 0（首）与 block cap-1（末）都应可读、不 panic / 不挂死。
    {
        let mut first = [0u8; 512];
        let mut last = [0u8; 512];
        let rf = device.read_block_raw(0, &mut first);
        // cap 恒 > 0（真实设备必有容量）；cap==0 为理论不可能，读 block0 占位避免 underflow。
        let last_id = if cap > 0 { cap - 1 } else { 0 };
        let rl = device.read_block_raw(last_id, &mut last);
        if rf.is_ok() && rl.is_ok() {
            println!(
                "[blk-sync] T4 first(block0) & last(block{}) both readable -> PASS",
                last_id
            );
            passed += 1;
        } else {
            println!("[blk-sync] T4 FAIL: first={:?} last={:?}", rf, rl);
            failed += 1;
        }
        if rl.is_ok() {
            println!(
                "[blk-sync] fnv block={} {:08x} (for deferred T6)",
                last_id,
                fnv1a32(&last)
            );
        }
    }

    // ---- T5：非法块（越界） ----
    // block cap（恰好越界一个）与 block cap+1000 都应返回 Err（或至少不 panic、
    // 控制流正常返回）。若返回 Ok，则是「越界未拦截」的发现，记 FAIL 但继续。
    {
        let mut buf = [0u8; 512];
        let r1 = device.read_block_raw(cap, &mut buf);
        let r2 = device.read_block_raw(cap + 1000, &mut buf);
        let ok1 = r1.is_ok();
        let ok2 = r2.is_ok();
        if r1.is_err() && r2.is_err() {
            println!(
                "[blk-sync] T5 out-of-range reads (block{} / block{}) -> PASS (returned Err)",
                cap,
                cap + 1000
            );
            passed += 1;
        } else {
            println!(
                "[blk-sync] T5 WARN: out-of-range reads returned Ok (block{}={}, block{}={}) \
                 -> 越界未拦截（非 panic，记录为发现）",
                cap, ok1, cap + 1000, ok2
            );
            failed += 1;
        }
    }

    // ---- 汇总 ----
    if failed == 0 {
        println!(
            "[blk-sync] ==== RESULT: PASS (T1..T5 all passed, {} checks) ====",
            passed
        );
    } else {
        println!(
            "[blk-sync] ==== RESULT: FAIL (passed={} failed={}) ====",
            passed, failed
        );
    }
    println!("[blk-sync] (T6 host cross-check deferred per request; fnv sums above for later use)");
    println!("[blk-sync] ==== Sync BlockDevice correctness matrix end ====");
}

/// Step ④：同步块设备基准测试（SYNC BASELINE）。
///
/// 只读不写，全部走 `read_block_raw`（绕过页缓存、直达 virtio-blk 同步读）。
/// 三项指标均与后续「Sync vs Async」对比口径对齐：
/// - 单块读延迟（平均，微秒）；
/// - 顺序读吞吐（KiB/s）；
/// - 随机读吞吐（KiB/s，LCG 伪随机块号）。
///
/// 计时用 `timer::read_timer()`（ticks），按 `CLOCK_FREQ`(Hz) 换算；全程整数运算，
/// 不依赖 no_std 浮点格式化。吞吐 / 延迟数值即为后续异步基准对照的 SYNC BASELINE。
pub fn sync_benchmark() {
    println!("[blk-bench] ==== Sync BlockDevice benchmark begin ====");
    let device = match BLOCK_DEVICE.get() {
        Some(d) => d.clone(),
        None => {
            println!("[blk-bench] BLOCK_DEVICE not initialized -> SKIP");
            return;
        }
    };
    let cap = device.size() / 512;
    let mut buf = [0u8; 512];

    // --- 单块读延迟（平均 over LAT_N 次）---
    const LAT_N: usize = 300;
    let t0 = read_timer() as u64;
    for _ in 0..LAT_N {
        let _ = device.read_block_raw(4096, &mut buf);
    }
    let t1 = read_timer() as u64;
    let lat_us = (t1 - t0) * 1_000_000 / CLOCK_FREQ as u64 / LAT_N as u64;
    println!(
        "[blk-bench] single-block read latency: avg={} us (over {} reads)",
        lat_us, LAT_N
    );

    // --- 顺序读吞吐 ---
    const SEQ_BLOCKS: usize = 2048; // 1 MiB
    let seq_base = 8192u64;
    let t0 = read_timer() as u64;
    for i in 0..SEQ_BLOCKS {
        let _ = device.read_block_raw((seq_base + i as u64) as usize, &mut buf);
    }
    let t1 = read_timer() as u64;
    let seq_ticks = t1 - t0;
    let seq_bytes = SEQ_BLOCKS * 512;
    let seq_ms = seq_ticks * 1000 / CLOCK_FREQ as u64;
    let seq_kib_s = (seq_bytes as u64) * CLOCK_FREQ as u64 / seq_ticks / 1024;
    println!(
        "[blk-bench] sequential read: {} blocks ({} KiB) in {} ms -> {} KiB/s",
        SEQ_BLOCKS,
        seq_bytes / 1024,
        seq_ms,
        seq_kib_s
    );

    // --- 随机读吞吐（LCG 伪随机块号）---
    const RAND_BLOCKS: usize = 2048;
    let mut seed: u64 = 0x1234_5678_9abc_def0;
    let t0 = read_timer() as u64;
    for _ in 0..RAND_BLOCKS {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let blk = ((seed >> 20) % cap as u64) as usize;
        let _ = device.read_block_raw(blk, &mut buf);
    }
    let t1 = read_timer() as u64;
    let rnd_ticks = t1 - t0;
    let rnd_bytes = RAND_BLOCKS * 512;
    let rnd_ms = rnd_ticks * 1000 / CLOCK_FREQ as u64;
    let rnd_kib_s = (rnd_bytes as u64) * CLOCK_FREQ as u64 / rnd_ticks / 1024;
    println!(
        "[blk-bench] random read: {} blocks ({} KiB) in {} ms -> {} KiB/s",
        RAND_BLOCKS,
        rnd_bytes / 1024,
        rnd_ms,
        rnd_kib_s
    );

    println!("[blk-bench] ==== Sync BlockDevice benchmark end ====");
}

/// Phase 8.1 / Step 3：QD=1 异步块设备最小闭环自检。
///
/// 目标只有一条：证明
///
/// ```text
/// task context -> read_block_async() -> read_blocks_nb() -> submit
///   -> schedule_now() 挂起 -> virtio IRQ -> handle_irq() -> peek_used()
///   -> wake task -> complete_read_blocks() -> 返回数据
/// ```
///
/// 这条链能跑通，且**异步读与同步读返回的数据一致**。
///
/// 设计约束（刻意最小化）：
/// - **只读不写**：不向 sdcard.img 写任何块，避免破坏上面的 FAT32 文件系统；
/// - **绕过页缓存**：用 `read_block_raw` / `read_block_async_raw` 两个透传钩子，
///   两次读都直达 virtio-blk，保证比较的是同一份设备内容；
/// - **不碰 `read/write` 主路径**：缓存层接入异步属于 Phase 8.3；
/// - 必须在 **task 上下文**运行（挂在 `kthread_init`），否则 `take_current_task()`
///   返回 `None` 会静默降级成同步读 —— 那样数据也会相等，但链根本没走到异步。
///
/// 判读方式（关键）：
/// - 出现 `[blk-irq] WAKE token=Some(..) -> task resumed by IRQ`
///   → 任务确实被挂起并由中断唤醒，异步闭环成立；
/// - 只有 `RESULT: true` 而没有 WAKE 行
///   → 说明降级成了同步读，**不能**算作异步成功。
pub fn qd1_async_selftest() {
    println!("[blk-async] ==== QD=1 async block device selftest begin ====");

    let device = match BLOCK_DEVICE.get() {
        Some(device) => device.clone(),
        None => {
            println!("[blk-async] BLOCK_DEVICE not initialized -> SKIP");
            return;
        }
    };

    // 对照组：同步读（绕过缓存，直达 virtio-blk）
    let mut sync_buf = [0u8; 512];
    if let Err(err) = device.read_block_raw(TEST_BLOCK_ID, &mut sync_buf) {
        println!("[blk-async] sync read failed: {:?} -> ABORT", err);
        return;
    }

    // 实验组：异步读（同一块）
    let mut async_buf = [0u8; 512];
    if let Err(err) = device.read_block_async_raw(TEST_BLOCK_ID, &mut async_buf) {
        println!("[blk-async] async read failed: {:?} -> ABORT", err);
        return;
    }

    let equal = sync_buf == async_buf;

    println!(
        "[blk-async] block_id={} sync [0..16] = {:?}",
        TEST_BLOCK_ID,
        &sync_buf[..16]
    );
    println!(
        "[blk-async] block_id={} async[0..16] = {:?}",
        TEST_BLOCK_ID,
        &async_buf[..16]
    );

    if equal {
        println!("[blk-async] RESULT: true  (async data == sync data)");
    } else {
        println!("[blk-async] RESULT: FALSE (async data != sync data) !!");
    }

    println!("[blk-async] ==== QD=1 async block device selftest end ====");
    println!("[blk-async] HINT: 必须同时看到 '[blk-irq] WAKE token=Some(..)' 才算异步真正跑通；");
    println!("[blk-async]       若只有 RESULT: true 而无 WAKE 行，说明降级成了同步读。");
}

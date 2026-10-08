#!/usr/bin/env bash
# =============================================================================
# run_crash.sh —— DBFS2 崩溃一致性测试（M6）一键自动化脚本
#
# 用 expect 驱动 qemu，自动完成「写入 → 断电 → 重启 → 校验」全流程，无需手动操作。
#
# 用法：
#   ./run_crash.sh              # 持久化模式：写完正常断电 → 重启校验（验证持久性 D）
#   ./run_crash.sh crash        # 崩溃模式：写入后强制 kill（模拟断电）→ 重启校验（验证崩溃一致性）
#   ./run_crash.sh clean        # 只重置 dbfs.img，不跑
#
# 依赖：expect、qemu-system-riscv64；crash_write/crash_verify 已编进 initramfs
#
# 判定：crash_verify 输出 CRASH_CONSISTENCY,PASS 即通过。
# =============================================================================

set -e
cd "$(dirname "$0")"

MODE="${1:-persist}"
DBFS_IMG="tools/dbfs.img"
EXP_DIR="/tmp/run_crash"

mkdir -p "$EXP_DIR"

log() { echo -e "\n\033[1;36m[run_crash]\033[0m $*"; }

# ---- 前置检查 ----
if [ ! -f kernel-qemu ]; then
    log "错误：缺少 kernel-qemu。请先执行一次：make DBFS_PERSIST=y run"
    log "（首次会编译内核 + 打包用户程序 + 生成 sdcard.img，boot 后 Ctrl-A x 退出即可）"
    exit 1
fi
if [ ! -f tools/sdcard.img ]; then
    log "错误：缺少 tools/sdcard.img。请先执行：make DBFS_PERSIST=y run"
    exit 1
fi
if [ ! -f "$DBFS_IMG" ]; then
    log "首次运行：生成 $DBFS_IMG（16MB 零填充）"
    python3 -c "open('$DBFS_IMG','wb').write(b'\x00'*(16*1024*1024))"
fi

# ---- qemu 命令（与 Makefile DBFS_PERSIST=y 完全一致）----
QEMU_CMD="qemu-system-riscv64 -M virt -bios default \
  -drive file=tools/sdcard.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0 \
  -kernel kernel-qemu -nographic \
  -drive file=$DBFS_IMG,if=none,format=raw,id=x1 -device virtio-blk-device,drive=x1 \
  -device virtio-net-device,netdev=net0 \
  -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555 \
  -initrd tools/initrd/initramfs.cpio.gz -append rdinit=/init \
  -smp 1 -m 1024M -serial mon:stdio"

# ---- 步骤 0：重置镜像 ----
if [ "$MODE" = "clean" ]; then
    log "重置 $DBFS_IMG（16MB 零填充）"
    python3 -c "open('$DBFS_IMG','wb').write(b'\x00'*(16*1024*1024))"
    log "已重置。完成。"
    exit 0
fi

# persist 模式需要干净镜像起步（首次 boot 做格式化）
if [ "$MODE" = "persist" ]; then
    log "重置 $DBFS_IMG（保证从干净状态开始）"
    python3 -c "open('$DBFS_IMG','wb').write(b'\x00'*(16*1024*1024))"
fi

# ---- 通用 expect 驱动脚本 ----
# 参数：<要执行的 shell 命令> <完成标记> <退出方式 graceful|hard>
cat > "$EXP_DIR/drive.exp" <<'EXPECT_EOF'
#!/usr/bin/expect -f
set timeout 180
set CMD    [lindex $argv 0]
set MARKER [lindex $argv 1]
set KILLMODE [lindex $argv 2]

# spawn qemu（-serial mon:stdio，Ctrl-A x 退出）
spawn qemu-system-riscv64 -M virt -bios default \
  -drive file=tools/sdcard.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0 \
  -kernel kernel-qemu -nographic \
  -drive file=[lindex $argv 3],if=none,format=raw,id=x1 -device virtio-blk-device,drive=x1 \
  -device virtio-net-device,netdev=net0 \
  -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555 \
  -initrd tools/initrd/initramfs.cpio.gz -append rdinit=/init \
  -smp 1 -m 1024M -serial mon:stdio

# 等待 Alien shell 提示符
expect {
    -re {Alien:/#\s*$} { }
    timeout { puts "TIMEOUT waiting for shell prompt"; exit 2 }
}

# 发送命令
send -- "$CMD\r"

# 等待完成标记
expect {
    -re $MARKER { }
    timeout { puts "TIMEOUT waiting for marker: $MARKER"; exit 3 }
}

# 等输出刷完
after 1000

if { $KILLMODE == "hard" } {
    # 强制断电：SIGKILL qemu
    set pid [exp_pid -i $spawn_id]
    exec kill -9 $pid
} else {
    # 优雅退出：Ctrl-A x（qemu monitor 退出）
    send -- "\x01x"
    after 1000
}

catch { expect eof }
exit 0
EXPECT_EOF
chmod +x "$EXP_DIR/drive.exp"

# ---- 阶段 1：写入 ----
if [ "$MODE" = "crash" ]; then
    log "阶段 1（崩溃模式）：写入后强制断电（hard kill 模拟断电）"
    KILL="hard"
else
    log "阶段 1（持久化模式）：写入后正常退出"
    KILL="graceful"
fi

"$EXP_DIR/drive.exp" "cd /tests && ./crash_write /dbfs" "CRASH_WRITE,DONE" "$KILL" "$DBFS_IMG" 2>&1 | tee "$EXP_DIR/write.log"
log "阶段 1 完成，qemu 已退出（模拟断电）"

# ---- 阶段 2：重启 + 校验（不重置镜像）----
log "阶段 2：同镜像重启（不重置），运行 crash_verify"
"$EXP_DIR/drive.exp" "cd /tests && ./crash_verify /dbfs" "CRASH_CONSISTENCY" "graceful" "$DBFS_IMG" 2>&1 | tee "$EXP_DIR/verify.log"
log "校验完成，qemu 已退出"

# ---- 判定 ----
echo ""
echo "=============================================================="
if grep -q "CRASH_CONSISTENCY,PASS" "$EXP_DIR/verify.log"; then
    echo " 结果：PASS —— 崩溃一致（commit 数据完整，无 torn write）"
    RESULT=0
else
    echo " 结果：FAIL —— 存在数据丢失/损坏，见上方 crash_verify 输出"
    RESULT=1
fi
echo "=============================================================="

exit $RESULT

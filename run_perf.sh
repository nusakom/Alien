#!/bin/bash
# run_perf.sh —— DBFS2 性能基准一键运行（M2）
#
# 用法（宿主机 Ubuntu/parallels，repo 根目录）：
#   ./run_perf.sh                    # 只跑 DBFS2 (/dbfs)
#   ./run_perf.sh both               # 跑 DBFS2 (/dbfs) + fat32 (/tests) 对照
#
# 输出 CSV 行，可重定向：
#   ./run_perf.sh both | tee perf_result.csv
#
# 每行格式：PERF,<metric>,<path>,<param>,<status>,<value>,<unit>,cycles=<n>

set -e
REPO="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO"

echo "=== [1/3] 编译内核（含写放大计数器埋点）==="
make 2>&1 | tail -20

MODE="${1:-dbfs}"

echo ""
echo "=== [2/3] 测试命令 ==="
echo "  boot 到 Alien:/# 后手动输入："
echo "    cd /tests"
if [ "$MODE" = "both" ]; then
    echo "    ./dbfs_perf /dbfs"
    echo "    ./dbfs_perf /tests"
else
    echo "    ./dbfs_perf /dbfs"
fi
echo ""

if command -v expect >/dev/null 2>&1; then
    echo "=== [3/3] 自动运行 ==="
    if [ "$MODE" = "both" ]; then
        expect <<'EOF'
set timeout 600
spawn make run
expect "Alien:/#"
send "cd /tests\r"
expect "Alien:/tests#"
send "./dbfs_perf /dbfs\r"
expect "DBFS2 性能基准完成"
send "./dbfs_perf /tests\r"
expect "DBFS2 性能基准完成"
send "\u0001x"
expect eof
EOF
    else
        expect <<'EOF'
set timeout 600
spawn make run
expect "Alien:/#"
send "cd /tests\r"
expect "Alien:/tests#"
send "./dbfs_perf /dbfs\r"
expect "DBFS2 性能基准完成"
send "\u0001x"
expect eof
EOF
    fi
else
    echo "=== [3/3] 未检测到 expect，请手动运行 ==="
    echo "  make run"
    if [ "$MODE" = "both" ]; then
        echo "  cd /tests && ./dbfs_perf /dbfs && ./dbfs_perf /tests"
    else
        echo "  cd /tests && ./dbfs_perf /dbfs"
    fi
fi

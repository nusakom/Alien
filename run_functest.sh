#!/bin/bash
# run_functest.sh —— DBFS2 功能测试一键运行（M1）
#
# 用法（在宿主机 Ubuntu/parallels 上，repo 根目录执行）：
#   ./run_functest.sh
#
# 作用：
#   1. 编译内核 + 用户态 apps（含 dbfs_functest）
#   2. 启动 qemu，boot 后自动在 shell 里跑 dbfs_functest（双路径对照 /dbfs vs /tests）
#   3. 输出 CSV 到终端，可重定向保存：
#        ./run_functest.sh | tee functest_result.txt
#
# 注意：qemu 交互式 shell 无法自动输入命令，脚本采用「expect 式」管道：
#   boot 完成后向串口写入测试命令。若你的环境没有 expect，用下面手动方式。

set -e
REPO="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO"

echo "=== [1/3] 编译内核 ==="
make 2>&1 | tail -20

echo ""
echo "=== [2/3] 说明：测试命令 ==="
echo "  boot 到 Alien:/# 后，手动输入："
echo "    cd /tests"
echo "    ./dbfs_functest"
echo ""
echo "  脚本会把上面两条命令通过 expect 自动注入（若安装了 expect）。"
echo ""

# 尝试用 expect 自动化；否则退回手动引导
if command -v expect >/dev/null 2>&1; then
    echo "=== [3/3] 自动运行（expect 注入命令）==="
    expect <<'EOF'
set timeout 300
spawn make run
expect {
    "Alien:/#" { }
    timeout { puts "TIMEOUT waiting for shell"; exit 1 }
}
# 进入 shell 后执行测试
send "cd /tests\r"
expect "Alien:/tests#"
send "./dbfs_functest\r"
expect "DBFS2 功能测试 全部完成"
# 收集完成后退出 qemu（Ctrl-a x 退出）
send "\u0001x"
expect eof
EOF
else
    echo "=== [3/3] 未检测到 expect，请手动运行 ==="
    echo "  make run"
    echo "  然后输入：cd /tests && ./dbfs_functest"
fi

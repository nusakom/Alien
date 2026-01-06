#!/bin/bash
# Elle 测试套件 - 一键运行所有测试
#
# 使用方法:
#   ./run_all_elle_tests.sh              # 交互式选择
#   ./run_all_elle_tests.sh all          # 运行所有测试
#   ./run_all_elle_tests.sh mock         # 使用 mock 内核测试
#   ./run_all_elle_tests.sh real         # 使用真实内核测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ALIEN_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo ""
    echo "========================================"
    echo "$1"
    echo "========================================"
    echo ""
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

# 显示菜单
show_menu() {
    clear
    print_header "🔬 Elle 测试套件"
    echo "请选择测试模式:"
    echo ""
    echo "  1) Mock 内核测试 (快速开发测试)"
    echo "  2) 真实内核测试 (完整集成测试)"
    echo "  3) 通信检查 (TCP 连接测试)"
    echo "  4) 单事务测试 (快速验证)"
    echo "  5) 小规模测试 (2 并发)"
    echo "  6) 完整 Elle 测试 (50000 ops)"
    echo "  7) 运行所有测试"
    echo "  8) 退出"
    echo ""
    read -p "请输入选项 [1-8]: " choice
    echo ""
}

# Mock 内核测试
test_mock_kernel() {
    print_header "🧪 Mock 内核测试"

    print_info "检查 Python..."
    if ! command -v python3 &> /dev/null; then
        print_error "Python3 未安装"
        return 1
    fi
    print_success "Python3 已安装"

    print_info "启动 Mock 内核服务器..."
    echo ""
    print_warning "在另一个终端运行 Elle 客户端:"
    echo ""
    echo "  cd /home/ubuntu2204/Desktop/elle_dbfs_client"
    echo "  ./target/release/elle_dbfs_client"
    echo ""
    print_info "按 Ctrl+C 停止服务器"
    echo ""

    cd "$SCRIPT_DIR"
    python3 mock_kernel_server.py
}

# 真实内核测试
test_real_kernel() {
    print_header "🚀 真实内核测试"

    print_info "启动 Alien 内核..."
    print_warning "内核启动后，在另一个终端运行 Elle 客户端"
    echo ""

    cd "$ALIEN_DIR"
    make elle
}

# 通信检查
test_communication() {
    print_header "🔍 通信检查"

    if [ -f "$SCRIPT_DIR/test_tcp_communication.sh" ]; then
        bash "$SCRIPT_DIR/test_tcp_communication.sh"
    else
        print_error "test_tcp_communication.sh 未找到"
        return 1
    fi
}

# 单事务测试
test_single_transaction() {
    print_header "📝 单事务测试"

    if [ -f "$SCRIPT_DIR/test_single_transaction.sh" ]; then
        bash "$SCRIPT_DIR/test_single_transaction.sh"
    else
        print_error "test_single_transaction.sh 未找到"
        return 1
    fi
}

# 小规模测试
test_small() {
    print_header "🔬 小规模并发测试"

    if [ -f "$SCRIPT_DIR/test_small.sh" ]; then
        bash "$SCRIPT_DIR/test_small.sh"
    else
        print_error "test_small.sh 未找到"
        return 1
    fi
}

# 完整 Elle 测试
test_full_elle() {
    print_header "🎯 完整 Elle 测试"

    if [ -f "$SCRIPT_DIR/run_elle_test.sh" ]; then
        bash "$SCRIPT_DIR/run_elle_test.sh"
    else
        print_error "run_elle_test.sh 未找到"
        return 1
    fi
}

# 运行所有测试
run_all_tests() {
    print_header "🧪 运行所有 Elle 测试"

    local failed_tests=0

    print_info "测试 1/6: 通信检查"
    if ! test_communication; then
        print_error "通信检查失败"
        ((failed_tests++))
    fi
    echo ""

    print_info "测试 2/6: Mock 内核测试"
    print_warning "需要手动启动客户端，按 Ctrl+C 继续..."
    read -p "按 Enter 继续..."
    echo ""

    print_info "测试 3/6: 单事务测试"
    if ! test_single_transaction; then
        print_error "单事务测试失败"
        ((failed_tests++))
    fi
    echo ""

    print_info "测试 4/6: 小规模测试"
    if ! test_small; then
        print_error "小规模测试失败"
        ((failed_tests++))
    fi
    echo ""

    print_info "测试 5/6: 完整 Elle 测试"
    print_warning "这将运行 50000 个操作，需要较长时间..."
    read -p "是否继续? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if ! test_full_elle; then
            print_error "完整 Elle 测试失败"
            ((failed_tests++))
        fi
    else
        print_warning "跳过完整 Elle 测试"
    fi
    echo ""

    print_header "📊 测试结果总结"
    if [ $failed_tests -eq 0 ]; then
        print_success "所有测试通过！"
        return 0
    else
        print_error "$failed_tests 个测试失败"
        return 1
    fi
}

# 主函数
main() {
    local mode="$1"

    if [ -z "$mode" ]; then
        # 交互式模式
        while true; do
            show_menu
            case $choice in
                1) test_mock_kernel ;;
                2) test_real_kernel ;;
                3) test_communication ;;
                4) test_single_transaction ;;
                5) test_small ;;
                6) test_full_elle ;;
                7) run_all_tests ;;
                8)
                    print_info "退出"
                    exit 0
                    ;;
                *)
                    print_error "无效选项"
                    ;;
            esac

            echo ""
            read -p "按 Enter 返回菜单..."
        done
    else
        # 命令行模式
        case $mode in
            mock) test_mock_kernel ;;
            real) test_real_kernel ;;
            comm) test_communication ;;
            single) test_single_transaction ;;
            small) test_small ;;
            full) test_full_elle ;;
            all) run_all_tests ;;
            *)
                print_error "未知模式: $mode"
                echo "用法: $0 [all|mock|real|comm|single|small|full]"
                exit 1
                ;;
        esac
    fi
}

# 运行主函数
main "$@"

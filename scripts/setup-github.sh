#!/bin/bash
# GitHub 仓库设置脚本

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查命令是否存在
check_command() {
    if ! command -v $1 &> /dev/null; then
        print_error "$1 未安装，请先安装"
        exit 1
    fi
}

# 主函数
main() {
    echo "========================================="
    echo "  Pingora Slice GitHub 仓库设置向导"
    echo "========================================="
    echo ""

    # 检查必需工具
    print_info "检查必需工具..."
    check_command git
    check_command cargo
    print_success "所有必需工具已安装"
    echo ""

    # 获取 GitHub 用户名
    print_info "请输入你的 GitHub 用户名："
    read -p "> " GITHUB_USERNAME

    if [ -z "$GITHUB_USERNAME" ]; then
        print_error "GitHub 用户名不能为空"
        exit 1
    fi

    print_success "GitHub 用户名: $GITHUB_USERNAME"
    echo ""

    # 确认
    print_warning "将要执行以下操作："
    echo "  1. 替换所有文件中的 'your-username' 为 '$GITHUB_USERNAME'"
    echo "  2. 初始化 Git 仓库（如果需要）"
    echo "  3. 添加远程仓库"
    echo "  4. 提交并推送代码"
    echo ""
    read -p "是否继续? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "操作已取消"
        exit 0
    fi

    # 替换用户名
    print_info "替换文件中的用户名..."
    
    FILES_TO_UPDATE=(
        ".github/workflows/release.yml"
        ".github/workflows/ci.yml"
        "packaging/install.sh"
        "packaging/pingora-slice.service"
        "packaging/README.md"
        "QUICKSTART.md"
        "CONTRIBUTING.md"
        "SETUP_GUIDE.md"
        "GITHUB_SETUP_SUMMARY.md"
    )

    for file in "${FILES_TO_UPDATE[@]}"; do
        if [ -f "$file" ]; then
            sed -i.bak "s/your-username/$GITHUB_USERNAME/g" "$file"
            rm -f "${file}.bak"
            print_success "已更新: $file"
        else
            print_warning "文件不存在: $file"
        fi
    done
    echo ""

    # 初始化 Git（如果需要）
    if [ ! -d ".git" ]; then
        print_info "初始化 Git 仓库..."
        git init
        git branch -M main
        print_success "Git 仓库已初始化"
    else
        print_info "Git 仓库已存在"
    fi
    echo ""

    # 添加远程仓库
    REMOTE_URL="https://github.com/${GITHUB_USERNAME}/pingora-slice.git"
    
    if git remote | grep -q "^origin$"; then
        print_info "远程仓库 'origin' 已存在"
        CURRENT_URL=$(git remote get-url origin)
        if [ "$CURRENT_URL" != "$REMOTE_URL" ]; then
            print_warning "当前远程 URL: $CURRENT_URL"
            print_warning "期望的 URL: $REMOTE_URL"
            read -p "是否更新远程 URL? (y/n) " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                git remote set-url origin "$REMOTE_URL"
                print_success "远程 URL 已更新"
            fi
        fi
    else
        print_info "添加远程仓库..."
        git remote add origin "$REMOTE_URL"
        print_success "远程仓库已添加: $REMOTE_URL"
    fi
    echo ""

    # 提交更改
    print_info "准备提交更改..."
    
    if [ -n "$(git status --porcelain)" ]; then
        git add .
        git commit -m "chore: setup GitHub repository for $GITHUB_USERNAME"
        print_success "更改已提交"
    else
        print_info "没有需要提交的更改"
    fi
    echo ""

    # 推送代码
    print_info "准备推送代码到 GitHub..."
    read -p "是否现在推送? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        print_info "推送到 origin/main..."
        if git push -u origin main; then
            print_success "代码已成功推送到 GitHub!"
        else
            print_error "推送失败，请检查："
            echo "  1. GitHub 仓库是否已创建: https://github.com/${GITHUB_USERNAME}/pingora-slice"
            echo "  2. 是否有推送权限"
            echo "  3. 网络连接是否正常"
            exit 1
        fi
    else
        print_info "跳过推送，你可以稍后手动推送："
        echo "  git push -u origin main"
    fi
    echo ""

    # 完成
    echo "========================================="
    print_success "设置完成！"
    echo "========================================="
    echo ""
    echo "下一步："
    echo ""
    echo "1. 访问你的 GitHub 仓库："
    echo "   https://github.com/${GITHUB_USERNAME}/pingora-slice"
    echo ""
    echo "2. 验证 GitHub Actions 是否正常运行："
    echo "   https://github.com/${GITHUB_USERNAME}/pingora-slice/actions"
    echo ""
    echo "3. 创建第一个 release："
    echo "   git tag v0.1.0"
    echo "   git push origin v0.1.0"
    echo ""
    echo "4. 查看生成的 RPM 包："
    echo "   https://github.com/${GITHUB_USERNAME}/pingora-slice/releases"
    echo ""
    echo "详细文档请查看："
    echo "  - QUICKSTART.md - 快速开始"
    echo "  - SETUP_GUIDE.md - 完整设置指南"
    echo "  - GITHUB_SETUP_SUMMARY.md - GitHub 配置总结"
    echo ""
}

# 运行主函数
main "$@"

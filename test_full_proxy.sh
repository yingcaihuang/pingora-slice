#!/bin/bash
# 测试完整代理服务器

set -e

echo "=== 完整代理服务器测试 ==="
echo

# 步骤 1: 创建缓存文件
CACHE_FILE="./my-slice-raw"
CACHE_SIZE=$((1024 * 1024 * 1024))  # 1GB

if [ ! -f "$CACHE_FILE" ]; then
    echo "步骤 1: 创建缓存文件..."
    dd if=/dev/zero of="$CACHE_FILE" bs=1048576 count=1024 2>/dev/null
    chmod 600 "$CACHE_FILE"
    echo "  ✓ 缓存文件已创建: $(ls -lh $CACHE_FILE | awk '{print $5}')"
else
    echo "步骤 1: 缓存文件已存在"
fi
echo

# 步骤 2: 更新配置
echo "步骤 2: 检查配置..."
if [ ! -f "pingora_slice_raw_disk_full.yaml" ]; then
    echo "  ✗ 配置文件不存在!"
    exit 1
fi

# 更新配置中的 total_size 为 1GB
sed -i.bak 's/total_size: 10737418240/total_size: 1073741824/' pingora_slice_raw_disk_full.yaml
echo "  ✓ 配置已更新（total_size = 1GB）"
echo

# 步骤 3: 编译
echo "步骤 3: 编译代理服务器..."
cargo build --release --example full_proxy_server --quiet
echo "  ✓ 编译完成"
echo

# 步骤 4: 运行服务器
echo "步骤 4: 启动代理服务器..."
echo "  配置文件: pingora_slice_raw_disk_full.yaml"
echo "  缓存文件: $CACHE_FILE"
echo "  监听地址: http://127.0.0.1:8080"
echo
echo "=========================================="
echo "服务器已启动！"
echo
echo "在另一个终端中测试："
echo "  # 第一次请求（缓存未命中，会回源）"
echo "  curl http://localhost:8080/dl/15m.iso -o /dev/null -v"
echo
echo "  # 第二次请求（缓存命中）"
echo "  curl http://localhost:8080/dl/15m.iso -o /dev/null -v"
echo
echo "  # 查看缓存统计"
echo "  curl http://localhost:8080/stats | jq ."
echo
echo "按 Ctrl+C 停止服务器"
echo "=========================================="
echo

./target/release/examples/full_proxy_server pingora_slice_raw_disk_full.yaml

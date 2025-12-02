# 测试完整代理服务器

## 问题已修复

之前的错误 `builder error` 是因为 URL 被重复拼接导致的。现在已经修复。

## 快速测试

### 1. 启动服务器

```bash
# 确保缓存文件存在
ls -lh ./my-slice-raw

# 如果不存在，创建它
dd if=/dev/zero of=./my-slice-raw bs=1048576 count=1024
chmod 600 ./my-slice-raw

# 启动服务器
./target/release/examples/full_proxy_server pingora_slice_raw_disk_full.yaml
```

### 2. 在另一个终端测试

```bash
# 测试 1: 请求一个文件（第一次 - 缓存未命中，会回源）
curl http://localhost:8080/dl/15m.iso -o /dev/null -v

# 预期日志：
# INFO full_proxy_server: Cache MISS: http://localhost:8080/dl/15m.iso, fetching from origin
# INFO full_proxy_server: Fetching from upstream: http://mirrors.verycloud.cn/dl/15m.iso
# INFO full_proxy_server: Upstream response: 200 OK
# INFO full_proxy_server: Fetched XXXXX bytes from upstream
# INFO full_proxy_server: Stored in cache

# 测试 2: 再次请求同一个文件（第二次 - 缓存命中）
curl http://localhost:8080/dl/15m.iso -o /dev/null -v

# 预期日志：
# INFO full_proxy_server: Cache HIT: http://localhost:8080/dl/15m.iso

# 测试 3: 查看缓存统计
curl http://localhost:8080/stats | jq .

# 预期输出：
# {
#   "cache": {
#     "l1": {
#       "entries": X,
#       "bytes": XXXX,
#       "hits": X
#     },
#     "l2": {
#       "hits": X,
#       "writes": X,
#       "errors": 0
#     },
#     "misses": X
#   }
# }
```

## 注意事项

### URL 格式

代理服务器现在支持两种 URL 格式：

1. **完整 URL**（推荐用于测试）：
   ```bash
   curl http://localhost:8080/http://mirrors.verycloud.cn/dl/15m.iso
   ```
   或者直接使用 wget 的代理模式：
   ```bash
   wget -e http_proxy=127.0.0.1:8080 http://mirrors.verycloud.cn/dl/15m.iso -O /dev/null
   ```

2. **路径格式**（需要配置 upstream_address）：
   ```bash
   # 配置文件中设置：upstream_address: "mirrors.verycloud.cn:80"
   curl http://localhost:8080/dl/15m.iso -o /dev/null
   ```

### 当前配置

查看 `pingora_slice_raw_disk_full.yaml`：
```yaml
upstream_address: "mirrors.verycloud.cn:80"
```

这意味着请求 `http://localhost:8080/dl/15m.iso` 会被转发到 `http://mirrors.verycloud.cn:80/dl/15m.iso`

## 故障排查

### 问题：仍然报 "builder error"

**原因**：URL 格式不正确

**解决**：
```bash
# 错误的方式（URL 会被重复）
curl http://localhost:8080/http://mirrors.verycloud.cn/dl/15m.iso

# 正确的方式（使用路径）
curl http://localhost:8080/dl/15m.iso -o /dev/null
```

### 问题：连接超时

**原因**：上游服务器不可达

**解决**：
```bash
# 测试上游服务器是否可访问
curl -I http://mirrors.verycloud.cn/dl/15m.iso

# 如果不可访问，更换上游服务器
# 编辑 pingora_slice_raw_disk_full.yaml:
upstream_address: "httpbin.org:80"

# 然后测试
curl http://localhost:8080/get -o /dev/null
```

### 问题：缓存未生效

**检查**：
```bash
# 查看缓存统计
curl http://localhost:8080/stats | jq .

# 检查缓存文件
ls -lh ./my-slice-raw

# 查看服务器日志
# 应该看到 "Stored in cache" 消息
```

## 性能测试

```bash
# 测试缓存性能
# 第一次请求（缓存未命中）
time curl http://localhost:8080/dl/15m.iso -o /dev/null

# 第二次请求（缓存命中，应该更快）
time curl http://localhost:8080/dl/15m.iso -o /dev/null

# 多次请求测试
for i in {1..10}; do
  time curl http://localhost:8080/dl/15m.iso -o /dev/null
done
```

## 下一步

1. ✅ 基本代理功能已工作
2. ✅ Raw disk cache 已集成
3. ✅ 缓存命中/未命中逻辑正确

**改进建议**：
- 添加 Range 请求支持
- 添加更详细的错误处理
- 添加请求/响应头转发
- 添加更多的监控指标

## 总结

现在代理服务器可以：
- ✅ 正确处理缓存未命中
- ✅ 从上游服务器获取内容
- ✅ 存储到 raw disk cache
- ✅ 后续请求从缓存返回

测试命令：
```bash
# 启动服务器
./target/release/examples/full_proxy_server pingora_slice_raw_disk_full.yaml

# 在另一个终端测试
curl http://localhost:8080/dl/15m.iso -o /dev/null -v
```

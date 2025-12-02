# 完整代理服务器使用指南

## 问题说明

你遇到的问题是：运行 `pingora-slice` 二进制文件时，虽然配置了上游服务器，但请求返回 404，没有回源获取数据。

**根本原因**：`pingora-slice` 二进制文件（来自 `examples/http_purge_server.rs`）是一个**测试服务器**，只用于测试缓存和 PURGE 功能，**不会回源到上游服务器**。

## 解决方案

使用新创建的 `full_proxy_server` 示例，它是一个完整的代理服务器，支持：
- ✅ 缓存查找
- ✅ 缓存未命中时回源
- ✅ 存储获取的内容到缓存
- ✅ 支持 raw disk cache

## 使用步骤

### 1. 创建缓存文件（如果使用 raw disk cache）

```bash
# 创建 1GB 缓存文件用于测试
dd if=/dev/zero of=./my-slice-raw bs=1048576 count=1024
chmod 600 ./my-slice-raw
```

### 2. 更新配置文件

编辑 `pingora_slice_raw_disk_full.yaml`，确保：

```yaml
# 上游服务器地址（不要包含 http://）
upstream_address: "mirrors.verycloud.cn:80"

# Raw disk cache 配置
raw_disk_cache:
  device_path: "./my-slice-raw"
  total_size: 1073741824  # 1GB（与创建的文件大小匹配）
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

### 3. 编译并运行完整代理服务器

```bash
# 编译
cargo build --release --example full_proxy_server

# 运行
./target/release/examples/full_proxy_server pingora_slice_raw_disk_full.yaml
```

### 4. 测试

```bash
# 第一次请求 - 缓存未命中，会从上游获取
curl http://localhost:8080/dl/15m.iso -o /dev/null -v

# 第二次请求 - 缓存命中，直接从缓存返回
curl http://localhost:8080/dl/15m.iso -o /dev/null -v

# 查看缓存统计
curl http://localhost:8080/stats | jq .
```

## 日志说明

### 缓存未命中（第一次请求）

```
INFO full_proxy_server: GET http://localhost:8080/dl/15m.iso
INFO full_proxy_server: Cache MISS: http://localhost:8080/dl/15m.iso, fetching from origin
INFO full_proxy_server: Fetching from upstream: http://mirrors.verycloud.cn:80/dl/15m.iso
INFO full_proxy_server: Upstream response: 200 OK
INFO full_proxy_server: Fetched 15728640 bytes from upstream
INFO full_proxy_server: Stored in cache: http://localhost:8080/dl/15m.iso
```

### 缓存命中（第二次请求）

```
INFO full_proxy_server: GET http://localhost:8080/dl/15m.iso
INFO full_proxy_server: Cache HIT: http://localhost:8080/dl/15m.iso
```

## 与原 pingora-slice 的区别

| 特性 | pingora-slice (http_purge_server) | full_proxy_server |
|------|----------------------------------|-------------------|
| 缓存查找 | ✅ | ✅ |
| 回源获取 | ❌ | ✅ |
| 存储到缓存 | ❌ | ✅ |
| PURGE 支持 | ✅ | ❌ |
| 用途 | 测试缓存和 PURGE | 完整代理服务器 |

## 故障排查

### 问题：仍然返回 404

**检查项**：
1. 上游服务器地址是否正确
2. 上游服务器是否可访问
3. 请求的路径是否存在

```bash
# 直接测试上游服务器
curl http://mirrors.verycloud.cn/dl/15m.iso -I

# 查看代理服务器日志
# 应该看到 "Fetching from upstream" 消息
```

### 问题：缓存文件错误

```
Error: Failed to create raw disk cache
```

**解决方案**：
```bash
# 确保缓存文件存在且大小正确
ls -lh ./my-slice-raw

# 确保配置中的 total_size 与文件大小匹配
# 文件大小 = total_size 字节
```

### 问题：权限错误

```
Error: Permission denied
```

**解决方案**：
```bash
# 设置正确的权限
chmod 600 ./my-slice-raw

# 确保当前用户有读写权限
ls -l ./my-slice-raw
```

## 性能优化

### 调整缓存大小

```yaml
# 增加缓存大小以存储更多内容
raw_disk_cache:
  total_size: 10737418240  # 10GB
```

### 调整块大小

```yaml
# 对于大文件，使用更大的块
raw_disk_cache:
  block_size: 16384  # 16KB
```

### 禁用压缩（如果内容已压缩）

```yaml
# 对于已压缩的内容（如视频、图片）
raw_disk_cache:
  enable_compression: false
```

## 下一步

1. **生产部署**：参考 `docs/RAW_DISK_USER_GUIDE.md` 了解生产环境配置
2. **性能调优**：参考 `docs/PERFORMANCE_TUNING.md` 优化性能
3. **监控**：使用 `/stats` 端点监控缓存性能

## 总结

- ✅ 使用 `full_proxy_server` 示例获得完整的代理功能
- ✅ 支持缓存未命中时自动回源
- ✅ 支持 raw disk cache 和 file-based cache
- ✅ 适合开发和测试使用

对于生产环境，建议基于 `full_proxy_server` 示例进行扩展，添加更多功能如：
- 完整的 HTTP 头处理
- Range 请求支持
- 错误重试机制
- 更详细的日志和监控

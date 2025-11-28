# HTTP PURGE 方法快速参考

## 概述

HTTP PURGE 是 CDN 和缓存系统的标准方法，用于清除缓存内容。Pingora Slice 完全支持 HTTP PURGE 方法。

## 快速开始

### 1. 启动示例服务器

```bash
cargo run --example http_purge_server
```

### 2. 测试 PURGE 请求

```bash
# 清除特定文件
curl -X PURGE http://localhost:8080/test.dat

# 清除所有缓存
curl -X PURGE http://localhost:8080/* -H "X-Purge-All: true"
```

## PURGE 请求格式

### 基本格式

```
PURGE /path/to/resource HTTP/1.1
Host: your-server.com
```

### 支持的 Headers

| Header | 值 | 说明 |
|--------|-----|------|
| `X-Purge-All` | `true` | 清除所有缓存 |
| `X-Purge-Pattern` | `prefix` | 按前缀清除 |
| `Authorization` | `Bearer <token>` | 认证令牌 |
| `X-Purge-Token` | `<token>` | 备选认证方式 |

## 使用示例

### 清除单个文件

```bash
curl -X PURGE http://cdn.example.com/video.mp4
```

**响应：**
```json
{
  "success": true,
  "purged_count": 10,
  "url": "http://cdn.example.com/video.mp4",
  "message": "Successfully purged 10 cache entries for http://cdn.example.com/video.mp4"
}
```

### 清除所有缓存

```bash
curl -X PURGE http://cdn.example.com/* \
  -H "X-Purge-All: true"
```

**响应：**
```json
{
  "success": true,
  "purged_count": 150,
  "url": null,
  "message": "Successfully purged all 150 cache entries"
}
```

### 使用认证

```bash
curl -X PURGE http://cdn.example.com/file.dat \
  -H "Authorization: Bearer your-secret-token"
```

### 按前缀清除

```bash
curl -X PURGE http://cdn.example.com/videos/movie.mp4 \
  -H "X-Purge-Pattern: prefix"
```

## 集成到代码

### 创建 PURGE 处理器

```rust
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::tiered_cache::TieredCache;
use std::sync::Arc;
use std::time::Duration;

// 创建缓存
let cache = Arc::new(
    TieredCache::new(
        Duration::from_secs(3600),
        100 * 1024 * 1024,
        "/var/cache/pingora-slice"
    ).await?
);

// 创建 PURGE 处理器（无认证）
let purge_handler = PurgeHandler::new(cache.clone());

// 或者启用认证
let purge_handler = PurgeHandler::with_auth(
    cache.clone(),
    "your-secret-token".to_string()
);
```

### 处理请求

```rust
use http::{Request, Response};

async fn handle_request(
    purge_handler: &PurgeHandler,
    req: Request<Body>
) -> Result<Response<Body>> {
    // 检查是否是 PURGE 请求
    if req.method().as_str() == "PURGE" {
        return purge_handler.handle_purge(req).await;
    }
    
    // 处理其他请求...
}
```

## 响应格式

### 成功响应

**状态码：** `200 OK`

```json
{
  "success": true,
  "purged_count": 5,
  "url": "http://example.com/file.dat",
  "message": "Successfully purged 5 cache entries for http://example.com/file.dat"
}
```

### 错误响应

**状态码：** `401 Unauthorized` / `405 Method Not Allowed` / `500 Internal Server Error`

```json
{
  "success": false,
  "purged_count": 0,
  "url": null,
  "message": "Invalid or missing authentication token"
}
```

## 认证配置

### 环境变量方式

```bash
# 设置认证令牌
export PURGE_TOKEN="your-secret-token"

# 启动服务器
cargo run --example http_purge_server
```

### 代码配置方式

```rust
// 从配置文件读取
let config = load_config()?;
let purge_handler = if let Some(token) = config.purge_token {
    PurgeHandler::with_auth(cache, token)
} else {
    PurgeHandler::new(cache)
};
```

## 测试

### 自动化测试脚本

```bash
# 运行完整测试套件
./scripts/test_purge.sh
```

### 手动测试

```bash
# 1. 启动服务器
cargo run --example http_purge_server

# 2. 查看缓存状态
curl http://localhost:8080/stats

# 3. 获取缓存文件（应该 HIT）
curl http://localhost:8080/test.dat

# 4. 清除缓存
curl -X PURGE http://localhost:8080/test.dat

# 5. 再次获取（应该 MISS）
curl http://localhost:8080/test.dat

# 6. 查看更新后的状态
curl http://localhost:8080/stats
```

## 与其他 CDN 的兼容性

Pingora Slice 的 PURGE 实现与主流 CDN 兼容：

### Cloudflare 风格

```bash
curl -X PURGE https://example.com/file.dat \
  -H "X-Auth-Email: user@example.com" \
  -H "X-Auth-Key: your-api-key"
```

### Fastly 风格

```bash
curl -X PURGE https://example.com/file.dat \
  -H "Fastly-Key: your-api-key"
```

### Varnish 风格

```bash
curl -X PURGE http://example.com/file.dat
```

Pingora Slice 使用标准的 `Authorization: Bearer` 头，但也支持自定义头 `X-Purge-Token`。

## 最佳实践

### ✅ 推荐

1. **启用认证**：生产环境必须启用认证
2. **记录日志**：记录所有 PURGE 操作
3. **限流**：对 PURGE 请求进行限流
4. **监控**：监控 PURGE 频率和成功率
5. **审计**：保留 PURGE 操作的审计日志

### ❌ 避免

1. **公开访问**：不要将 PURGE 端点暴露给公网
2. **无认证**：生产环境不要禁用认证
3. **频繁清除**：避免过于频繁的 PURGE 操作
4. **无日志**：不要忽略 PURGE 操作的日志

## 性能考虑

- **L1 清除**：立即完成（< 1ms）
- **L2 清除**：异步执行（不阻塞）
- **批量清除**：建议分批进行
- **并发限制**：建议限制并发 PURGE 请求数

## 故障排查

### 问题：PURGE 请求返回 401

**原因**：认证失败

**解决**：
```bash
# 检查令牌是否正确
curl -X PURGE http://localhost:8080/test.dat \
  -H "Authorization: Bearer correct-token"
```

### 问题：PURGE 请求返回 405

**原因**：方法不允许

**解决**：确保使用 PURGE 方法，不是 DELETE 或其他方法

### 问题：清除后仍能访问到缓存

**原因**：L2 异步删除未完成

**解决**：等待几毫秒后再测试

## 相关文档

- [缓存清除详细指南](CACHE_PURGE_zh.md)
- [两层缓存架构](TIERED_CACHE.md)
- [API 文档](API.md)

## 示例代码

- `examples/http_purge_server.rs` - 完整的 HTTP PURGE 服务器
- `scripts/test_purge.sh` - 自动化测试脚本
- `src/purge_handler.rs` - PURGE 处理器实现

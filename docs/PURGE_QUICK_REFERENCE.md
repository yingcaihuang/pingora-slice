# Cache Purge Quick Reference

## 快速参考

### 三种清除方法

| 方法 | 用途 | 返回值 | 示例 |
|------|------|--------|------|
| `purge(url, range)` | 清除单个切片 | `bool` | `cache.purge(url, &range).await?` |
| `purge_url(url)` | 清除 URL 所有切片 | `usize` | `cache.purge_url(url).await?` |
| `purge_all()` | 清除所有缓存 | `usize` | `cache.purge_all().await?` |

### 代码示例

```rust
use pingora_slice::tiered_cache::TieredCache;
use pingora_slice::models::ByteRange;

// 1. 清除单个切片
let range = ByteRange::new(0, 1048575)?;
let found = cache.purge("http://example.com/file.dat", &range).await?;
println!("Found and purged: {}", found);

// 2. 清除整个文件
let count = cache.purge_url("http://example.com/file.dat").await?;
println!("Purged {} slices", count);

// 3. 清除所有
let count = cache.purge_all().await?;
println!("Purged {} total items", count);
```

### 常见场景

#### 内容更新
```rust
// 源站文件更新后
cache.purge_url("http://cdn.example.com/app.js").await?;
```

#### 紧急下线
```rust
// 立即删除敏感内容
cache.purge_url("http://cdn.example.com/sensitive.pdf").await?;
```

#### 批量清理
```rust
// 清理多个文件
for url in urls_to_purge {
    cache.purge_url(&url).await?;
}
```

#### 维护模式
```rust
// 进入维护前清空缓存
cache.purge_all().await?;
```

### HTTP API 示例

```rust
// POST /api/cache/purge
{
  "url": "http://example.com/file.dat"
}

// POST /api/cache/purge
{
  "purge_all": true
}
```

### 性能特点

- ⚡ L1 清除：< 1ms（同步）
- ⚡ L2 清除：异步，不阻塞
- ⚡ 时间复杂度：O(1) 单项，O(n) 批量

### 注意事项

✅ **推荐**
- 记录清除操作日志
- 添加监控指标
- 错误处理不影响主流程

❌ **避免**
- 过于频繁清除
- 在请求路径中同步清除
- 无权限验证的公开 API

### 运行示例

```bash
cargo run --example tiered_cache_purge_example
```

### 更多信息

- 详细文档：[docs/CACHE_PURGE_zh.md](CACHE_PURGE_zh.md)
- 架构说明：[docs/TIERED_CACHE.md](TIERED_CACHE.md)
- 功能总结：[docs/PURGE_FEATURE_SUMMARY.md](PURGE_FEATURE_SUMMARY.md)

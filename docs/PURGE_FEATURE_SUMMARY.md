# Cache Purge Feature Summary

## 新增功能

为 `TieredCache` 添加了三个缓存清除方法：

### 1. `purge(url, range)` - 清除单个缓存项

```rust
pub async fn purge(&self, url: &str, range: &ByteRange) -> Result<bool>
```

- 删除指定 URL 和字节范围的缓存
- 同时清除 L1（内存）和 L2（磁盘）
- 返回是否找到并删除了缓存项

### 2. `purge_url(url)` - 清除 URL 的所有切片

```rust
pub async fn purge_url(&self, url: &str) -> Result<usize>
```

- 删除指定 URL 的所有缓存切片
- 返回清除的缓存项数量

### 3. `purge_all()` - 清除所有缓存

```rust
pub async fn purge_all(&self) -> Result<usize>
```

- 清空整个缓存系统
- 返回清除的缓存项数量

## 实现细节

### 消息类型扩展

在 `DiskWriteMessage` 枚举中添加了 `Delete` 变体：

```rust
enum DiskWriteMessage {
    Write { key: String, data: Bytes, expires_at: SystemTime },
    Delete { key: String },  // 新增
    Shutdown,
}
```

### 异步磁盘删除

L2 缓存的删除操作通过异步任务处理，不阻塞主请求流程：

```rust
DiskWriteMessage::Delete { key } => {
    let file_path = Self::get_l2_file_path_static(&base_path, &key);
    if let Err(e) = fs::remove_file(&file_path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("Failed to delete L2 cache file: {}", e);
        }
    }
}
```

## 测试覆盖

添加了三个测试用例：

1. `test_purge_single_entry` - 测试单项清除
2. `test_purge_url` - 测试 URL 清除
3. `test_purge_all` - 测试全部清除

所有测试通过：

```
running 5 tests
test tiered_cache::tests::test_l1_cache ... ok
test tiered_cache::tests::test_purge_url ... ok
test tiered_cache::tests::test_purge_all ... ok
test tiered_cache::tests::test_purge_single_entry ... ok
test tiered_cache::tests::test_l2_persistence ... ok
```

## 文档

### 新增文件

1. **examples/tiered_cache_purge_example.rs**
   - 完整的使用示例
   - 展示四种不同的清除场景

2. **docs/CACHE_PURGE_zh.md**
   - 详细的中文使用指南
   - 包含实际使用场景和最佳实践
   - HTTP API 集成示例

### 更新文件

1. **docs/TIERED_CACHE.md**
   - 添加了"缓存清除"章节
   - 说明清除方法和使用场景

2. **Cargo.toml**
   - 添加 `tempfile = "3.0"` 到 dev-dependencies

## 使用示例

### 基本用法

```rust
use pingora_slice::tiered_cache::TieredCache;
use pingora_slice::models::ByteRange;
use std::time::Duration;

// 创建缓存
let cache = TieredCache::new(
    Duration::from_secs(3600),
    10 * 1024 * 1024,
    "/var/cache/pingora-slice"
).await?;

// 清除单个切片
let range = ByteRange::new(0, 1048575)?;
cache.purge("http://example.com/file.dat", &range).await?;

// 清除整个文件
cache.purge_url("http://example.com/file.dat").await?;

// 清除所有缓存
cache.purge_all().await?;
```

### 运行示例

```bash
cargo run --example tiered_cache_purge_example
```

## 性能特点

- ✅ **非阻塞**：L2 删除是异步的，不影响请求处理
- ✅ **高效**：L1 删除是 O(1) 操作
- ✅ **原子性**：L1 删除是原子操作
- ✅ **幂等性**：重复删除不会报错

## API 稳定性

所有新增的 API 都是 `pub async fn`，可以安全地在生产环境使用。

## 向后兼容

- ✅ 不影响现有 API
- ✅ 不改变现有行为
- ✅ 纯新增功能

## 下一步

可以考虑的增强功能：

1. **模式匹配清除**：支持通配符或正则表达式
2. **批量清除 API**：一次清除多个 URL
3. **清除统计**：记录清除操作的指标
4. **清除回调**：清除完成后的通知机制
5. **分布式清除**：多实例环境下的清除同步

## 相关文件

- `src/tiered_cache.rs` - 核心实现
- `examples/tiered_cache_purge_example.rs` - 使用示例
- `docs/CACHE_PURGE_zh.md` - 中文文档
- `docs/TIERED_CACHE.md` - 架构文档

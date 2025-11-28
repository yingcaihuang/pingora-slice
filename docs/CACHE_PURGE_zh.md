# 缓存清除功能使用指南

## 概述

Pingora Slice 的两层缓存系统支持灵活的缓存清除（Purge）操作，可以精确控制要删除的缓存内容。

## 三种清除方式

### 1. 清除单个缓存项 (purge)

删除特定 URL 和字节范围的缓存切片。

```rust
use pingora_slice::tiered_cache::TieredCache;
use pingora_slice::models::ByteRange;

// 创建缓存实例
let cache = TieredCache::new(
    Duration::from_secs(3600),
    10 * 1024 * 1024,
    "/var/cache/pingora-slice"
).await?;

// 清除特定的缓存切片
let url = "http://example.com/video.mp4";
let range = ByteRange::new(0, 1048575)?; // 第一个 1MB 切片
let purged = cache.purge(url, &range).await?;

if purged {
    println!("✓ 缓存已清除");
} else {
    println!("✗ 缓存不存在");
}
```

**适用场景**：
- 需要精确控制清除哪个切片
- 只有部分内容更新时

### 2. 清除 URL 的所有切片 (purge_url)

删除某个 URL 的所有缓存切片（包括所有字节范围）。

```rust
// 清除整个文件的所有切片
let url = "http://example.com/largefile.bin";
let count = cache.purge_url(url).await?;
println!("✓ 清除了 {} 个缓存项", count);
```

**适用场景**：
- 源站文件完全更新
- 需要下线某个文件
- 文件内容有错误需要重新缓存

### 3. 清除所有缓存 (purge_all)

清空整个缓存系统的所有数据。

```rust
// 清除所有缓存数据
let count = cache.purge_all().await?;
println!("✓ 清除了 {} 个缓存项", count);
```

**适用场景**：
- 系统维护
- 批量内容更新
- 缓存策略调整

## 清除行为说明

### L1 和 L2 同时清除

所有清除操作都会同时作用于 L1（内存）和 L2（磁盘）：

- **L1 清除**：立即从内存中删除（同步）
- **L2 清除**：异步从磁盘删除（不阻塞请求）

### 性能特点

- ✅ **非阻塞**：清除操作不会阻塞正常请求
- ✅ **原子性**：L1 清除是原子操作
- ✅ **幂等性**：重复清除同一项不会报错
- ✅ **高效**：L1 清除是 O(1) 时间复杂度

## 实际使用示例

### 示例 1：内容更新后清除缓存

```rust
// 场景：CDN 上的 JavaScript 文件更新了
async fn update_js_file(cache: &TieredCache) -> Result<()> {
    let url = "http://cdn.example.com/app.js";
    
    // 1. 更新源站文件（在你的部署流程中）
    deploy_new_version(url).await?;
    
    // 2. 清除旧缓存
    let count = cache.purge_url(url).await?;
    tracing::info!("清除了 {} 个缓存切片", count);
    
    // 3. 下次请求会自动从源站获取新版本
    Ok(())
}
```

### 示例 2：批量清理视频缓存

```rust
// 场景：清理所有视频文件的缓存
async fn purge_all_videos(cache: &TieredCache) -> Result<usize> {
    let video_urls = vec![
        "http://cdn.example.com/videos/movie1.mp4",
        "http://cdn.example.com/videos/movie2.mp4",
        "http://cdn.example.com/videos/movie3.mp4",
    ];
    
    let mut total_purged = 0;
    for url in video_urls {
        let count = cache.purge_url(url).await?;
        total_purged += count;
        tracing::info!("清除 {}: {} 个切片", url, count);
    }
    
    Ok(total_purged)
}
```

### 示例 3：选择性清除

```rust
// 场景：只清除特定模式的缓存
async fn selective_purge(cache: &TieredCache, pattern: &str) -> Result<usize> {
    // 获取所有缓存的 URL（需要你自己维护 URL 列表）
    let all_urls = get_cached_urls();
    
    let mut total_purged = 0;
    for url in all_urls {
        if url.contains(pattern) {
            let count = cache.purge_url(&url).await?;
            total_purged += count;
        }
    }
    
    Ok(total_purged)
}

// 使用示例
let purged = selective_purge(&cache, "/videos/").await?;
println!("清除了 {} 个视频缓存", purged);
```

### 示例 4：定时清理任务

```rust
// 场景：每天凌晨清理过期内容
use tokio::time::{interval, Duration};

async fn scheduled_purge_task(cache: Arc<TieredCache>) {
    let mut interval = interval(Duration::from_secs(86400)); // 24小时
    
    loop {
        interval.tick().await;
        
        // 清理特定的过期内容
        let urls_to_purge = get_expired_urls().await;
        
        for url in urls_to_purge {
            match cache.purge_url(&url).await {
                Ok(count) => {
                    tracing::info!("定时清理: {} ({} 个切片)", url, count);
                }
                Err(e) => {
                    tracing::error!("清理失败: {} - {}", url, e);
                }
            }
        }
    }
}
```

## HTTP PURGE 方法（推荐）

Pingora Slice 支持标准的 HTTP PURGE 方法，这是 CDN 和缓存系统的行业标准做法。

### 基本用法

```bash
# 清除特定 URL
curl -X PURGE http://your-server.com/path/to/file.dat

# 清除所有缓存
curl -X PURGE http://your-server.com/* -H "X-Purge-All: true"

# 使用认证
curl -X PURGE http://your-server.com/file.dat \
  -H "Authorization: Bearer your-secret-token"
```

### 集成到服务器

```rust
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::tiered_cache::TieredCache;
use std::sync::Arc;

// 创建缓存和 PURGE 处理器
let cache = Arc::new(TieredCache::new(...).await?);

// 不需要认证
let purge_handler = PurgeHandler::new(cache.clone());

// 或者启用认证
let purge_handler = PurgeHandler::with_auth(
    cache.clone(),
    "your-secret-token".to_string()
);

// 在请求处理中使用
async fn handle_request(req: Request) -> Response {
    if req.method() == "PURGE" {
        return purge_handler.handle_purge(req).await?;
    }
    // ... 其他请求处理
}
```

### 支持的 PURGE 选项

#### 1. 清除特定 URL

```bash
curl -X PURGE http://cdn.example.com/video.mp4
```

响应：
```json
{
  "success": true,
  "purged_count": 10,
  "url": "http://cdn.example.com/video.mp4",
  "message": "Successfully purged 10 cache entries for http://cdn.example.com/video.mp4"
}
```

#### 2. 清除所有缓存

```bash
curl -X PURGE http://cdn.example.com/* \
  -H "X-Purge-All: true"
```

响应：
```json
{
  "success": true,
  "purged_count": 150,
  "url": null,
  "message": "Successfully purged all 150 cache entries"
}
```

#### 3. 按前缀清除

```bash
curl -X PURGE http://cdn.example.com/videos/movie.mp4 \
  -H "X-Purge-Pattern: prefix"
```

### 认证方式

支持两种认证方式：

#### 方式 1：Authorization Bearer Token

```bash
curl -X PURGE http://cdn.example.com/file.dat \
  -H "Authorization: Bearer your-secret-token"
```

#### 方式 2：X-Purge-Token Header

```bash
curl -X PURGE http://cdn.example.com/file.dat \
  -H "X-Purge-Token: your-secret-token"
```

### 完整示例服务器

查看完整的 HTTP PURGE 服务器示例：

```bash
# 运行示例服务器
cargo run --example http_purge_server

# 在另一个终端测试
curl -X PURGE http://localhost:8080/test.dat
```

### 测试脚本

使用提供的测试脚本：

```bash
# 启动服务器
cargo run --example http_purge_server &

# 运行测试
./scripts/test_purge.sh
```

## HTTP API 集成（备选方案）

如果你更喜欢 RESTful API 风格，也可以使用 POST 请求：

```rust
use axum::{Router, Json};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct PurgeRequest {
    url: Option<String>,
    purge_all: Option<bool>,
}

#[derive(Serialize)]
struct PurgeResponse {
    success: bool,
    purged_count: usize,
    message: String,
}

async fn handle_purge(
    cache: Arc<TieredCache>,
    Json(req): Json<PurgeRequest>,
) -> Json<PurgeResponse> {
    let (count, message) = if req.purge_all == Some(true) {
        // 清除所有
        match cache.purge_all().await {
            Ok(count) => (count, format!("清除了所有 {} 个缓存项", count)),
            Err(e) => (0, format!("清除失败: {}", e)),
        }
    } else if let Some(url) = req.url {
        // 清除特定 URL
        match cache.purge_url(&url).await {
            Ok(count) => (count, format!("清除了 {} 的 {} 个缓存项", url, count)),
            Err(e) => (0, format!("清除失败: {}", e)),
        }
    } else {
        (0, "请指定 url 或 purge_all".to_string())
    };
    
    Json(PurgeResponse {
        success: count > 0,
        purged_count: count,
        message,
    })
}

// 路由配置
let app = Router::new()
    .route("/api/cache/purge", post(handle_purge));
```

使用 API：

```bash
# 清除特定 URL
curl -X POST http://localhost:8080/api/cache/purge \
  -H "Content-Type: application/json" \
  -d '{"url": "http://example.com/file.dat"}'

# 清除所有缓存
curl -X POST http://localhost:8080/api/cache/purge \
  -H "Content-Type: application/json" \
  -d '{"purge_all": true}'
```

## 监控和日志

### 记录清除操作

```rust
use tracing::{info, warn};

async fn purge_with_logging(
    cache: &TieredCache,
    url: &str,
) -> Result<usize> {
    let start = std::time::Instant::now();
    
    match cache.purge_url(url).await {
        Ok(count) => {
            let elapsed = start.elapsed();
            info!(
                url = %url,
                count = count,
                elapsed_ms = elapsed.as_millis(),
                "缓存清除成功"
            );
            Ok(count)
        }
        Err(e) => {
            warn!(
                url = %url,
                error = %e,
                "缓存清除失败"
            );
            Err(e)
        }
    }
}
```

### 添加指标

```rust
use prometheus::{Counter, Histogram};

lazy_static! {
    static ref CACHE_PURGE_TOTAL: Counter = 
        Counter::new("cache_purge_total", "Total cache purge operations").unwrap();
    
    static ref CACHE_PURGE_ITEMS: Counter = 
        Counter::new("cache_purge_items", "Total cache items purged").unwrap();
    
    static ref CACHE_PURGE_DURATION: Histogram = 
        Histogram::new("cache_purge_duration_seconds", "Cache purge duration").unwrap();
}

async fn purge_with_metrics(
    cache: &TieredCache,
    url: &str,
) -> Result<usize> {
    let timer = CACHE_PURGE_DURATION.start_timer();
    
    let count = cache.purge_url(url).await?;
    
    CACHE_PURGE_TOTAL.inc();
    CACHE_PURGE_ITEMS.inc_by(count as f64);
    timer.observe_duration();
    
    Ok(count)
}
```

## 最佳实践

### ✅ 推荐做法

1. **记录日志**：清除操作应该记录日志，便于追踪
2. **添加指标**：监控清除频率和数量
3. **错误处理**：清除失败不应该影响主流程
4. **批量操作**：大量清除时分批进行
5. **权限控制**：清除 API 应该有权限验证

### ❌ 避免的做法

1. **频繁清除**：不要过于频繁地清除缓存
2. **阻塞操作**：不要在请求处理路径中同步清除
3. **无日志**：清除操作应该有审计日志
4. **无验证**：清除 API 不应该公开访问

## 运行示例

查看完整的示例代码：

```bash
# 运行 purge 示例
cargo run --example tiered_cache_purge_example

# 查看示例代码
cat examples/tiered_cache_purge_example.rs
```

## 故障排查

### 问题：清除后仍然能访问到缓存

**可能原因**：
1. L2 异步删除还未完成
2. 其他实例的缓存未清除（多实例部署）

**解决方案**：
```rust
// 等待 L2 删除完成
cache.purge_url(url).await?;
tokio::time::sleep(Duration::from_millis(100)).await;

// 多实例部署需要广播清除命令
broadcast_purge_to_all_instances(url).await?;
```

### 问题：清除操作很慢

**可能原因**：
1. 清除的项目太多
2. 磁盘 I/O 慢

**解决方案**：
```rust
// 分批清除
let urls = get_urls_to_purge();
for chunk in urls.chunks(100) {
    for url in chunk {
        cache.purge_url(url).await?;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

## 参考

- [两层缓存架构](TIERED_CACHE.md)
- [配置指南](CONFIGURATION.md)
- [API 文档](API.md)

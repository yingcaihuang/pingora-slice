# 两层缓存架构 (Two-Tier Cache)

## 概述

Pingora Slice 现在支持两层缓存架构，结合了内存缓存的速度和磁盘缓存的持久化优势。

### 架构

```
┌─────────────────────────────────────────────────────────┐
│                    Client Request                        │
└────────────────────────┬────────────────────────────────┘
                         │
                         ▼
              ┌──────────────────────┐
              │   L1 Cache (Memory)  │
              │   - Fast access      │
              │   - LRU eviction     │
              │   - Hot data         │
              └──────────┬───────────┘
                         │ miss
                         ▼
              ┌──────────────────────┐
              │   L2 Cache (Disk)    │
              │   - Persistent       │
              │   - Cold data        │
              │   - Survives restart │
              └──────────┬───────────┘
                         │ miss
                         ▼
              ┌──────────────────────┐
              │   Origin Server      │
              └──────────────────────┘
```

## 特性

### L1 缓存（内存）

- ✅ **极快访问**：内存读写，微秒级延迟
- ✅ **LRU 淘汰**：自动淘汰最少使用的数据
- ✅ **热数据优化**：频繁访问的数据保持在内存中
- ✅ **可配置大小**：根据可用内存调整

### L2 缓存（磁盘）

- ✅ **持久化存储**：程序重启后数据仍然存在
- ✅ **异步写入**：不阻塞请求处理
- ✅ **自动恢复**：重启后自动从磁盘加载
- ✅ **分层目录**：避免单目录文件过多

### 智能管理

- ✅ **自动提升**：L2 命中自动提升到 L1
- ✅ **访问追踪**：记录访问频率和时间
- ✅ **过期管理**：自动清理过期数据
- ✅ **统计监控**：详细的缓存统计信息
- ✅ **缓存清除**：支持精确清除特定缓存项

## 配置

### 基本配置

```yaml
# 启用缓存
enable_cache: true

# 缓存 TTL（秒）
cache_ttl: 3600

# L1 内存缓存大小（字节）
l1_cache_size_bytes: 104857600  # 100MB

# 启用 L2 磁盘缓存
enable_l2_cache: true

# L2 缓存目录
l2_cache_dir: "/var/cache/pingora-slice"
```

### 推荐配置

#### 小型部署（< 1000 req/s）

```yaml
l1_cache_size_bytes: 52428800    # 50MB
enable_l2_cache: true
l2_cache_dir: "/var/cache/pingora-slice"
cache_ttl: 3600
```

#### 中型部署（1000-10000 req/s）

```yaml
l1_cache_size_bytes: 262144000   # 250MB
enable_l2_cache: true
l2_cache_dir: "/mnt/ssd/pingora-cache"
cache_ttl: 7200
```

#### 大型部署（> 10000 req/s）

```yaml
l1_cache_size_bytes: 1073741824  # 1GB
enable_l2_cache: true
l2_cache_dir: "/mnt/nvme/pingora-cache"
cache_ttl: 14400
```

## 工作流程

### 读取路径

1. **检查 L1**：首先查找内存缓存
   - 命中 → 立即返回（最快）
   - 未命中 → 继续

2. **检查 L2**：查找磁盘缓存
   - 命中 → 提升到 L1，返回数据
   - 未命中 → 继续

3. **回源**：从源站获取
   - 存储到 L1（同步）
   - 存储到 L2（异步）

### 写入路径

```
数据从源站获取
    ↓
存储到 L1（同步，立即可用）
    ↓
异步写入 L2（后台任务，不阻塞）
```

### 提升机制

当 L2 缓存命中时：

```
L2 命中
    ↓
读取磁盘数据
    ↓
提升到 L1（下次访问更快）
    ↓
返回给客户端
```

### LRU 淘汰

当 L1 缓存满时：

```
新数据需要存储
    ↓
L1 已满？
    ↓ 是
找到最少使用的条目
    ↓
从 L1 移除（仍在 L2 中）
    ↓
存储新数据到 L1
```

## 性能优势

### 对比单层缓存

| 场景 | 单层内存 | 单层磁盘 | 两层缓存 |
|------|---------|---------|---------|
| 热数据访问 | 极快 | 慢 | 极快 |
| 冷数据访问 | 回源 | 快 | 快 |
| 重启后 | 全部回源 | 快 | 快 |
| 内存使用 | 高 | 低 | 中 |
| 持久化 | ❌ | ✅ | ✅ |

### 性能指标

- **L1 命中延迟**：< 1ms
- **L2 命中延迟**：< 10ms（SSD）
- **回源延迟**：50-500ms（取决于网络）

### 缓存命中率提升

```
场景：程序重启

单层内存缓存：
  重启后命中率：0% → 需要时间预热
  
两层缓存：
  重启后命中率：70-90% → L2 立即可用
  几分钟后：90-95% → 热数据提升到 L1
```

## 监控

### 指标

通过 Prometheus 端点（`/metrics`）可以获取：

```
# L1 统计
l1_entries          # L1 中的条目数
l1_bytes            # L1 使用的字节数
l1_hits             # L1 命中次数

# L2 统计
l2_hits             # L2 命中次数（已提升到 L1）
disk_writes         # 成功的磁盘写入次数
disk_errors         # 失败的磁盘写入次数

# 总体统计
misses              # 缓存未命中次数（需要回源）
```

### 计算缓存命中率

```
总命中率 = (l1_hits + l2_hits) / (l1_hits + l2_hits + misses)
L1 命中率 = l1_hits / (l1_hits + l2_hits + misses)
L2 命中率 = l2_hits / (l1_hits + l2_hits + misses)
```

### Grafana 仪表板示例

```promql
# 总体命中率
rate(l1_hits[5m]) + rate(l2_hits[5m]) / 
  (rate(l1_hits[5m]) + rate(l2_hits[5m]) + rate(misses[5m]))

# L1 内存使用率
l1_bytes / l1_cache_size_bytes * 100

# 磁盘写入速率
rate(disk_writes[5m])

# 磁盘错误率
rate(disk_errors[5m]) / rate(disk_writes[5m])
```

## 故障排查

### L2 缓存不工作

**症状**：`disk_writes = 0`，`disk_errors > 0`

**检查**：

```bash
# 检查目录权限
ls -la /var/cache/pingora-slice

# 检查磁盘空间
df -h /var/cache/pingora-slice

# 检查日志
journalctl -u pingora-slice | grep "L2\|disk"
```

**解决**：

```bash
# 修复权限
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chmod 755 /var/cache/pingora-slice

# 清理空间
sudo du -sh /var/cache/pingora-slice/*
sudo rm -rf /var/cache/pingora-slice/old-data
```

### L1 缓存命中率低

**症状**：`l1_hits` 很低，`l2_hits` 很高

**原因**：L1 缓存太小，频繁淘汰

**解决**：

```yaml
# 增加 L1 缓存大小
l1_cache_size_bytes: 524288000  # 从 100MB 增加到 500MB
```

### 磁盘 I/O 过高

**症状**：磁盘 I/O 使用率高

**原因**：L2 写入过于频繁

**解决**：

```yaml
# 选项 1：减少缓存 TTL（更快过期）
cache_ttl: 1800  # 从 1 小时减少到 30 分钟

# 选项 2：增加 L1 大小（减少 L2 访问）
l1_cache_size_bytes: 524288000  # 500MB

# 选项 3：使用更快的存储
l2_cache_dir: "/mnt/nvme/pingora-cache"  # 使用 NVMe SSD
```

## 最佳实践

### 1. 合理设置 L1 大小

```yaml
# 根据可用内存的 10-20% 设置
# 例如：8GB RAM → 800MB-1.6GB L1 cache
l1_cache_size_bytes: 838860800  # 800MB
```

### 2. 使用快速存储作为 L2

```yaml
# 优先级：NVMe SSD > SATA SSD > HDD
l2_cache_dir: "/mnt/nvme/pingora-cache"
```

### 3. 定期清理过期数据

```bash
# 添加 cron 任务清理过期文件
0 2 * * * find /var/cache/pingora-slice -type f -mtime +7 -delete
```

### 4. 监控磁盘使用

```bash
# 设置磁盘使用告警
df -h /var/cache/pingora-slice | awk 'NR==2 {print $5}' | sed 's/%//'
```

### 5. 预热缓存

```bash
# 重启后预热热门内容
curl -s http://localhost:8080/popular-file-1
curl -s http://localhost:8080/popular-file-2
```

## 迁移指南

### 从单层内存缓存迁移

1. **更新配置**：

```yaml
# 添加新配置项
l1_cache_size_bytes: 104857600
enable_l2_cache: true
l2_cache_dir: "/var/cache/pingora-slice"
```

2. **创建缓存目录**：

```bash
sudo mkdir -p /var/cache/pingora-slice
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice
```

3. **重启服务**：

```bash
sudo systemctl restart pingora-slice
```

4. **验证**：

```bash
# 检查 L2 目录有文件写入
ls -la /var/cache/pingora-slice/

# 检查指标
curl http://localhost:9090/metrics | grep -E "l1_|l2_|disk_"
```

## 常见问题

### Q: L1 和 L2 的数据会不一致吗？

A: 不会。L2 是 L1 的超集。所有 L1 的数据都会异步写入 L2。

### Q: 重启后需要多久才能达到最佳性能？

A: 立即可用。L2 数据持久化，重启后直接从 L2 读取，热数据会快速提升到 L1。

### Q: 如果磁盘满了会怎样？

A: L2 写入会失败，但不影响服务。L1 仍然工作，只是重启后会丢失缓存。

### Q: 可以禁用 L2 只使用 L1 吗？

A: 可以。设置 `enable_l2_cache: false` 即可。

### Q: L2 缓存会占用多少磁盘空间？

A: 取决于流量和 TTL。建议预留至少 10GB，大流量场景建议 50-100GB。

## 缓存清除 (Cache Purge)

两层缓存支持灵活的缓存清除操作，可以精确控制要删除的缓存内容。

### 清除方法

#### 1. 清除单个缓存项

删除特定 URL 和字节范围的缓存：

```rust
use pingora_slice::tiered_cache::TieredCache;
use pingora_slice::models::ByteRange;

// 清除特定的缓存切片
let url = "http://example.com/video.mp4";
let range = ByteRange::new(0, 1048575)?; // 0-1MB
let purged = cache.purge(url, &range).await?;

if purged {
    println!("缓存已清除");
} else {
    println!("缓存不存在");
}
```

#### 2. 清除 URL 的所有切片

删除某个 URL 的所有缓存切片：

```rust
// 清除整个文件的所有切片
let url = "http://example.com/largefile.bin";
let count = cache.purge_url(url).await?;
println!("清除了 {} 个缓存项", count);
```

#### 3. 清除所有缓存

清空整个缓存：

```rust
// 清除所有缓存数据
let count = cache.purge_all().await?;
println!("清除了 {} 个缓存项", count);
```

### 清除行为

- **L1 清除**：立即从内存中删除
- **L2 清除**：异步从磁盘删除（不阻塞）
- **原子性**：L1 清除是原子操作
- **幂等性**：重复清除不会报错

### 使用场景

#### 场景 1：内容更新

当源站内容更新时，清除旧缓存：

```rust
// 源站文件更新后
cache.purge_url("http://cdn.example.com/app.js").await?;
```

#### 场景 2：紧急下线

需要紧急下线某个文件：

```rust
// 立即清除所有相关缓存
cache.purge_url("http://cdn.example.com/sensitive-data.pdf").await?;
```

#### 场景 3：批量清理

定期清理特定模式的缓存：

```rust
// 清理所有视频文件缓存
let video_urls = get_video_urls();
for url in video_urls {
    cache.purge_url(&url).await?;
}
```

#### 场景 4：维护模式

进入维护模式前清空缓存：

```rust
// 清空所有缓存
cache.purge_all().await?;
```

### HTTP API 集成

可以通过 HTTP API 暴露清除功能：

```rust
// 示例：添加 purge 端点
async fn handle_purge(req: Request) -> Result<Response> {
    let url = req.query_param("url")?;
    
    if let Some(url) = url {
        // 清除特定 URL
        let count = cache.purge_url(&url).await?;
        Ok(Response::json(json!({
            "status": "success",
            "purged": count,
            "url": url
        })))
    } else {
        // 清除所有
        let count = cache.purge_all().await?;
        Ok(Response::json(json!({
            "status": "success",
            "purged": count
        })))
    }
}
```

### 性能考虑

- **L1 清除**：O(1) 时间复杂度，非常快
- **L2 清除**：异步执行，不影响请求处理
- **批量清除**：建议分批进行，避免一次清除过多

### 监控清除操作

建议记录清除操作的日志和指标：

```rust
// 记录清除操作
tracing::info!(
    url = %url,
    purged = count,
    "Cache purged"
);

// 更新指标
metrics.cache_purge_total.inc();
metrics.cache_purge_items.add(count as i64);
```

### 示例代码

完整示例请参考：`examples/tiered_cache_purge_example.rs`

```bash
# 运行示例
cargo run --example tiered_cache_purge_example
```

## 参考

- [配置指南](CONFIGURATION.md)
- [性能调优](PERFORMANCE_TUNING.md)
- [部署指南](DEPLOYMENT.md)

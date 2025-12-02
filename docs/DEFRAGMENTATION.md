# Defragmentation 碎片整理

## 概述

碎片整理功能用于检测和减少 Raw Disk Cache 中的磁盘碎片，提高空间利用率和性能。

## 什么是碎片化？

当缓存条目被删除后，会在磁盘上留下空隙（gaps）。新的条目可能会被分配到这些空隙中，但如果空隙太小或分布不均，就会导致：

1. **空间浪费**：许多小空隙无法被有效利用
2. **性能下降**：数据分散在磁盘各处，增加寻道时间
3. **分配失败**：虽然总空闲空间足够，但没有足够大的连续空间

## 碎片率计算

碎片率的计算公式：

```
fragmentation = (total_gap_space - largest_gap) / total_used_space
```

- `total_gap_space`: 所有空隙的总大小
- `largest_gap`: 最大空隙的大小（一个大空隙不算碎片）
- `total_used_space`: 已使用空间的总大小

碎片率范围：0.0 - 1.0
- 0.0: 无碎片（数据连续或只有一个大空隙）
- 1.0: 严重碎片（许多小空隙）

## 配置

### DefragConfig

```rust
pub struct DefragConfig {
    /// 触发碎片整理的阈值 (0.0-1.0)
    pub fragmentation_threshold: f64,
    
    /// 每次整理移动的最大条目数
    pub batch_size: usize,
    
    /// 是否增量整理
    pub incremental: bool,
    
    /// 执行整理所需的最小空闲空间比例
    pub min_free_space_ratio: f64,
    
    /// 目标压缩比例
    pub target_compaction_ratio: f64,
}
```

### 默认配置

```rust
DefragConfig {
    fragmentation_threshold: 0.3,    // 30% 碎片率触发
    batch_size: 100,                 // 每批移动 100 个条目
    incremental: true,               // 增量整理
    min_free_space_ratio: 0.15,      // 需要至少 15% 空闲空间
    target_compaction_ratio: 0.95,   // 目标 95% 压缩率
}
```

## 使用方法

### 1. 检测碎片率

```rust
// 获取当前碎片率
let frag_ratio = cache.fragmentation_ratio().await;
println!("Fragmentation: {:.2}%", frag_ratio * 100.0);

// 检查是否应该执行整理
let should_defrag = cache.should_defragment().await;
if should_defrag {
    println!("Defragmentation recommended");
}
```

### 2. 手动执行整理

```rust
// 同步执行整理
let moved = cache.defragment().await?;
println!("Moved {} entries", moved);
```

### 3. 后台执行整理

```rust
// 异步后台执行
cache.defragment_background().await;
// 不会阻塞，立即返回
```

### 4. 配置整理参数

```rust
let config = DefragConfig {
    fragmentation_threshold: 0.2,  // 20% 触发
    batch_size: 50,                // 小批量
    incremental: true,
    min_free_space_ratio: 0.1,
    target_compaction_ratio: 0.9,
};

cache.update_defrag_config(config).await;
```

### 5. 查看统计信息

```rust
let stats = cache.defrag_stats().await;
println!("Total runs: {}", stats.total_runs);
println!("Total entries moved: {}", stats.total_entries_moved);
println!("Total bytes moved: {}", stats.total_bytes_moved);
println!("Total duration: {:?}", stats.total_duration);
println!("Failed moves: {}", stats.failed_moves);

if let Some(last_run) = stats.last_run {
    println!("Last run: {:?} ago", last_run.elapsed());
    println!("Fragmentation: {:.2}% -> {:.2}%",
             stats.last_fragmentation_before * 100.0,
             stats.last_fragmentation_after * 100.0);
}
```

## 整理策略

### 移动策略

碎片整理采用"从后向前"的策略：

1. 识别磁盘前部的空隙
2. 选择磁盘后部的条目
3. 将后部条目移动到前部空隙
4. 释放后部空间

这样可以：
- 将数据压缩到磁盘前部
- 在磁盘后部形成大的连续空闲空间
- 减少碎片，提高分配效率

### 增量 vs 全量整理

**增量整理** (incremental = true):
- 分批处理，每批移动 `batch_size` 个条目
- 在批次之间让出 CPU，不阻塞其他操作
- 适合在线服务，对性能影响小
- 可能需要多次运行才能完全整理

**全量整理** (incremental = false):
- 一次性处理所有需要移动的条目
- 可能阻塞较长时间
- 整理效果更彻底
- 适合离线维护或低负载时段

## 性能考虑

### 何时触发整理

建议在以下情况触发整理：

1. **定期检查**：每隔一段时间检查碎片率
2. **分配失败后**：当分配失败但有足够总空间时
3. **低负载时段**：在流量低谷期执行
4. **GC 之后**：垃圾回收后可能产生碎片

### 性能影响

整理过程会：
- 读取旧位置的数据
- 写入新位置
- 更新元数据

影响因素：
- 移动的条目数量
- 条目大小
- 磁盘 I/O 性能
- 是否使用增量模式

### 优化建议

1. **合理设置阈值**：
   - 太低：频繁整理，浪费资源
   - 太高：碎片严重，影响性能
   - 建议：0.2 - 0.3

2. **选择合适的批量大小**：
   - 太小：整理次数多，开销大
   - 太大：单次阻塞时间长
   - 建议：50 - 200

3. **使用增量模式**：
   - 在线服务应使用增量模式
   - 离线维护可使用全量模式

4. **在低负载时段执行**：
   - 避免在高峰期整理
   - 可以配合定时任务

## 监控指标

### 关键指标

1. **碎片率** (fragmentation_ratio)
   - 当前碎片化程度
   - 建议保持在 30% 以下

2. **整理次数** (total_runs)
   - 总共执行的整理次数
   - 频繁整理可能需要调整阈值

3. **移动条目数** (total_entries_moved)
   - 总共移动的条目数
   - 反映整理工作量

4. **失败次数** (failed_moves)
   - 移动失败的次数
   - 应该接近 0，否则需要调查

### 告警建议

```yaml
# Prometheus 告警规则示例
- alert: HighFragmentation
  expr: raw_disk_cache_fragmentation_ratio > 0.5
  for: 1h
  annotations:
    summary: "High cache fragmentation"
    description: "Fragmentation ratio is {{ $value }}"

- alert: FrequentDefragmentation
  expr: rate(raw_disk_cache_defrag_runs_total[1h]) > 0.1
  annotations:
    summary: "Frequent defragmentation"
    description: "Defragmentation running too often"

- alert: DefragmentationFailures
  expr: rate(raw_disk_cache_defrag_failed_moves_total[5m]) > 0
  annotations:
    summary: "Defragmentation failures detected"
```

## 最佳实践

### 1. 定期监控

```rust
// 定期检查碎片率
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        
        let frag = cache.fragmentation_ratio().await;
        if frag > 0.3 {
            warn!("High fragmentation: {:.2}%", frag * 100.0);
            
            if cache.should_defragment().await {
                cache.defragment_background().await;
            }
        }
    }
});
```

### 2. 与 GC 配合

```rust
// GC 后检查是否需要整理
cache.run_smart_gc().await?;

if cache.should_defragment().await {
    cache.defragment_background().await;
}
```

### 3. 优雅关闭

```rust
// 关闭前执行整理
async fn shutdown(cache: &RawDiskCache) -> Result<()> {
    info!("Running final defragmentation before shutdown");
    cache.defragment().await?;
    cache.save_metadata().await?;
    Ok(())
}
```

### 4. 负载感知

```rust
// 根据负载决定是否整理
async fn maybe_defragment(cache: &RawDiskCache, load: f64) {
    if load < 0.3 && cache.should_defragment().await {
        // 低负载时执行
        cache.defragment().await.ok();
    } else if cache.fragmentation_ratio().await > 0.5 {
        // 碎片严重时后台执行
        cache.defragment_background().await;
    }
}
```

## 故障排查

### 问题：整理后碎片率没有明显下降

**可能原因**：
1. 空闲空间不足，无法移动条目
2. 条目大小不匹配空隙大小
3. 批量大小太小，只处理了部分条目

**解决方案**：
- 增加 `min_free_space_ratio`
- 使用全量模式
- 增加 `batch_size`

### 问题：整理过程中出现失败

**可能原因**：
1. 磁盘 I/O 错误
2. 空间不足
3. 数据校验失败

**解决方案**：
- 检查 `failed_moves` 计数
- 查看日志中的错误信息
- 验证磁盘健康状态

### 问题：整理影响性能

**可能原因**：
1. 批量大小太大
2. 未使用增量模式
3. 在高负载时段执行

**解决方案**：
- 减小 `batch_size`
- 启用 `incremental` 模式
- 在低负载时段执行

## 示例代码

完整示例请参考：`examples/defrag_example.rs`

```bash
cargo run --example defrag_example
```

## 参考

- [Raw Disk Cache 设计文档](RAW_DISK_CACHE_DESIGN.md)
- [Smart GC 文档](SMART_GC.md)
- [性能调优指南](PERFORMANCE_TUNING.md)

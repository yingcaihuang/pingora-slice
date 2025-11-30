# Raw Disk Cache 设计方案

## 概述

本文档描述了类似 Apache Traffic Server 的 raw disk 缓存方案，直接在裸设备或块设备上管理缓存，不依赖文件系统。

## 背景

### 当前方案的局限性

当前的两层缓存方案使用文件系统存储 L2 缓存：
- **优点**: 实现简单，易于调试，可以使用标准工具查看
- **缺点**: 
  - 文件系统开销（inode、目录结构、元数据）
  - 小文件性能问题
  - 文件系统碎片化
  - 无法精确控制磁盘空间使用
  - 受文件系统限制（文件数量、大小等）

### Apache Traffic Server Raw Disk 方案

ATS 使用 raw disk 方案的优势：
- **零文件系统开销**: 直接管理磁盘块
- **精确的空间控制**: 可以精确分配和回收空间
- **更好的性能**: 减少系统调用和上下文切换
- **避免碎片化**: 自己管理空间分配
- **更高的并发**: 不受文件系统锁的限制

## 可行性分析

### ✅ 技术可行性

1. **Rust 生态支持**
   - `nix` crate: 提供底层系统调用
   - `libc` crate: 直接访问 POSIX API
   - `tokio::fs::File`: 支持异步 I/O
   - `io_uring`: 高性能异步 I/O（Linux）

2. **平台支持**
   - Linux: 完全支持，可以使用 O_DIRECT
   - macOS: 支持，但需要特殊处理
   - 可以使用块设备或大文件模拟

3. **性能优势**
   - 减少系统调用
   - 避免页缓存双重缓存
   - 更好的 I/O 调度控制
   - 支持 DMA 直接内存访问

### ⚠️ 挑战和考虑

1. **复杂性增加**
   - 需要实现自己的空间管理
   - 需要处理数据一致性
   - 需要实现垃圾回收
   - 需要处理崩溃恢复

2. **调试困难**
   - 无法直接查看缓存内容
   - 需要专门的工具
   - 错误更难定位

3. **运维复杂度**
   - 需要预分配磁盘空间
   - 需要专门的监控工具
   - 备份和恢复更复杂

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    L1 Cache (Memory)                     │
│                     LRU + HashMap                        │
└────────────────────┬────────────────────────────────────┘
                     │
                     ↓
┌─────────────────────────────────────────────────────────┐
│              L2 Cache (Raw Disk)                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │           Cache Directory (Metadata)              │  │
│  │  - Hash Index (Key → Disk Location)              │  │
│  │  - Free Space Bitmap                             │  │
│  │  - LRU List                                      │  │
│  └──────────────────────────────────────────────────┘  │
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Storage Manager (Data)                    │  │
│  │  - Block Allocator                               │  │
│  │  - Write Buffer                                  │  │
│  │  - Read Cache                                    │  │
│  └──────────────────────────────────────────────────┘  │
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Disk I/O Layer                           │  │
│  │  - Direct I/O (O_DIRECT)                        │  │
│  │  - Async I/O (io_uring/tokio)                  │  │
│  │  - Alignment Handling                           │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────┬───────────────────────────────────┘
                      │
                      ↓
              ┌───────────────┐
              │  Raw Device   │
              │  /dev/sdb or  │
              │  cache.disk   │
              └───────────────┘
```

### 核心组件

#### 1. Cache Directory (元数据管理)

```rust
struct CacheDirectory {
    // 哈希索引: Key → DiskLocation
    index: HashMap<CacheKey, DiskLocation>,
    
    // 空闲空间位图
    free_blocks: BitVec,
    
    // LRU 链表
    lru: LinkedList<CacheKey>,
    
    // 统计信息
    stats: CacheStats,
}

struct DiskLocation {
    offset: u64,      // 磁盘偏移
    size: u32,        // 数据大小
    checksum: u32,    // 校验和
    timestamp: u64,   // 时间戳
}
```

#### 2. Block Allocator (空间分配器)

```rust
struct BlockAllocator {
    block_size: usize,        // 块大小 (4KB)
    total_blocks: usize,      // 总块数
    free_blocks: BitVec,      // 空闲块位图
    allocation_strategy: AllocationStrategy,
}

enum AllocationStrategy {
    FirstFit,    // 首次适应
    BestFit,     // 最佳适应
    NextFit,     // 循环首次适应
}
```

#### 3. Disk I/O Manager (磁盘 I/O 管理)

```rust
struct DiskIOManager {
    file: File,              // 磁盘文件/设备
    alignment: usize,        // 对齐要求 (512/4096)
    use_direct_io: bool,     // 是否使用 O_DIRECT
    write_buffer: WriteBuffer,
    read_cache: ReadCache,
}
```

## 数据布局

### 磁盘布局

```
┌─────────────────────────────────────────────────────────┐
│  Superblock (4KB)                                        │
│  - Magic Number                                          │
│  - Version                                               │
│  - Block Size                                            │
│  - Total Size                                            │
│  - Metadata Offset                                       │
├─────────────────────────────────────────────────────────┤
│  Metadata Region (Variable)                              │
│  - Cache Directory                                       │
│  - Free Block Bitmap                                     │
│  - LRU List                                              │
├─────────────────────────────────────────────────────────┤
│  Data Region (Majority of disk)                          │
│  ┌───────────────────────────────────────────────────┐  │
│  │ Block 0: [Header | Data | Padding]                │  │
│  ├───────────────────────────────────────────────────┤  │
│  │ Block 1: [Header | Data | Padding]                │  │
│  ├───────────────────────────────────────────────────┤  │
│  │ ...                                                │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 数据块格式

```
┌─────────────────────────────────────────────────────────┐
│  Block Header (64 bytes)                                 │
│  - Magic: u32                                            │
│  - Key Hash: u64                                         │
│  - Data Size: u32                                        │
│  - Checksum: u32                                         │
│  - Timestamp: u64                                        │
│  - Next Block: u64 (for large objects)                  │
│  - Reserved: [u8; 28]                                    │
├─────────────────────────────────────────────────────────┤
│  Data (Variable)                                         │
│  - Actual cache data                                     │
├─────────────────────────────────────────────────────────┤
│  Padding (to block boundary)                             │
└─────────────────────────────────────────────────────────┘
```

## 关键算法

### 1. 写入流程

```rust
async fn write_to_disk(key: &str, data: &[u8]) -> Result<()> {
    // 1. 计算需要的块数
    let blocks_needed = calculate_blocks(data.len());
    
    // 2. 分配空间
    let location = allocator.allocate(blocks_needed)?;
    
    // 3. 准备数据（对齐）
    let aligned_data = align_data(data);
    
    // 4. 写入磁盘（使用 O_DIRECT）
    disk.write_at(location.offset, &aligned_data).await?;
    
    // 5. 更新元数据
    directory.insert(key, location);
    
    // 6. 持久化元数据
    directory.sync().await?;
    
    Ok(())
}
```

### 2. 读取流程

```rust
async fn read_from_disk(key: &str) -> Result<Option<Vec<u8>>> {
    // 1. 查找元数据
    let location = directory.lookup(key)?;
    
    // 2. 读取数据
    let data = disk.read_at(location.offset, location.size).await?;
    
    // 3. 验证校验和
    verify_checksum(&data, location.checksum)?;
    
    // 4. 更新 LRU
    directory.touch(key);
    
    Ok(Some(data))
}
```

### 3. 垃圾回收

```rust
async fn garbage_collect() -> Result<()> {
    // 1. 选择要淘汰的对象（LRU）
    let victims = directory.select_victims(target_size);
    
    // 2. 标记空间为空闲
    for victim in victims {
        allocator.free(victim.location);
        directory.remove(&victim.key);
    }
    
    // 3. 可选：碎片整理
    if fragmentation_ratio() > threshold {
        defragment().await?;
    }
    
    Ok(())
}
```

## 性能优化

### 1. 批量 I/O

```rust
struct WriteBatch {
    operations: Vec<WriteOp>,
    total_size: usize,
}

// 批量提交写入
async fn flush_batch(batch: WriteBatch) -> Result<()> {
    // 使用 io_uring 批量提交
    let mut ring = IoUring::new(batch.operations.len())?;
    
    for op in batch.operations {
        ring.submit_write(op.offset, &op.data)?;
    }
    
    ring.submit_and_wait_all().await?;
    Ok(())
}
```

### 2. 预读取

```rust
async fn prefetch(keys: &[String]) -> Result<()> {
    let locations: Vec<_> = keys.iter()
        .filter_map(|k| directory.lookup(k).ok())
        .collect();
    
    // 按磁盘位置排序，优化寻道
    locations.sort_by_key(|loc| loc.offset);
    
    // 批量预读
    for loc in locations {
        disk.prefetch(loc.offset, loc.size).await?;
    }
    
    Ok(())
}
```

### 3. 零拷贝

```rust
// 使用 mmap 或 sendfile 实现零拷贝
async fn zero_copy_read(key: &str, socket: &TcpStream) -> Result<()> {
    let location = directory.lookup(key)?;
    
    // 直接从磁盘发送到 socket
    sendfile(socket.as_raw_fd(), 
             disk.as_raw_fd(), 
             location.offset, 
             location.size)?;
    
    Ok(())
}
```

## 实现路线图

### Phase 1: 基础实现 (2-3 周)

- [ ] 实现 Superblock 和基础数据结构
- [ ] 实现 Block Allocator
- [ ] 实现基础的读写操作
- [ ] 单元测试

### Phase 2: 元数据管理 (2 周)

- [ ] 实现 Cache Directory
- [ ] 实现哈希索引
- [ ] 实现 LRU 管理
- [ ] 元数据持久化

### Phase 3: 高级特性 (2-3 周)

- [ ] 实现垃圾回收
- [ ] 实现碎片整理
- [ ] 实现崩溃恢复
- [ ] 性能优化

### Phase 4: 集成和测试 (1-2 周)

- [ ] 集成到现有缓存系统
- [ ] 性能测试和调优
- [ ] 压力测试
- [ ] 文档完善

## 配置示例

```yaml
cache:
  l1:
    size: 10GB
    type: memory
  
  l2:
    type: raw_disk
    device: /dev/sdb  # 或 /var/cache/pingora/cache.disk
    size: 100GB
    block_size: 4096
    direct_io: true
    
    # 空间管理
    allocation_strategy: next_fit
    gc_threshold: 0.9  # 90% 满时触发 GC
    
    # 性能调优
    write_buffer_size: 64MB
    read_cache_size: 128MB
    io_depth: 128  # io_uring 队列深度
    
    # 可靠性
    checksum: true
    sync_interval: 5s
```

## 性能预期

### 对比文件系统方案

| 指标 | 文件系统 | Raw Disk | 提升 |
|------|---------|----------|------|
| 小文件写入 | 5K ops/s | 20K ops/s | 4x |
| 小文件读取 | 10K ops/s | 40K ops/s | 4x |
| 大文件写入 | 500 MB/s | 800 MB/s | 1.6x |
| 大文件读取 | 800 MB/s | 1.2 GB/s | 1.5x |
| 空间利用率 | 70-80% | 95%+ | 1.2x |
| 延迟 (p99) | 10ms | 2ms | 5x |

## 风险和缓解

### 风险 1: 数据丢失

**缓解措施**:
- 实现 WAL (Write-Ahead Log)
- 定期 checkpoint
- 校验和验证

### 风险 2: 性能不达预期

**缓解措施**:
- 详细的性能测试
- 可配置的回退到文件系统
- 渐进式迁移

### 风险 3: 运维复杂度

**缓解措施**:
- 提供管理工具
- 详细的文档
- 监控和告警

## 结论

Raw disk 缓存方案是**完全可行**的，并且能带来显著的性能提升。主要优势：

✅ **性能**: 4-5x 小文件性能提升，1.5-2x 大文件提升
✅ **空间利用率**: 95%+ vs 70-80%
✅ **可控性**: 精确的空间和性能控制
✅ **扩展性**: 更好的并发和吞吐量

建议采用**渐进式实现**策略：
1. 先实现基础功能，验证可行性
2. 与现有文件系统方案并存，可配置切换
3. 充分测试后再作为默认方案
4. 保留文件系统方案作为备选

这个方案特别适合：
- 高并发场景
- 大量小文件缓存
- 对性能要求极高的场景
- 需要精确控制磁盘使用的场景

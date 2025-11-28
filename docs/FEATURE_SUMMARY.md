# Pingora Slice 功能总结

## 最新更新

本文档总结了 Pingora Slice 的所有主要功能，包括最新添加的两层缓存和 HTTP PURGE 支持。

## 核心功能

### 1. 自动分片 (Automatic Slicing)
- 透明地将大文件请求拆分为小的 Range 请求
- 可配置的分片大小（64KB - 10MB）
- 基于 URL 模式的智能分片决策
- 支持客户端 Range 请求

### 2. 并发处理 (Concurrent Processing)
- 并行获取多个分片
- 可配置的并发限制
- 智能的请求调度
- 自动重试机制（指数退避）

### 3. 两层缓存系统 (Two-Tier Cache) ⭐ 新功能

#### L1 内存缓存
- **速度**：微秒级访问
- **容量**：可配置（默认 100MB）
- **策略**：LRU 淘汰
- **用途**：热数据快速访问

#### L2 磁盘缓存
- **持久化**：服务重启后数据保留
- **容量**：可配置（默认 10GB）
- **异步**：非阻塞磁盘操作
- **用途**：冷数据持久化存储

#### 缓存特性
- ✅ 自动提升：L2 命中自动提升到 L1
- ✅ 智能淘汰：LRU 策略
- ✅ TTL 过期：自动清理过期数据
- ✅ 统计信息：详细的缓存指标

### 4. HTTP PURGE 支持 ⭐ 新功能

#### 清除方法
- **单个 URL**：`PURGE /path/to/file`
- **全部缓存**：`PURGE /*` + `X-Purge-All: true`
- **前缀匹配**：`PURGE /path/*` + `X-Purge-Pattern: prefix`

#### 安全特性
- 基于令牌的认证（Bearer Token）
- 可选的认证机制
- 审计日志记录

#### 监控指标
- 清除请求总数
- 清除成功/失败率
- 清除的项目数量
- 清除操作持续时间
- 认证失败统计

### 5. 监控和可观测性

#### Prometheus 指标
- **缓存指标**：L1/L2 命中率、大小、淘汰数
- **分片指标**：请求数、字节数、延迟
- **清除指标**：操作数、成功率、持续时间
- **性能指标**：吞吐量、延迟分布

#### 日志记录
- 结构化日志（tracing）
- 可配置的日志级别
- 详细的操作追踪

## 配置概览

### 完整配置示例

```yaml
# 分片配置
slice_size: 1048576                    # 1MB
max_concurrent_subrequests: 4
max_retries: 3
slice_patterns:
  - "^/large-files/.*"

# 缓存配置
enable_cache: true
cache_ttl: 3600                        # 1 hour

# L1 内存缓存
l1_cache_size_bytes: 104857600         # 100MB

# L2 磁盘缓存
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true

# 上游服务器
upstream_address: "origin.example.com:80"

# Prometheus 指标
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"

# HTTP PURGE
purge:
  enabled: true
  auth_token: "your-secret-token"
  enable_metrics: true
```

## 使用场景

### 场景 1：大文件分发
- **问题**：大文件下载占用大量带宽和内存
- **解决方案**：自动分片 + 两层缓存
- **效果**：减少源站负载，提高缓存效率

### 场景 2：视频点播
- **问题**：视频文件大，用户经常跳转
- **解决方案**：分片 + Range 请求支持
- **效果**：只获取需要的部分，节省带宽

### 场景 3：软件分发
- **问题**：ISO/安装包文件大，下载失败需重新开始
- **解决方案**：分片 + 重试机制 + 持久化缓存
- **效果**：失败重试只需重新获取失败的分片

### 场景 4：内容更新
- **问题**：内容更新后需要清除旧缓存
- **解决方案**：HTTP PURGE 方法
- **效果**：即时清除缓存，无需重启服务

## 性能特点

### 延迟
- **L1 缓存命中**：< 1ms
- **L2 缓存命中**：< 10ms（SSD）
- **源站获取**：50-500ms（取决于网络）

### 吞吐量
- **并发处理**：支持数千并发连接
- **分片并行**：可配置并发数（默认 4）
- **缓存效率**：90%+ 命中率（稳定状态）

### 资源使用
- **内存**：L1 缓存大小 + 运行时开销（~50MB）
- **磁盘**：L2 缓存大小（可配置）
- **CPU**：低开销，主要用于数据复制

## 部署架构

### 单机部署
```
Client → Pingora Slice → Origin
         ↓
         L1 (Memory) + L2 (Disk)
```

### 多实例部署
```
Client → Load Balancer → Pingora Slice 1 → Origin
                       → Pingora Slice 2 → Origin
                       → Pingora Slice 3 → Origin
                       
每个实例有独立的 L1 + L2 缓存
```

### CDN 边缘部署
```
Client → Edge (Pingora Slice) → Regional (Pingora Slice) → Origin
         L1 + L2                 L1 + L2
```

## 最佳实践

### 缓存配置
1. **L1 大小**：根据可用内存设置（建议 10-20% 的可用内存）
2. **L2 大小**：根据磁盘空间和 TTL 设置
3. **TTL**：根据内容更新频率设置（静态内容可设置更长）

### 分片配置
1. **分片大小**：1-4MB 适合大多数场景
2. **并发数**：根据源站能力设置（4-8 为佳）
3. **URL 模式**：只对大文件启用分片

### 安全配置
1. **PURGE 认证**：生产环境必须启用
2. **令牌管理**：定期轮换认证令牌
3. **访问控制**：限制 PURGE 请求来源

### 监控配置
1. **指标收集**：启用 Prometheus 指标
2. **告警设置**：监控缓存命中率、错误率
3. **日志级别**：生产环境使用 INFO 或 WARN

## 故障排查

### 缓存问题
- **症状**：缓存命中率低
- **检查**：查看 L1/L2 大小配置、TTL 设置
- **解决**：增加缓存大小或调整 TTL

### 性能问题
- **症状**：响应慢
- **检查**：查看并发数、分片大小
- **解决**：调整并发数或分片大小

### PURGE 问题
- **症状**：PURGE 请求失败
- **检查**：认证令牌、配置是否启用
- **解决**：检查令牌、确认配置正确

## 版本历史

### v0.1.0（当前版本）
- ✅ 自动分片功能
- ✅ 并发获取
- ✅ 基本缓存
- ✅ Range 请求支持
- ✅ Prometheus 指标
- ✅ 两层缓存系统（L1 + L2）
- ✅ HTTP PURGE 支持
- ✅ 完整的配置系统

## 相关文档

- [README.md](../README.md) - 项目主文档
- [README_zh.md](../README_zh.md) - 中文主文档
- [TIERED_CACHE.md](TIERED_CACHE.md) - 两层缓存详细说明
- [PURGE_QUICK_START.md](PURGE_QUICK_START.md) - PURGE 快速开始
- [PURGE_CONFIG_AND_METRICS.md](PURGE_CONFIG_AND_METRICS.md) - PURGE 配置和指标
- [CACHE_PURGE_zh.md](CACHE_PURGE_zh.md) - 缓存清除中文指南

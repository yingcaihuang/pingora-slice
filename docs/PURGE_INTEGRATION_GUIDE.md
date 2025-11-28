# Purge 功能集成指南

## 概述

**重要说明**：Purge 功能是 Pingora Slice 模块的一部分，不是独立的服务。当你启用 pingora-slice 时，只需在配置文件中启用 purge 功能，就可以使用 HTTP PURGE 方法来清除缓存。

## 快速开始

### 1. 配置文件

在你的 `pingora_slice.yaml` 中添加 purge 配置：

```yaml
# 基本的 Pingora Slice 配置
slice_size: 1048576
max_concurrent_subrequests: 4
enable_cache: true
cache_ttl: 3600
upstream_address: "origin.example.com:80"

# 启用 Purge 功能（添加这个配置块）
purge:
  enabled: true                          # 启用 PURGE
  auth_token: "your-secret-token-here"   # 认证令牌
  enable_metrics: true                   # 启用指标
```

### 2. 启动 Pingora Slice

```bash
# 使用配置文件启动
./pingora-slice -c pingora_slice.yaml
```

### 3. 使用 PURGE 方法

一旦 pingora-slice 启动，你就可以直接发送 PURGE 请求：

```bash
# 清除特定 URL
curl -X PURGE http://your-server.com/path/to/file.dat \
  -H "Authorization: Bearer your-secret-token-here"

# 清除所有缓存
curl -X PURGE http://your-server.com/* \
  -H "X-Purge-All: true" \
  -H "Authorization: Bearer your-secret-token-here"
```

## 架构说明

```
┌─────────────────────────────────────────────────────────┐
│              Pingora Slice 服务                          │
│                                                          │
│  ┌────────────────────┐      ┌────────────────────┐    │
│  │  Slice 处理        │      │  Purge 处理器      │    │
│  │  - 切片请求        │      │  - HTTP PURGE      │    │
│  │  - 缓存管理        │◄────►│  - 缓存清除        │    │
│  │  - 响应组装        │      │  - 指标收集        │    │
│  └────────────────────┘      └────────────────────┘    │
│           │                           │                 │
│           └───────────┬───────────────┘                 │
│                       ▼                                 │
│           ┌────────────────────┐                        │
│           │   两层缓存系统      │                        │
│           │   L1: 内存          │                        │
│           │   L2: 磁盘          │                        │
│           └────────────────────┘                        │
└─────────────────────────────────────────────────────────┘
```

## 集成方式

Purge 功能通过以下方式集成到 Pingora Slice 中：

### 方式 1：在 Proxy 中集成（推荐）

在你的 Pingora Slice proxy 实现中添加 PURGE 处理：

```rust
use pingora_slice::config::SliceConfig;
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::purge_metrics::PurgeMetrics;
use pingora_slice::tiered_cache::TieredCache;

pub struct SliceProxy {
    cache: Arc<TieredCache>,
    purge_handler: Option<Arc<PurgeHandler>>,
    // ... 其他字段
}

impl SliceProxy {
    pub async fn new(config: Arc<SliceConfig>) -> Result<Self> {
        // 创建缓存
        let cache = Arc::new(TieredCache::new(...).await?);
        
        // 如果配置中启用了 purge，创建处理器
        let purge_handler = if let Some(purge_config) = &config.purge {
            if purge_config.enabled {
                let metrics = if purge_config.enable_metrics {
                    Some(Arc::new(PurgeMetrics::new()?))
                } else {
                    None
                };
                
                let mut handler = if let Some(token) = &purge_config.auth_token {
                    PurgeHandler::with_auth(cache.clone(), token.clone())
                } else {
                    PurgeHandler::new(cache.clone())
                };
                
                if let Some(m) = metrics {
                    handler = handler.with_metrics(m);
                }
                
                Some(Arc::new(handler))
            } else {
                None
            }
        } else {
            None
        };
        
        Ok(Self {
            cache,
            purge_handler,
            // ...
        })
    }
}

// 在请求处理中检查 PURGE 方法
impl ProxyHttp for SliceProxy {
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        // 检查是否是 PURGE 请求
        if session.req_header().method == "PURGE" {
            if let Some(handler) = &self.purge_handler {
                // 处理 PURGE 请求
                let response = handler.handle_purge(session.req_header()).await?;
                // 发送响应
                session.write_response_header(response).await?;
                return Ok(true); // 请求已处理
            }
        }
        
        // 继续正常的 slice 处理
        Ok(false)
    }
}
```

### 方式 2：作为独立端点（可选）

如果你想要一个专门的 PURGE 管理端点：

```rust
// 在单独的端口上运行 PURGE 管理服务
async fn start_purge_admin_server(
    cache: Arc<TieredCache>,
    config: &PurgeConfig,
) -> Result<()> {
    let purge_handler = PurgeHandler::with_auth(
        cache,
        config.auth_token.clone().unwrap_or_default()
    );
    
    // 绑定到管理端口（例如 8081）
    let addr = "127.0.0.1:8081".parse()?;
    // ... 启动 HTTP 服务器
}
```

## 完整示例

### 配置文件 (pingora_slice.yaml)

```yaml
# Pingora Slice 完整配置
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
enable_cache: true
cache_ttl: 3600
l1_cache_size_bytes: 104857600  # 100MB
l2_cache_dir: "/var/cache/pingora-slice"
enable_l2_cache: true
upstream_address: "origin.example.com:80"

# Metrics 端点
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"

# Purge 配置（集成在 Slice 中）
purge:
  enabled: true
  auth_token: "my-secret-purge-token-2024"
  enable_metrics: true
```

### 启动服务

```bash
# 1. 启动 Pingora Slice（包含 Purge 功能）
./pingora-slice -c pingora_slice.yaml

# 2. 服务现在同时支持：
#    - 正常的 GET 请求（slice 处理）
#    - PURGE 请求（缓存清除）
#    - /metrics 端点（Prometheus 指标）
```

### 使用示例

```bash
# 正常请求（会被 slice 处理）
curl http://your-server.com/large-file.iso

# 清除缓存（使用 PURGE 方法）
curl -X PURGE http://your-server.com/large-file.iso \
  -H "Authorization: Bearer my-secret-purge-token-2024"

# 查看指标（包括 purge 指标）
curl http://your-server.com:9090/metrics | grep purge
```

## 常见问题

### Q: Purge 是独立的服务吗？

**A:** 不是。Purge 是 Pingora Slice 的一个集成功能，通过配置文件启用即可使用。

### Q: 我需要单独部署 Purge 服务吗？

**A:** 不需要。只需在 pingora-slice 的配置文件中启用 purge 功能即可。

### Q: Purge 和 Slice 使用同一个端口吗？

**A:** 是的，默认情况下它们使用同一个端口。Pingora Slice 会根据 HTTP 方法（GET vs PURGE）来决定如何处理请求。

### Q: 如何知道 Purge 功能是否启用？

**A:** 检查配置文件中的 `purge.enabled` 字段，或者尝试发送一个 PURGE 请求：

```bash
# 如果返回 200 或 401，说明 Purge 已启用
# 如果返回 404 或 405，说明 Purge 未启用
curl -X PURGE http://your-server.com/test
```

### Q: 可以只启用 Purge 而不启用 Slice 吗？

**A:** 不建议。Purge 是用来清除 Slice 缓存的，如果没有 Slice 功能，Purge 就没有意义。但技术上可以通过配置实现。

### Q: Purge 会影响 Slice 的性能吗？

**A:** 不会。Purge 操作是异步的（特别是 L2 磁盘删除），不会阻塞正常的请求处理。

## 部署建议

### 生产环境

```yaml
# 生产环境配置
purge:
  enabled: true
  auth_token: "${PURGE_TOKEN}"  # 从环境变量读取
  enable_metrics: true

# 使用环境变量
export PURGE_TOKEN=$(openssl rand -hex 32)
./pingora-slice -c pingora_slice.yaml
```

### 安全建议

1. **必须启用认证**：生产环境必须设置 `auth_token`
2. **限制访问**：使用防火墙限制 PURGE 请求来源
3. **监控告警**：设置 Prometheus 告警监控异常 PURGE 活动
4. **日志审计**：记录所有 PURGE 操作

## 总结

- ✅ Purge 是 Pingora Slice 的**集成功能**，不是独立服务
- ✅ 只需在配置文件中**启用 purge**，无需额外部署
- ✅ 使用**同一个服务**处理 GET 和 PURGE 请求
- ✅ 通过**配置文件**控制所有功能
- ✅ **开箱即用**，配置简单

## 相关文档

- [HTTP PURGE 参考](HTTP_PURGE_REFERENCE.md)
- [配置和指标](PURGE_CONFIG_AND_METRICS.md)
- [缓存清除指南](CACHE_PURGE_zh.md)

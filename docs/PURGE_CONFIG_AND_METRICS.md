# Purge 配置和 Prometheus 指标

## 概述

本文档说明如何配置 HTTP PURGE 功能以及如何使用 Prometheus 监控 purge 操作。

## 配置文件

### 基本配置

在 `pingora_slice.yaml` 中添加 purge 配置：

```yaml
# Purge configuration
purge:
  enabled: true                          # 启用 PURGE 功能
  auth_token: "your-secret-token-here"   # 认证令牌（可选）
  enable_metrics: true                   # 启用 Prometheus 指标
```

### 完整配置示例

```yaml
# 完整的 Pingora Slice 配置
slice_size: 1048576
max_concurrent_subrequests: 4
max_retries: 3
enable_cache: true
cache_ttl: 3600
upstream_address: "origin.example.com:80"

# Metrics 端点
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"

# Purge 配置
purge:
  enabled: true
  auth_token: "my-secret-purge-token-2024"
  enable_metrics: true
```

### 配置选项说明

| 选项 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `enabled` | boolean | `false` | 是否启用 PURGE 功能 |
| `auth_token` | string | `null` | 认证令牌，如果不设置则不需要认证 |
| `enable_metrics` | boolean | `true` | 是否启用 Prometheus 指标 |

## Prometheus 指标

### 可用指标

#### 1. `pingora_slice_purge_requests_total`

总 PURGE 请求数，按方法分类。

**标签：**
- `method`: 清除方法 (`url`, `all`, `pattern`)

**示例：**
```
pingora_slice_purge_requests_total{method="url"} 150
pingora_slice_purge_requests_total{method="all"} 5
pingora_slice_purge_requests_total{method="pattern"} 20
```

#### 2. `pingora_slice_purge_requests_by_result`

按结果分类的 PURGE 请求数。

**标签：**
- `method`: 清除方法 (`url`, `all`, `pattern`)
- `result`: 结果 (`success`, `failure`)

**示例：**
```
pingora_slice_purge_requests_by_result{method="url",result="success"} 145
pingora_slice_purge_requests_by_result{method="url",result="failure"} 5
pingora_slice_purge_requests_by_result{method="all",result="success"} 5
```

#### 3. `pingora_slice_purge_items_total`

清除的缓存项总数。

**标签：**
- `method`: 清除方法

**示例：**
```
pingora_slice_purge_items_total{method="url"} 1450
pingora_slice_purge_items_total{method="all"} 5000
```

#### 4. `pingora_slice_purge_duration_seconds`

PURGE 操作持续时间（直方图）。

**标签：**
- `method`: 清除方法

**Buckets:** 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0

**示例：**
```
pingora_slice_purge_duration_seconds_bucket{method="url",le="0.001"} 50
pingora_slice_purge_duration_seconds_bucket{method="url",le="0.005"} 120
pingora_slice_purge_duration_seconds_bucket{method="url",le="0.01"} 145
pingora_slice_purge_duration_seconds_sum{method="url"} 1.234
pingora_slice_purge_duration_seconds_count{method="url"} 150
```

#### 5. `pingora_slice_purge_auth_failures_total`

认证失败总数。

**标签：**
- `reason`: 失败原因 (`missing_token`, `invalid_token`)

**示例：**
```
pingora_slice_purge_auth_failures_total{reason="invalid_token"} 10
pingora_slice_purge_auth_failures_total{reason="missing_token"} 5
```

### 查询指标

#### 通过 HTTP 端点

```bash
# 获取所有指标
curl http://localhost:9090/metrics

# 过滤 purge 相关指标
curl http://localhost:9090/metrics | grep purge
```

#### 在 Prometheus 中查询

```promql
# PURGE 请求速率（每秒）
rate(pingora_slice_purge_requests_total[5m])

# PURGE 成功率
sum(rate(pingora_slice_purge_requests_by_result{result="success"}[5m])) 
/ 
sum(rate(pingora_slice_purge_requests_total[5m]))

# 平均 PURGE 持续时间
rate(pingora_slice_purge_duration_seconds_sum[5m]) 
/ 
rate(pingora_slice_purge_duration_seconds_count[5m])

# 每次 PURGE 平均清除的项目数
rate(pingora_slice_purge_items_total[5m]) 
/ 
rate(pingora_slice_purge_requests_total[5m])

# 认证失败率
rate(pingora_slice_purge_auth_failures_total[5m])
```

## 代码集成

### 创建带指标的 PURGE 处理器

```rust
use pingora_slice::config::SliceConfig;
use pingora_slice::purge_handler::PurgeHandler;
use pingora_slice::purge_metrics::PurgeMetrics;
use pingora_slice::tiered_cache::TieredCache;
use std::sync::Arc;

// 加载配置
let config = SliceConfig::from_file("pingora_slice.yaml")?;

// 创建缓存
let cache = Arc::new(TieredCache::new(...).await?);

// 创建 PURGE 指标
let purge_metrics = Arc::new(PurgeMetrics::new()?);

// 创建 PURGE 处理器
let purge_handler = if let Some(purge_config) = &config.purge {
    if purge_config.enabled {
        let mut handler = if let Some(token) = &purge_config.auth_token {
            PurgeHandler::with_auth(cache.clone(), token.clone())
        } else {
            PurgeHandler::new(cache.clone())
        };
        
        // 如果启用了指标，添加指标
        if purge_config.enable_metrics {
            handler = handler.with_metrics(purge_metrics.clone());
        }
        
        Some(Arc::new(handler))
    } else {
        None
    }
} else {
    None
};
```

### 在请求处理中使用

```rust
async fn handle_request(req: Request) -> Response {
    // 检查是否是 PURGE 请求
    if req.method().as_str() == "PURGE" {
        if let Some(handler) = &purge_handler {
            return handler.handle_purge(req).await?;
        } else {
            return error_response(
                StatusCode::NOT_FOUND,
                "PURGE functionality is disabled"
            );
        }
    }
    
    // 处理其他请求...
}
```

## Grafana 仪表板

### 示例面板配置

#### 1. PURGE 请求速率

```json
{
  "title": "PURGE Requests Rate",
  "targets": [{
    "expr": "sum(rate(pingora_slice_purge_requests_total[5m])) by (method)"
  }],
  "type": "graph"
}
```

#### 2. PURGE 成功率

```json
{
  "title": "PURGE Success Rate",
  "targets": [{
    "expr": "sum(rate(pingora_slice_purge_requests_by_result{result=\"success\"}[5m])) / sum(rate(pingora_slice_purge_requests_total[5m])) * 100"
  }],
  "type": "gauge",
  "unit": "percent"
}
```

#### 3. PURGE 持续时间

```json
{
  "title": "PURGE Duration (p95)",
  "targets": [{
    "expr": "histogram_quantile(0.95, rate(pingora_slice_purge_duration_seconds_bucket[5m]))"
  }],
  "type": "graph",
  "unit": "s"
}
```

#### 4. 清除的项目数

```json
{
  "title": "Items Purged",
  "targets": [{
    "expr": "sum(rate(pingora_slice_purge_items_total[5m])) by (method)"
  }],
  "type": "graph"
}
```

## 告警规则

### Prometheus 告警配置

```yaml
groups:
  - name: purge_alerts
    rules:
      # PURGE 失败率过高
      - alert: HighPurgeFailureRate
        expr: |
          sum(rate(pingora_slice_purge_requests_by_result{result="failure"}[5m])) 
          / 
          sum(rate(pingora_slice_purge_requests_total[5m])) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High PURGE failure rate"
          description: "PURGE failure rate is {{ $value | humanizePercentage }} (threshold: 10%)"
      
      # 认证失败过多
      - alert: HighPurgeAuthFailures
        expr: rate(pingora_slice_purge_auth_failures_total[5m]) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High PURGE authentication failures"
          description: "PURGE auth failures: {{ $value }} per second"
      
      # PURGE 操作过慢
      - alert: SlowPurgeOperations
        expr: |
          histogram_quantile(0.95, 
            rate(pingora_slice_purge_duration_seconds_bucket[5m])
          ) > 1.0
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Slow PURGE operations"
          description: "P95 PURGE duration is {{ $value }}s (threshold: 1s)"
```

## 测试

### 运行示例服务器

```bash
# 启动服务器（带指标）
cargo run --example http_purge_server

# 在另一个终端查看指标
curl http://localhost:8080/metrics | grep purge
```

### 测试 PURGE 操作

```bash
# 执行 PURGE
curl -X PURGE http://localhost:8080/test.dat

# 查看指标变化
curl http://localhost:8080/metrics | grep purge_requests_total
```

### 预期输出

```
# HELP pingora_slice_purge_requests_total Total number of cache purge requests
# TYPE pingora_slice_purge_requests_total counter
pingora_slice_purge_requests_total{method="url"} 1

# HELP pingora_slice_purge_requests_by_result Total number of purge requests by result
# TYPE pingora_slice_purge_requests_by_result counter
pingora_slice_purge_requests_by_result{method="url",result="success"} 1

# HELP pingora_slice_purge_items_total Total number of cache items purged
# TYPE pingora_slice_purge_items_total counter
pingora_slice_purge_items_total{method="url"} 5

# HELP pingora_slice_purge_duration_seconds Duration of purge operations in seconds
# TYPE pingora_slice_purge_duration_seconds histogram
pingora_slice_purge_duration_seconds_bucket{method="url",le="0.001"} 0
pingora_slice_purge_duration_seconds_bucket{method="url",le="0.005"} 1
pingora_slice_purge_duration_seconds_bucket{method="url",le="+Inf"} 1
pingora_slice_purge_duration_seconds_sum{method="url"} 0.002
pingora_slice_purge_duration_seconds_count{method="url"} 1
```

## 最佳实践

### 配置

1. **生产环境必须启用认证**
   ```yaml
   purge:
     enabled: true
     auth_token: "use-a-strong-random-token"
     enable_metrics: true
   ```

2. **使用环境变量存储敏感信息**
   ```bash
   export PURGE_TOKEN=$(openssl rand -hex 32)
   ```

3. **定期轮换认证令牌**

### 监控

1. **设置告警**：监控失败率和认证失败
2. **跟踪趋势**：观察 PURGE 频率和模式
3. **性能监控**：关注 PURGE 持续时间
4. **容量规划**：根据清除的项目数调整缓存大小

### 安全

1. **限制访问**：只允许内部网络访问 PURGE 端点
2. **日志审计**：记录所有 PURGE 操作
3. **速率限制**：防止 PURGE 滥用
4. **监控异常**：告警异常的 PURGE 模式

## 相关文档

- [HTTP PURGE 参考](HTTP_PURGE_REFERENCE.md)
- [缓存清除指南](CACHE_PURGE_zh.md)
- [配置文档](CONFIGURATION.md)
- [Prometheus 集成](../monitoring/prometheus.yml)

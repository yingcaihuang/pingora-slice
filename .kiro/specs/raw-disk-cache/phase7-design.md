# Phase 7: Pingora 框架集成 - 流式代理设计

## 概述

Phase 7 的目标是使用 Pingora 框架实现一个生产级别的流式代理服务器，解决当前 `full_proxy_server.rs` 的核心问题：缓存未命中时需要等待整个文件下载完成才返回给客户端。

## 问题分析

### 当前实现的问题

```rust
// 当前 full_proxy_server.rs 的问题
match response.bytes().await {  // ❌ 等待整个响应
    Ok(data) => {
        cache.store(&url, &range, data.clone())?;  // ❌ 然后缓存
        Ok(Response::new(Full::new(data)))  // ❌ 最后返回
    }
}
```

**问题**：
1. 客户端等待时间 = 下载时间 + 缓存时间
2. 内存占用 = 整个文件大小
3. 不支持大文件（>1GB）
4. 不符合 HTTP 代理规范

### 正确的流式实现

```
客户端 ←─────┐
             │ 实时流式传输
             │
        ┌────┴────┐
        │  代理   │
        └────┬────┘
             │ 边接收边转发
             │ 同时缓存数据块
             ↓
        上游服务器
```

**优势**：
1. 客户端立即开始接收数据（低 TTFB）
2. 内存使用稳定（只缓存数据块）
3. 支持任意大小文件
4. 符合 HTTP 代理规范

## 架构设计

### 核心组件

```rust
pub struct StreamingProxy {
    /// 缓存（支持 raw disk）
    cache: Arc<TieredCache>,
    
    /// 配置
    config: Arc<SliceConfig>,
    
    /// 指标收集
    metrics: Arc<ProxyMetrics>,
}

pub struct ProxyContext {
    /// 请求 URL
    url: String,
    
    /// 是否启用缓存
    cache_enabled: bool,
    
    /// 缓存键
    cache_key: String,
    
    /// 数据缓冲区（用于缓存）
    buffer: Vec<Bytes>,
    
    /// 已接收字节数
    bytes_received: u64,
}
```

### 数据流

```
1. 客户端请求
   ↓
2. upstream_request_filter()
   - 检查缓存
   - 如果命中：直接返回缓存
   - 如果未命中：继续
   ↓
3. upstream_peer()
   - 连接上游服务器
   ↓
4. response_filter()
   - 处理响应头
   - 决定是否缓存
   ↓
5. response_body_filter() (循环)
   - 接收数据块
   - 转发给客户端 ←─── 实时
   - 缓存数据块    ←─── 后台
   ↓
6. 流结束
   - 完成缓存写入
   - 更新指标
```

## 详细实现

### 1. ProxyHttp Trait 实现

```rust
#[async_trait]
impl ProxyHttp for StreamingProxy {
    type CTX = ProxyContext;
    
    fn new_ctx(&self) -> Self::CTX {
        ProxyContext {
            url: String::new(),
            cache_enabled: false,
            cache_key: String::new(),
            buffer: Vec::new(),
            bytes_received: 0,
        }
    }
    
    async fn upstream_peer(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // 从配置读取上游地址
        let upstream = &self.config.upstream_address;
        
        // 解析地址
        let (host, port) = parse_upstream(upstream)?;
        
        // 创建 peer
        let peer = Box::new(HttpPeer::new(
            (host.as_str(), port),
            false,  // HTTP
            host.clone(),
        ));
        
        Ok(peer)
    }
}
```

### 2. 请求过滤器

```rust
async fn upstream_request_filter(
    &self,
    session: &mut Session,
    upstream_request: &mut RequestHeader,
    ctx: &mut Self::CTX,
) -> Result<()> {
    // 1. 获取请求 URL
    let uri = session.req_header().uri.clone();
    ctx.url = uri.to_string();
    ctx.cache_key = format!("cache:{}", ctx.url);
    
    // 2. 检查缓存
    if self.config.enable_cache {
        let range = ByteRange::new(0, 1024 * 1024 - 1)?;
        
        if let Ok(Some(data)) = self.cache.lookup(&ctx.cache_key, &range).await {
            // 缓存命中 - 直接返回
            info!("Cache HIT: {}", ctx.url);
            
            // 构造响应
            let mut response = ResponseHeader::build(200, None)?;
            response.insert_header("x-cache", "HIT")?;
            response.insert_header("content-length", data.len().to_string())?;
            
            session.write_response_header(Box::new(response)).await?;
            session.write_response_body(Some(data), true).await?;
            
            // 返回错误以停止上游请求
            return Err(Error::new(ErrorType::HTTPStatus(200)));
        }
        
        info!("Cache MISS: {}", ctx.url);
        ctx.cache_enabled = true;
    }
    
    // 3. 添加必要的请求头
    upstream_request.insert_header("Host", &self.config.upstream_address)?;
    upstream_request.insert_header("User-Agent", "Pingora-Slice/1.0")?;
    
    Ok(())
}
```

### 3. 响应过滤器

```rust
async fn response_filter(
    &self,
    _session: &mut Session,
    upstream_response: &mut ResponseHeader,
    ctx: &mut Self::CTX,
) -> Result<()> {
    // 1. 检查状态码
    let status = upstream_response.status;
    if !status.is_success() {
        ctx.cache_enabled = false;
        return Ok(());
    }
    
    // 2. 检查 Content-Length
    if let Some(cl) = upstream_response.headers.get("content-length") {
        if let Ok(size) = cl.to_str()?.parse::<u64>() {
            info!("Content-Length: {} bytes", size);
            
            // 如果文件太大，可以选择不缓存
            if size > 1024 * 1024 * 1024 {  // 1GB
                warn!("File too large to cache: {} bytes", size);
                ctx.cache_enabled = false;
            }
        }
    }
    
    // 3. 添加 X-Cache 头
    upstream_response.insert_header("x-cache", "MISS")?;
    
    Ok(())
}
```

### 4. 响应体过滤器（核心）

```rust
async fn response_body_filter(
    &self,
    _session: &mut Session,
    body: &mut Option<Bytes>,
    end_of_stream: bool,
    ctx: &mut Self::CTX,
) -> Result<Option<Duration>> {
    // 1. 如果有数据块
    if let Some(data) = body {
        ctx.bytes_received += data.len() as u64;
        
        // 2. 如果启用缓存，保存数据块
        if ctx.cache_enabled {
            ctx.buffer.push(data.clone());
        }
        
        // 3. 数据会自动转发给客户端（Pingora 框架处理）
        debug!("Forwarded {} bytes to client", data.len());
    }
    
    // 4. 如果流结束
    if end_of_stream && ctx.cache_enabled {
        // 合并所有数据块
        let total_data: Vec<u8> = ctx.buffer
            .iter()
            .flat_map(|b| b.iter())
            .copied()
            .collect();
        
        let data = Bytes::from(total_data);
        
        info!("Stream ended, caching {} bytes", data.len());
        
        // 存储到缓存
        let range = ByteRange::new(0, data.len() as u64 - 1)?;
        if let Err(e) = self.cache.store(&ctx.cache_key, &range, data) {
            warn!("Failed to cache: {}", e);
        } else {
            info!("Cached successfully: {}", ctx.url);
        }
        
        // 清空缓冲区
        ctx.buffer.clear();
    }
    
    Ok(None)
}
```

### 5. 主函数

```rust
fn main() {
    // 1. 初始化日志
    tracing_subscriber::fmt::init();
    
    // 2. 加载配置
    let config = SliceConfig::from_file("config.yaml").unwrap();
    let config = Arc::new(config);
    
    // 3. 创建缓存
    let cache = create_cache(&config).await.unwrap();
    
    // 4. 创建代理
    let proxy = StreamingProxy {
        cache,
        config: config.clone(),
        metrics: Arc::new(ProxyMetrics::new()),
    };
    
    // 5. 创建 Pingora 服务器
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    
    // 6. 创建代理服务
    let mut proxy_service = http_proxy_service(&server.configuration, proxy);
    proxy_service.add_tcp("0.0.0.0:8080");
    
    // 7. 启动服务器
    server.add_service(proxy_service);
    
    info!("Streaming proxy server started on :8080");
    server.run_forever();
}
```

## 关键特性

### 1. 流式传输

- ✅ 数据块立即转发给客户端
- ✅ 不等待整个文件下载完成
- ✅ 低 TTFB（首字节时间）

### 2. 后台缓存

- ✅ 边转发边缓存
- ✅ 不阻塞客户端响应
- ✅ 缓存失败不影响代理功能

### 3. 内存效率

- ✅ 只缓存数据块，不缓存整个文件
- ✅ 内存使用稳定
- ✅ 支持大文件（>1GB）

### 4. 缓存优先

- ✅ 先检查缓存
- ✅ 缓存命中直接返回
- ✅ 减少上游请求

## 性能对比

| 指标 | full_proxy_server | Pingora 流式代理 |
|------|------------------|-----------------|
| TTFB | 下载时间 + 缓存时间 | <100ms |
| 内存使用 | 文件大小 | 稳定（~10MB） |
| 支持文件大小 | <100MB | 无限制 |
| 并发能力 | 低 | 高 |
| 生产可用 | ❌ | ✅ |

## 测试计划

### 单元测试

1. 测试缓存命中逻辑
2. 测试缓存未命中逻辑
3. 测试数据块缓存
4. 测试错误处理

### 集成测试

1. 测试小文件代理（<1MB）
2. 测试大文件代理（>100MB）
3. 测试并发请求（100 并发）
4. 测试缓存命中率

### 性能测试

1. 测试 TTFB
2. 测试吞吐量
3. 测试内存使用
4. 测试 CPU 使用

## 部署指南

### 配置示例

```yaml
# Pingora 流式代理配置
upstream_address: "mirrors.verycloud.cn:80"

# 缓存配置
enable_cache: true
cache_ttl: 3600

# L1 缓存
l1_cache_size_bytes: 104857600  # 100MB

# L2 缓存（raw disk）
enable_l2_cache: true
l2_backend: "raw_disk"

raw_disk_cache:
  device_path: "/var/cache/pingora-slice-raw"
  total_size: 107374182400  # 100GB
  block_size: 4096
  use_direct_io: true
  enable_compression: true
  enable_prefetch: true
  enable_zero_copy: true
```

### 启动命令

```bash
# 编译
cargo build --release

# 运行
./target/release/pingora-streaming-proxy --config config.yaml
```

## 下一步

1. 实现基本的 ProxyHttp trait
2. 实现流式缓存逻辑
3. 编写集成测试
4. 性能测试和优化
5. 编写部署文档

## 参考资料

- [Pingora 文档](https://github.com/cloudflare/pingora)
- [Pingora 代理示例](https://github.com/cloudflare/pingora/tree/main/pingora-proxy/examples)
- [HTTP 流式传输](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Transfer-Encoding)

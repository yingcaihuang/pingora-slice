# 流式代理实现说明

## 问题

当前的 `full_proxy_server.rs` 实现有一个重要缺陷：

**问题**：第一次缓存未命中时，会等待整个文件下载完成后才返回给客户端。

**正确行为**：应该边下载边返回（streaming），同时在后台缓存。

## 为什么会这样

当前代码：
```rust
// 等待整个响应下载完成
match response.bytes().await {
    Ok(data) => {
        // 存储到缓存
        state.cache.store(&url, &range, data.clone())?;
        
        // 然后才返回给客户端
        Ok(Response::builder()
            .body(Full::new(data))
            .unwrap())
    }
}
```

这种方式：
1. ❌ 等待整个文件下载完成
2. ❌ 占用大量内存（整个文件在内存中）
3. ❌ 客户端等待时间长
4. ❌ 不符合 HTTP 代理规范

## 正确的实现方式

### 方案 1：使用 Hyper 的 Stream（复杂）

需要实现：
1. 从上游获取 `Stream<Item = Result<Bytes>>`
2. 创建一个 `tee` stream，同时：
   - 发送数据给客户端
   - 缓存数据到磁盘
3. 返回 streaming response

```rust
// 伪代码示例
let stream = response.bytes_stream();

// 创建一个 tee，同时发送给客户端和缓存
let (client_stream, cache_stream) = stream.tee();

// 后台任务：缓存数据
tokio::spawn(async move {
    let mut buffer = Vec::new();
    while let Some(chunk) = cache_stream.next().await {
        buffer.extend_from_slice(&chunk?);
    }
    cache.store(&url, &range, Bytes::from(buffer))?;
});

// 返回 streaming response
Ok(Response::builder()
    .body(Body::wrap_stream(client_stream))
    .unwrap())
```

### 方案 2：使用 Pingora 框架（推荐）

Pingora 框架已经内置了完整的流式代理功能，包括：
- ✅ 边下载边返回
- ✅ 自动缓存
- ✅ Range 请求支持
- ✅ 连接池管理
- ✅ 错误处理和重试

## 为什么 `full_proxy_server.rs` 不实现流式代理

1. **复杂性**：流式代理需要处理很多边界情况
2. **示例目的**：这只是一个演示 raw disk cache 集成的简单示例
3. **框架依赖**：完整实现需要 Pingora 框架的支持

## 使用 Pingora 框架的完整代理

Pingora 框架提供了完整的代理服务器实现。要使用它：

### 1. 使用 Pingora 的 Proxy Trait

```rust
use pingora::prelude::*;
use pingora::proxy::http_proxy_service;

pub struct MyProxy {
    cache: Arc<TieredCache>,
}

#[async_trait]
impl ProxyHttp for MyProxy {
    type CTX = ();
    
    fn new_ctx(&self) -> Self::CTX {
        ()
    }
    
    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        // 返回上游服务器
        let peer = Box::new(HttpPeer::new(
            ("mirrors.verycloud.cn", 80),
            false,
            "mirrors.verycloud.cn".to_string(),
        ));
        Ok(peer)
    }
    
    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        // 修改上游请求
        Ok(())
    }
    
    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        // 处理响应头
        Ok(())
    }
    
    async fn response_body_filter(
        &self,
        _session: &mut Session,
        body: &mut Option<Bytes>,
        end_of_stream: bool,
        _ctx: &mut Self::CTX,
    ) -> Result<Option<Duration>> {
        // 这里可以缓存数据
        if let Some(data) = body {
            // 缓存数据块
        }
        Ok(None)
    }
}
```

### 2. 启动 Pingora 服务器

```rust
fn main() {
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    
    let mut proxy_service = http_proxy_service(
        &server.configuration,
        MyProxy { cache: /* ... */ },
    );
    
    proxy_service.add_tcp("0.0.0.0:8080");
    server.add_service(proxy_service);
    server.run_forever();
}
```

## 当前示例的适用场景

`full_proxy_server.rs` 适合：
- ✅ 演示 raw disk cache 的基本功能
- ✅ 测试缓存的读写
- ✅ 小文件代理（< 10MB）
- ✅ 开发和调试

**不适合**：
- ❌ 生产环境
- ❌ 大文件代理（> 10MB）
- ❌ 高并发场景
- ❌ 需要流式传输的场景

## 解决方案

### 短期方案：限制文件大小

在 `full_proxy_server.rs` 中添加文件大小检查：

```rust
// 检查 Content-Length
if let Some(content_length) = response.headers().get("content-length") {
    let size: u64 = content_length.to_str()?.parse()?;
    if size > 10 * 1024 * 1024 {  // 10MB
        return Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Full::new(Bytes::from("File too large for this proxy")))
            .unwrap());
    }
}
```

### 长期方案：使用 Pingora 框架

创建一个基于 Pingora 框架的完整代理服务器，它会自动处理：
- 流式传输
- 缓存
- Range 请求
- 错误处理
- 连接管理

## 参考资料

1. **Pingora 文档**：https://github.com/cloudflare/pingora
2. **Pingora 示例**：https://github.com/cloudflare/pingora/tree/main/pingora-proxy/examples
3. **HTTP Streaming**：https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Transfer-Encoding

## 总结

- ✅ `full_proxy_server.rs` 是一个**简化的示例**，用于演示 raw disk cache
- ❌ 它**不适合生产环境**，因为不支持流式传输
- ✅ 对于生产环境，应该使用 **Pingora 框架**的完整代理实现
- ✅ Pingora 框架已经内置了所有必要的功能，包括流式传输和缓存

## 下一步

如果你需要生产级别的代理服务器，我建议：

1. 学习 Pingora 框架的使用
2. 参考 Pingora 的代理示例
3. 将 raw disk cache 集成到 Pingora 代理中
4. 使用 Pingora 的 `response_body_filter` 来缓存数据

这样你就能获得：
- ✅ 流式传输（边下载边返回）
- ✅ 自动缓存
- ✅ 高性能
- ✅ 生产级别的稳定性

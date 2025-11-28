# Design Document - Pingora Slice Module

## Overview

Pingora Slice Module 是一个为 Pingora 代理服务器设计的分片回源模块，实现类似 Nginx Slice 模块的功能。该模块通过将大文件请求自动拆分为多个小的 HTTP Range 请求，实现高效的分片缓存和并发回源，从而提高大文件传输的性能和可靠性。

核心设计理念：
- **透明性**：对客户端完全透明，客户端无需知道代理使用了分片技术
- **高效性**：通过并发回源和分片缓存提高性能
- **可靠性**：支持重试机制和错误处理
- **灵活性**：可配置的分片大小和并发控制

## Architecture

### 系统架构

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │ HTTP Request (GET /large-file.bin)
       ▼
┌─────────────────────────────────────────────────┐
│           Pingora Proxy Server                  │
│  ┌───────────────────────────────────────────┐  │
│  │        Slice Module                       │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  1. Request Analyzer                │  │  │
│  │  │     - Check if slicing applicable   │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  2. Metadata Fetcher                │  │  │
│  │  │     - HEAD request to origin        │  │  │
│  │  │     - Get Content-Length            │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  3. Slice Calculator                │  │  │
│  │  │     - Calculate slice ranges        │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  4. Subrequest Manager              │  │  │
│  │  │     - Concurrent subrequests        │  │  │
│  │  │     - Retry logic                   │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  5. Response Assembler              │  │  │
│  │  │     - Buffer management             │  │  │
│  │  │     - Ordered streaming             │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  6. Cache Manager                   │  │  │
│  │  │     - Slice storage                 │  │  │
│  │  │     - Cache lookup                  │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────┘  │
└──────────┬──────────────────────────────────────┘
           │ Multiple Range Requests
           ▼
    ┌──────────────┐
    │Origin Server │
    └──────────────┘
```

### 请求处理流程

```
Client Request
      │
      ▼
[Request Analyzer] ──No──> [Normal Proxy Mode]
      │ Yes
      ▼
[Metadata Fetcher] ──HEAD──> Origin
      │ (Content-Length, Accept-Ranges)
      ▼
[Check Cache] ──All Cached──> [Response Assembler]
      │ Partial/None
      ▼
[Slice Calculator]
      │ (Calculate missing slices)
      ▼
[Subrequest Manager] ──Range Requests──> Origin
      │ (Concurrent fetching)
      ▼
[Cache Manager] (Store slices)
      │
      ▼
[Response Assembler] (Stream to client)
      │
      ▼
Client Response
```

## Components and Interfaces

### 1. SliceProxy (主代理结构)

实现 Pingora 的 `ProxyHttp` trait：

```rust
pub struct SliceProxy {
    config: Arc<SliceConfig>,
    http_client: HttpConnector,
    cache_storage: Option<Arc<dyn CacheStorage>>,
    metrics: Arc<SliceMetrics>,
}

pub struct SliceContext {
    // 是否启用分片
    slice_enabled: bool,
    // 文件元数据
    metadata: Option<FileMetadata>,
    // 客户端请求的范围
    client_range: Option<ByteRange>,
    // 计算的分片
    slices: Vec<SliceSpec>,
}

#[async_trait]
impl ProxyHttp for SliceProxy {
    type CTX = SliceContext;
    
    fn new_ctx(&self) -> Self::CTX {
        SliceContext {
            slice_enabled: false,
            metadata: None,
            client_range: None,
            slices: Vec::new(),
        }
    }
    
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        // 决定是否使用分片
        // 返回 false 表示我们将自己处理响应
    }
    
    async fn upstream_peer(&self, _session: &mut Session, _ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        // 如果不使用分片，返回正常的上游服务器
    }
}

impl SliceProxy {
    pub fn new(config: SliceConfig, cache_storage: Option<Arc<dyn CacheStorage>>) -> Self;
    
    async fn handle_slice_request(&self, session: &mut Session, ctx: &mut SliceContext) -> Result<()>;
}
```

### 2. SliceConfig (配置)

```rust
pub struct SliceConfig {
    /// 分片大小 (字节)
    pub slice_size: usize,
    /// 最大并发子请求数
    pub max_concurrent_subrequests: usize,
    /// 子请求重试次数
    pub max_retries: usize,
    /// 启用分片的 URL 模式
    pub slice_patterns: Vec<String>,
    /// 是否启用缓存
    pub enable_cache: bool,
    /// 缓存过期时间 (秒)
    pub cache_ttl: u64,
}

impl SliceConfig {
    pub fn from_file(path: &str) -> Result<Self>;
    pub fn validate(&self) -> Result<()>;
}
```

### 3. RequestAnalyzer (请求分析器)

```rust
pub struct RequestAnalyzer {
    config: Arc<SliceConfig>,
}

impl RequestAnalyzer {
    pub fn should_slice(&self, req: &RequestHeader) -> bool;
    pub fn extract_client_range(&self, req: &RequestHeader) -> Option<ByteRange>;
}
```

### 4. MetadataFetcher (元数据获取器)

```rust
pub struct MetadataFetcher;

pub struct FileMetadata {
    pub content_length: u64,
    pub supports_range: bool,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl MetadataFetcher {
    pub async fn fetch_metadata(
        &self,
        session: &mut Session,
        url: &str
    ) -> Result<FileMetadata>;
}
```

### 5. SliceCalculator (分片计算器)

```rust
pub struct SliceCalculator {
    slice_size: usize,
}

pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

pub struct SliceSpec {
    pub index: usize,
    pub range: ByteRange,
}

impl SliceCalculator {
    pub fn calculate_slices(
        &self,
        file_size: u64,
        client_range: Option<ByteRange>
    ) -> Vec<SliceSpec>;
    
    pub fn calculate_total_slices(&self, file_size: u64) -> usize;
}
```

### 6. SubrequestManager (子请求管理器)

使用 Pingora 的 `HttpConnector` 或独立的 HTTP 客户端发起子请求：

```rust
pub struct SubrequestManager {
    http_connector: HttpConnector,
    max_concurrent: usize,
    max_retries: usize,
    retry_policy: RetryPolicy,
}

pub struct SubrequestResult {
    pub slice_index: usize,
    pub data: Bytes,
    pub status: u16,
    pub headers: ResponseHeader,
}

impl SubrequestManager {
    pub fn new(max_concurrent: usize, max_retries: usize) -> Self;
    
    /// 并发获取多个分片
    pub async fn fetch_slices(
        &self,
        slices: Vec<SliceSpec>,
        url: &str,
        original_headers: &RequestHeader,
    ) -> Result<Vec<SubrequestResult>>;
    
    /// 获取单个分片（带重试）
    async fn fetch_single_slice(
        &self,
        slice: &SliceSpec,
        url: &str,
        original_headers: &RequestHeader,
    ) -> Result<SubrequestResult>;
    
    /// 创建 Range 请求
    fn build_range_request(
        &self,
        url: &str,
        range: &ByteRange,
        original_headers: &RequestHeader,
    ) -> RequestHeader;
}
```

### 7. ResponseAssembler (响应组装器)

```rust
pub struct ResponseAssembler {
    total_size: u64,
}

pub struct SliceBuffer {
    slices: HashMap<usize, Bytes>,
    next_index: usize,
}

impl ResponseAssembler {
    pub fn new(total_size: u64) -> Self;
    
    pub async fn stream_response(
        &self,
        session: &mut Session,
        slice_results: Vec<SubrequestResult>,
        metadata: &FileMetadata
    ) -> Result<()>;
    
    fn build_response_header(
        &self,
        metadata: &FileMetadata,
        range: Option<ByteRange>
    ) -> ResponseHeader;
}
```

### 8. SliceCache (缓存管理器)

集成 Pingora 的缓存系统：

```rust
use pingora::cache::{CacheKey, CacheMeta, CacheStorage};

pub struct SliceCache {
    storage: Arc<dyn CacheStorage>,
    ttl: Duration,
}

impl SliceCache {
    pub fn new(storage: Arc<dyn CacheStorage>, ttl: Duration) -> Self;
    
    /// 生成分片的缓存键
    pub fn generate_cache_key(&self, url: &str, range: &ByteRange) -> CacheKey {
        let key_str = format!("{}:slice:{}:{}", url, range.start, range.end);
        CacheKey::new("", &key_str, "")
    }
    
    /// 查找缓存的分片
    pub async fn lookup_slice(&self, url: &str, range: &ByteRange) -> Result<Option<Bytes>> {
        let key = self.generate_cache_key(url, range);
        match self.storage.lookup(&key).await? {
            Some(_meta) => {
                self.storage.get_body(&key, None).await
            }
            None => Ok(None),
        }
    }
    
    /// 存储分片到缓存
    pub async fn store_slice(
        &self,
        url: &str,
        range: &ByteRange,
        data: Bytes,
        headers: &ResponseHeader,
    ) -> Result<()> {
        let key = self.generate_cache_key(url, range);
        let meta = CacheMeta::new(
            headers.status.as_u16(),
            self.ttl,
            headers.clone(),
        );
        self.storage.put(&key, meta, data).await
    }
    
    /// 批量查找多个分片
    pub async fn lookup_multiple(
        &self,
        url: &str,
        ranges: &[ByteRange],
    ) -> HashMap<usize, Bytes> {
        let mut cached = HashMap::new();
        for (idx, range) in ranges.iter().enumerate() {
            if let Ok(Some(data)) = self.lookup_slice(url, range).await {
                cached.insert(idx, data);
            }
        }
        cached
    }
}
```

### 9. SliceMetrics (指标收集器)

```rust
pub struct SliceMetrics {
    total_requests: AtomicU64,
    sliced_requests: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    total_subrequests: AtomicU64,
    failed_subrequests: AtomicU64,
}

impl SliceMetrics {
    pub fn record_request(&self, sliced: bool);
    pub fn record_cache_hit(&self);
    pub fn record_cache_miss(&self);
    pub fn record_subrequest(&self, success: bool);
    pub fn get_stats(&self) -> MetricsSnapshot;
}
```

## Data Models

### ByteRange

表示字节范围：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    /// 起始字节位置 (inclusive)
    pub start: u64,
    /// 结束字节位置 (inclusive)
    pub end: u64,
}

impl ByteRange {
    pub fn new(start: u64, end: u64) -> Result<Self>;
    pub fn size(&self) -> u64;
    pub fn is_valid(&self) -> bool;
    pub fn from_header(header: &str) -> Result<Self>;
    pub fn to_header(&self) -> String;
}
```

### SliceSpec

分片规格：

```rust
#[derive(Debug, Clone)]
pub struct SliceSpec {
    /// 分片索引
    pub index: usize,
    /// 字节范围
    pub range: ByteRange,
}
```

### FileMetadata

文件元数据：

```rust
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// 文件总大小
    pub content_length: u64,
    /// 是否支持 Range 请求
    pub supports_range: bool,
    /// 内容类型
    pub content_type: Option<String>,
    /// ETag
    pub etag: Option<String>,
    /// 最后修改时间
    pub last_modified: Option<String>,
}
```

### SliceConfig

配置数据模型：

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct SliceConfig {
    #[serde(default = "default_slice_size")]
    pub slice_size: usize,
    
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_subrequests: usize,
    
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    
    #[serde(default)]
    pub slice_patterns: Vec<String>,
    
    #[serde(default = "default_true")]
    pub enable_cache: bool,
    
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
}

fn default_slice_size() -> usize { 1024 * 1024 } // 1MB
fn default_max_concurrent() -> usize { 4 }
fn default_max_retries() -> usize { 3 }
fn default_true() -> bool { true }
fn default_cache_ttl() -> u64 { 3600 } // 1 hour
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*


### Property Reflection

在分析了所有验收标准后，我识别出以下可以合并或简化的属性：

**合并机会：**
1. 配置验证相关的属性（1.2, 1.4）可以合并为一个综合的配置验证属性
2. 错误处理相关的属性（8.1, 8.2, 8.5）可以合并为错误响应传播属性
3. 指标记录相关的属性（9.1, 9.2, 9.3, 9.4）可以合并为综合的指标记录属性

**保留的独立属性：**
- 分片计算的正确性（覆盖范围、无重叠、无遗漏）
- 字节顺序保持（关键的正确性保证）
- 缓存一致性（缓存键唯一性、缓存命中正确性）
- Range 请求处理（解析、计算、响应格式）

### Configuration Properties

Property 1: 配置值范围验证
*For any* slice size configuration value, if the value is outside the range [64KB, 10MB], then validation should fail and the module should refuse to start
**Validates: Requirements 1.2, 1.4**

### Request Analysis Properties

Property 2: Range 请求透传
*For any* client request containing a Range header, the request should bypass the slicing logic and be passed through to the origin server directly
**Validates: Requirements 2.3**

Property 3: URL 模式匹配一致性
*For any* request URL and configured pattern list, the pattern matching result should be deterministic and consistent across multiple evaluations
**Validates: Requirements 2.4**

### Slice Calculation Properties

Property 4: 分片覆盖完整性
*For any* file size and slice size, the calculated slices should cover all bytes from 0 to file_size-1 without gaps
**Validates: Requirements 4.1, 4.2**

Property 5: 分片无重叠性
*For any* set of calculated slices, no two slices should have overlapping byte ranges
**Validates: Requirements 4.2**

Property 6: Range 头格式正确性
*For any* generated slice specification, the corresponding Range header should be in the format "bytes=start-end" where start <= end
**Validates: Requirements 4.2**

### Concurrent Fetching Properties

Property 7: 并发限制遵守
*For any* configured concurrency limit N and set of subrequests, at no point should there be more than N concurrent active subrequests
**Validates: Requirements 5.2**

Property 8: 重试次数限制
*For any* failed subrequest and configured max retry count M, the subrequest should be retried at most M times before giving up
**Validates: Requirements 5.4**

Property 9: 失败传播
*For any* subrequest that exhausts all retries, the entire request should be aborted and an error should be returned to the client
**Validates: Requirements 5.5**

### Response Assembly Properties

Property 10: 字节顺序保持（关键属性）
*For any* set of slices assembled into a response, the bytes in the final response should be in the exact same order as they appear in the original file
**Validates: Requirements 6.2**

Property 11: 响应头完整性
*For any* response sent to the client, the response headers should include Content-Length (or Transfer-Encoding: chunked) and Content-Type
**Validates: Requirements 6.5**

### Cache Properties

Property 12: 缓存键唯一性
*For any* two different slice specifications (different URL or different byte range), their cache keys should be different
**Validates: Requirements 7.2**

Property 13: 缓存命中正确性
*For any* cached slice, when retrieved from cache, the data should be identical to the data originally stored
**Validates: Requirements 7.4**

Property 14: 部分缓存命中优化
*For any* request where some slices are cached and some are not, only the non-cached slices should result in subrequests to the origin
**Validates: Requirements 7.4**

### Error Handling Properties

Property 15: 4xx 错误透传
*For any* 4xx status code returned by the origin server for a HEAD request, the same status code should be returned to the client
**Validates: Requirements 8.1**

Property 16: Content-Range 验证
*For any* 206 response received for a subrequest, if the Content-Range header does not match the requested range, the response should be treated as an error
**Validates: Requirements 8.3, 8.4**

### Range Request Properties

Property 17: Range 解析正确性
*For any* valid HTTP Range header, the parsed byte range should correctly represent the start and end positions specified in the header
**Validates: Requirements 10.1**

Property 18: 部分请求分片计算
*For any* client Range request, the calculated slices should only cover the requested byte range, not the entire file
**Validates: Requirements 10.2, 10.3**

Property 19: 206 响应格式
*For any* successful Range request, the response should have status code 206 and include a Content-Range header matching the requested range
**Validates: Requirements 10.4**

Property 20: 无效 Range 错误处理
*For any* Range request where the requested range is invalid or unsatisfiable (e.g., start > file_size), the response should be 416 Range Not Satisfiable
**Validates: Requirements 10.5**

## Error Handling

### Error Categories

1. **配置错误**
   - 无效的分片大小
   - 无效的并发限制
   - 无效的 URL 模式
   - 处理：启动时验证，记录错误，拒绝启动

2. **源站错误**
   - HEAD 请求失败（4xx/5xx）
   - 不支持 Range 请求
   - Content-Length 缺失或无效
   - 处理：回退到普通代理模式或返回错误给客户端

3. **子请求错误**
   - 单个子请求失败
   - Content-Range 不匹配
   - 超时
   - 处理：重试机制，达到最大重试次数后中止整个请求

4. **缓存错误**
   - 缓存读取失败
   - 缓存写入失败
   - 处理：记录警告，继续处理但不使用缓存

5. **组装错误**
   - 内存不足
   - 数据损坏
   - 处理：中止请求，返回 500 错误

### Error Handling Strategy

```rust
pub enum SliceError {
    ConfigError(String),
    MetadataFetchError(String),
    RangeNotSupported,
    SubrequestFailed { slice_index: usize, attempts: usize },
    CacheError(String),
    AssemblyError(String),
}

impl SliceError {
    pub fn should_retry(&self) -> bool;
    pub fn to_http_status(&self) -> u16;
    pub fn fallback_to_normal_proxy(&self) -> bool;
}
```

### Retry Logic

```rust
pub struct RetryPolicy {
    max_retries: usize,
    backoff_ms: Vec<u64>, // [100, 200, 400, ...]
}

impl RetryPolicy {
    pub fn should_retry(&self, attempt: usize, error: &SliceError) -> bool {
        attempt < self.max_retries && error.should_retry()
    }
    
    pub fn backoff_duration(&self, attempt: usize) -> Duration {
        Duration::from_millis(
            self.backoff_ms.get(attempt).copied()
                .unwrap_or(*self.backoff_ms.last().unwrap())
        )
    }
}
```

## Testing Strategy

### Unit Testing

使用 Rust 的标准测试框架进行单元测试：

1. **配置解析和验证测试**
   - 测试有效配置的解析
   - 测试无效配置的拒绝
   - 测试默认值的应用

2. **分片计算测试**
   - 测试各种文件大小和分片大小的组合
   - 测试边界情况（文件大小小于分片大小）
   - 测试最后一个分片的计算

3. **Range 头解析测试**
   - 测试标准 Range 头格式
   - 测试边界情况
   - 测试无效格式的错误处理

4. **缓存键生成测试**
   - 测试键的唯一性
   - 测试键的格式

5. **错误处理测试**
   - 测试各种错误场景的处理
   - 测试重试逻辑
   - 测试回退机制

### Property-Based Testing

使用 **proptest** 库进行基于属性的测试，验证系统在各种输入下的正确性：

**配置要求：**
- 每个属性测试至少运行 100 次迭代
- 使用 proptest 的策略生成器创建测试数据
- 每个测试必须明确标注对应的设计文档中的属性编号

**测试标注格式：**
```rust
// Feature: pingora-slice, Property 4: 分片覆盖完整性
#[test]
fn prop_slice_coverage_completeness() {
    // test implementation
}
```

**关键属性测试：**

1. **Property 4: 分片覆盖完整性**
   - 生成随机的文件大小和分片大小
   - 验证所有字节都被某个分片覆盖
   - 验证没有字节被遗漏

2. **Property 5: 分片无重叠性**
   - 生成随机的分片配置
   - 验证任意两个分片的字节范围不重叠

3. **Property 10: 字节顺序保持**
   - 生成随机的分片数据
   - 模拟乱序到达
   - 验证组装后的数据顺序正确

4. **Property 12: 缓存键唯一性**
   - 生成随机的 URL 和字节范围组合
   - 验证不同的组合产生不同的缓存键

5. **Property 17: Range 解析正确性**
   - 生成随机的有效 Range 头
   - 验证解析结果正确

6. **Property 18: 部分请求分片计算**
   - 生成随机的客户端 Range 请求
   - 验证只计算必要的分片

### Integration Testing

集成测试验证组件之间的交互：

1. **端到端流程测试**
   - 启动模拟的源站服务器
   - 发送完整文件请求
   - 验证响应正确且完整

2. **缓存集成测试**
   - 测试缓存命中和未命中场景
   - 测试部分缓存命中
   - 验证缓存一致性

3. **并发测试**
   - 测试多个并发请求
   - 验证并发限制
   - 测试资源竞争情况

4. **错误场景测试**
   - 模拟源站错误
   - 模拟网络超时
   - 验证错误处理和恢复

### Performance Testing

性能测试验证系统在负载下的表现：

1. **吞吐量测试**
   - 测量每秒处理的请求数
   - 比较启用和禁用分片的性能差异

2. **延迟测试**
   - 测量首字节时间（TTFB）
   - 测量完整响应时间

3. **资源使用测试**
   - 监控内存使用
   - 监控 CPU 使用
   - 监控网络带宽

## Implementation Notes

### Pingora Integration

本实现基于 Cloudflare Pingora (https://github.com/cloudflare/pingora) 框架。Pingora 是一个用 Rust 编写的高性能异步 HTTP 代理框架。

**Pingora 核心概念：**

1. **ProxyHttp Trait**
   
   Pingora 的核心是 `ProxyHttp` trait，它定义了代理生命周期的各个阶段：
   
   ```rust
   #[async_trait]
   pub trait ProxyHttp {
       type CTX;
       fn new_ctx(&self) -> Self::CTX;
       
       // 请求阶段
       async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool>;
       
       // 上游选择
       async fn upstream_peer(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>>;
       
       // 修改上游请求
       async fn upstream_request_filter(
           &self,
           session: &mut Session,
           upstream_request: &mut RequestHeader,
           ctx: &mut Self::CTX,
       ) -> Result<()>;
       
       // 响应过滤
       async fn response_filter(
           &self,
           session: &mut Session,
           upstream_response: &mut ResponseHeader,
           ctx: &mut Self::CTX,
       ) -> Result<()>;
       
       // 响应体过滤
       async fn response_body_filter(
           &self,
           session: &mut Session,
           body: &mut Option<Bytes>,
           end_of_stream: bool,
           ctx: &mut Self::CTX,
       ) -> Result<()>;
       
       // 日志记录
       async fn logging(&self, session: &mut Session, e: Option<&Error>, ctx: &mut Self::CTX);
   }
   ```

2. **Session**
   
   `Session` 对象代表一个客户端连接，提供访问请求/响应的方法：
   
   ```rust
   impl Session {
       // 获取客户端请求头
       pub fn req_header(&self) -> &RequestHeader;
       pub fn req_header_mut(&mut self) -> &mut RequestHeader;
       
       // 写响应
       pub async fn write_response_header(&mut self, header: Box<ResponseHeader>) -> Result<()>;
       pub async fn write_response_body(&mut self, data: Bytes) -> Result<()>;
       pub async fn finish_response_body(&mut self) -> Result<()>;
       
       // 缓存相关
       pub fn cache(&self) -> &HttpCache;
   }
   ```

3. **HttpPeer**
   
   定义上游服务器：
   
   ```rust
   pub struct HttpPeer {
       pub _address: SocketAddr,
       pub options: PeerOptions,
   }
   ```

4. **Pingora Cache**
   
   Pingora 内置了缓存系统：
   
   ```rust
   pub struct HttpCache {
       // 缓存后端存储
       storage: Arc<dyn CacheStorage>,
   }
   
   #[async_trait]
   pub trait CacheStorage: Send + Sync {
       async fn lookup(&self, key: &CacheKey) -> Result<Option<CacheMeta>>;
       async fn get_body(&self, key: &CacheKey, range: Option<Range>) -> Result<Option<Bytes>>;
       async fn put(&self, key: &CacheKey, meta: CacheMeta, body: Bytes) -> Result<()>;
   }
   ```

**Slice 模块实现策略：**

由于 Pingora v0.6 没有原生的子请求（subrequest）支持，我们需要采用以下策略：

1. **使用独立的 HTTP 客户端发起子请求**
   - 使用 `pingora::connectors::http::HttpConnector` 创建独立的 HTTP 连接
   - 或使用 `reqwest` 等第三方 HTTP 客户端库

2. **在 `request_filter` 阶段决定是否启用分片**
   - 检查请求方法、URL 模式
   - 返回 `Ok(true)` 继续正常代理流程
   - 返回 `Ok(false)` 表示我们将自己处理响应

3. **自定义响应处理**
   - 当决定使用分片时，在 `request_filter` 中返回 `false`
   - 直接使用 `session.write_response_header()` 和 `session.write_response_body()` 发送响应
   - 在后台并发发起多个 Range 请求到源站

4. **利用 Pingora 的缓存系统**
   - 为每个分片生成唯一的 `CacheKey`
   - 使用 Pingora 的 `HttpCache` 存储和检索分片
   - 缓存键格式：`{url}:slice:{start}-{end}`

### Key Implementation Challenges

1. **无原生子请求支持**
   - Pingora v0.6 没有内置的子请求机制
   - 解决方案：使用 `pingora::connectors::http::HttpConnector` 创建独立连接
   - 或者使用 `reqwest` 等成熟的 HTTP 客户端库

2. **内存管理**
   - 需要缓冲乱序到达的分片
   - 使用有界缓冲区防止内存耗尽
   - 实现背压机制：当缓冲区满时暂停获取新分片

3. **并发控制**
   - 使用 Tokio 的 `Semaphore` 限制并发子请求数
   - 使用 `FuturesUnordered` 或 `tokio::task::JoinSet` 管理多个异步任务
   - 实现请求队列，完成一个后启动下一个

4. **流式传输**
   - 尽早开始向客户端发送数据（收到第一个分片即开始）
   - 维护分片顺序：使用 `BTreeMap<usize, Bytes>` 按索引排序
   - 使用 `session.write_response_body()` 流式发送数据

5. **错误恢复**
   - 实现指数退避重试策略
   - 区分可重试错误（网络超时、5xx）和不可重试错误（4xx）
   - 单个分片失败后中止整个请求

6. **与 Pingora 生命周期集成**
   - 在 `request_filter` 中决定是否启用分片
   - 返回 `Ok(false)` 时需要自己完成整个响应
   - 正确处理客户端断开连接的情况

7. **缓存集成**
   - 使用 Pingora 的 `CacheStorage` trait
   - 为每个分片生成唯一的 `CacheKey`
   - 处理缓存失效和更新

### Configuration Example

```yaml
# pingora_slice.yaml
version: 1
threads: 4

# 上游服务器配置
upstream:
  address: "origin.example.com:80"

# Slice 模块配置
slice:
  # 分片大小（字节）
  slice_size: 1048576  # 1MB
  
  # 最大并发子请求数
  max_concurrent_subrequests: 4
  
  # 最大重试次数
  max_retries: 3
  
  # 启用分片的 URL 模式（正则表达式）
  slice_patterns:
    - "^/large-files/.*"
    - "^/downloads/.*\\.bin$"
  
  # 缓存配置
  cache:
    enabled: true
    ttl: 3600  # 1 hour
    # Pingora 缓存后端
    storage: "file"  # 或 "memory"
    cache_dir: "/var/cache/pingora/slices"
    max_cache_size: 10737418240  # 10GB
    
  # 重试配置
  retry:
    backoff_ms: [100, 200, 400, 800]
```

### Pingora 服务器启动示例

```rust
use pingora::prelude::*;
use pingora::services::listening::Service as ListeningService;

fn main() {
    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    
    // 加载配置
    let config = SliceConfig::from_file("pingora_slice.yaml").unwrap();
    
    // 创建缓存存储
    let cache_storage = if config.cache.enabled {
        Some(Arc::new(
            FileCacheStorage::new(&config.cache.cache_dir, config.cache.max_cache_size)
        ))
    } else {
        None
    };
    
    // 创建 Slice 代理
    let slice_proxy = Arc::new(SliceProxy::new(config, cache_storage));
    
    // 创建 HTTP 代理服务
    let mut proxy_service = http_proxy_service(&server.configuration, slice_proxy);
    proxy_service.add_tcp("0.0.0.0:8080");
    
    // 添加服务到服务器
    server.add_service(proxy_service);
    
    // 运行服务器
    server.run_forever();
}
```

### Performance Considerations

1. **分片大小选择**
   - 太小：过多的子请求开销
   - 太大：缓存效率降低
   - 建议：512KB - 2MB

2. **并发控制**
   - 太少：无法充分利用带宽
   - 太多：可能压垮源站
   - 建议：4 - 8 个并发请求

3. **缓存策略**
   - 使用 LRU 或 LFU 淘汰策略
   - 考虑分片的访问模式
   - 热点分片优先保留

4. **内存使用**
   - 限制缓冲区大小
   - 及时释放已发送的分片
   - 监控内存使用情况

## Security Considerations

1. **资源限制**
   - 限制最大文件大小
   - 限制并发请求数
   - 防止内存耗尽攻击

2. **缓存投毒**
   - 验证 Content-Range 匹配
   - 使用 ETag 验证缓存一致性
   - 定期清理过期缓存

3. **访问控制**
   - 继承原始请求的认证信息
   - 在子请求中传递必要的头部
   - 不缓存包含认证信息的响应

4. **错误信息泄露**
   - 不向客户端暴露内部错误细节
   - 记录详细错误但返回通用错误消息

## Monitoring and Observability

### Metrics

```rust
pub struct SliceMetrics {
    // 请求统计
    pub total_requests: Counter,
    pub sliced_requests: Counter,
    pub passthrough_requests: Counter,
    
    // 缓存统计
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub cache_errors: Counter,
    
    // 子请求统计
    pub total_subrequests: Counter,
    pub failed_subrequests: Counter,
    pub retried_subrequests: Counter,
    
    // 延迟统计
    pub request_duration: Histogram,
    pub subrequest_duration: Histogram,
    pub assembly_duration: Histogram,
    
    // 大小统计
    pub bytes_from_origin: Counter,
    pub bytes_from_cache: Counter,
    pub bytes_to_client: Counter,
}
```

### Logging

```rust
// 请求开始
info!("Slice request started: url={}, client_range={:?}", url, client_range);

// 元数据获取
debug!("Metadata fetched: size={}, supports_range={}", metadata.content_length, metadata.supports_range);

// 分片计算
debug!("Calculated {} slices for file size {}", slices.len(), file_size);

// 子请求
debug!("Subrequest {}/{}: range={}-{}, attempt={}", index, total, start, end, attempt);

// 缓存
debug!("Cache hit for slice {}: {}-{}", index, start, end);

// 错误
error!("Subrequest failed after {} attempts: slice={}, error={}", max_retries, index, error);

// 完成
info!("Slice request completed: url={}, duration={}ms, slices={}, cache_hits={}", 
      url, duration_ms, total_slices, cache_hits);
```

### Tracing

使用 OpenTelemetry 进行分布式追踪：

```rust
#[tracing::instrument(skip(session))]
async fn handle_slice_request(session: &mut Session) -> Result<()> {
    let span = tracing::span!(Level::INFO, "slice_request", url = %session.req_header().uri);
    // ...
}
```

## Detailed Implementation Flow

### 完整的请求处理流程

```rust
#[async_trait]
impl ProxyHttp for SliceProxy {
    type CTX = SliceContext;
    
    fn new_ctx(&self) -> Self::CTX {
        SliceContext::default()
    }
    
    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool> {
        let req = session.req_header();
        
        // 1. 检查是否应该启用分片
        let analyzer = RequestAnalyzer::new(&self.config);
        if !analyzer.should_slice(req) {
            // 不使用分片，继续正常代理流程
            return Ok(true);
        }
        
        // 2. 提取客户端的 Range 请求（如果有）
        ctx.client_range = analyzer.extract_client_range(req);
        
        // 3. 获取文件元数据
        let metadata_fetcher = MetadataFetcher::new(&self.http_client);
        let url = req.uri.to_string();
        
        match metadata_fetcher.fetch_metadata(&url, req).await {
            Ok(metadata) => {
                if !metadata.supports_range {
                    // 源站不支持 Range，回退到正常代理
                    return Ok(true);
                }
                ctx.metadata = Some(metadata);
            }
            Err(_) => {
                // 获取元数据失败，回退到正常代理
                return Ok(true);
            }
        }
        
        // 4. 计算需要的分片
        let calculator = SliceCalculator::new(self.config.slice_size);
        let metadata = ctx.metadata.as_ref().unwrap();
        ctx.slices = calculator.calculate_slices(
            metadata.content_length,
            ctx.client_range,
        );
        
        // 5. 检查缓存
        if let Some(cache) = &self.cache_storage {
            let slice_cache = SliceCache::new(cache.clone(), Duration::from_secs(self.config.cache.ttl));
            let cached_slices = slice_cache.lookup_multiple(
                &url,
                &ctx.slices.iter().map(|s| s.range).collect::<Vec<_>>(),
            ).await;
            
            // 标记哪些分片已缓存
            for (idx, _) in cached_slices {
                ctx.slices[idx].cached = true;
            }
        }
        
        // 6. 启用分片模式
        ctx.slice_enabled = true;
        
        // 7. 自己处理响应，不继续正常代理流程
        self.handle_slice_request(session, ctx).await?;
        
        Ok(false)  // 返回 false 表示我们已经处理了响应
    }
    
    async fn upstream_peer(&self, _session: &mut Session, ctx: &mut Self::CTX) -> Result<Box<HttpPeer>> {
        // 如果启用了分片，这个方法不会被调用
        // 否则返回正常的上游服务器
        if ctx.slice_enabled {
            Err(Error::new(ErrorType::InternalError))
        } else {
            Ok(Box::new(HttpPeer::new(
                self.config.upstream_address.parse()?,
                false,
                "".to_string(),
            )))
        }
    }
}

impl SliceProxy {
    async fn handle_slice_request(&self, session: &mut Session, ctx: &SliceContext) -> Result<()> {
        let metadata = ctx.metadata.as_ref().unwrap();
        let url = session.req_header().uri.to_string();
        
        // 1. 发送响应头
        let mut resp_header = ResponseHeader::build(
            if ctx.client_range.is_some() { 206 } else { 200 },
            None,
        )?;
        
        // 设置响应头
        if let Some(range) = ctx.client_range {
            resp_header.insert_header(
                "Content-Range",
                format!("bytes {}-{}/{}", range.start, range.end, metadata.content_length),
            )?;
            resp_header.insert_header("Content-Length", (range.end - range.start + 1).to_string())?;
        } else {
            resp_header.insert_header("Content-Length", metadata.content_length.to_string())?;
        }
        
        if let Some(ct) = &metadata.content_type {
            resp_header.insert_header("Content-Type", ct)?;
        }
        
        resp_header.insert_header("Accept-Ranges", "bytes")?;
        
        session.write_response_header(Box::new(resp_header)).await?;
        
        // 2. 获取需要的分片（未缓存的）
        let slices_to_fetch: Vec<SliceSpec> = ctx.slices.iter()
            .filter(|s| !s.cached)
            .cloned()
            .collect();
        
        // 3. 并发获取分片
        let subrequest_mgr = SubrequestManager::new(
            self.config.max_concurrent_subrequests,
            self.config.max_retries,
        );
        
        let fetch_results = subrequest_mgr.fetch_slices(
            slices_to_fetch,
            &url,
            session.req_header(),
        ).await?;
        
        // 4. 合并缓存的和新获取的分片
        let mut all_slices: BTreeMap<usize, Bytes> = BTreeMap::new();
        
        // 添加缓存的分片
        if let Some(cache) = &self.cache_storage {
            let slice_cache = SliceCache::new(cache.clone(), Duration::from_secs(self.config.cache.ttl));
            for (idx, slice_spec) in ctx.slices.iter().enumerate() {
                if slice_spec.cached {
                    if let Ok(Some(data)) = slice_cache.lookup_slice(&url, &slice_spec.range).await {
                        all_slices.insert(idx, data);
                    }
                }
            }
        }
        
        // 添加新获取的分片
        for result in fetch_results {
            all_slices.insert(result.slice_index, result.data.clone());
            
            // 存储到缓存
            if let Some(cache) = &self.cache_storage {
                let slice_cache = SliceCache::new(cache.clone(), Duration::from_secs(self.config.cache.ttl));
                let slice_spec = &ctx.slices[result.slice_index];
                let _ = slice_cache.store_slice(
                    &url,
                    &slice_spec.range,
                    result.data,
                    &result.headers,
                ).await;
            }
        }
        
        // 5. 按顺序流式发送数据
        for (_idx, data) in all_slices {
            session.write_response_body(data).await?;
        }
        
        // 6. 完成响应
        session.finish_response_body().await?;
        
        // 7. 记录指标
        self.metrics.record_request(true);
        
        Ok(())
    }
}
```

### HTTP 连接器实现

```rust
use pingora::connectors::http::HttpConnector;
use pingora::protocols::http::v1::client::HttpSession as ClientSession;

impl SubrequestManager {
    async fn fetch_single_slice(
        &self,
        slice: &SliceSpec,
        url: &str,
        original_headers: &RequestHeader,
    ) -> Result<SubrequestResult> {
        let mut attempts = 0;
        let mut last_error = None;
        
        while attempts <= self.max_retries {
            match self.try_fetch_slice(slice, url, original_headers).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    attempts += 1;
                    
                    if attempts <= self.max_retries {
                        let backoff = self.retry_policy.backoff_duration(attempts - 1);
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap())
    }
    
    async fn try_fetch_slice(
        &self,
        slice: &SliceSpec,
        url: &str,
        original_headers: &RequestHeader,
    ) -> Result<SubrequestResult> {
        // 解析 URL
        let uri: Uri = url.parse()?;
        let host = uri.host().ok_or_else(|| Error::new(ErrorType::InvalidInput))?;
        let port = uri.port_u16().unwrap_or(80);
        
        // 创建连接
        let peer = HttpPeer::new((host, port), false, host.to_string());
        let mut http_stream = self.http_connector.new_http_stream(&peer).await?;
        
        // 构建请求
        let mut req = RequestHeader::build("GET", uri.path_and_query().map(|p| p.as_str()).unwrap_or("/"), None)?;
        req.insert_header("Host", host)?;
        req.insert_header("Range", format!("bytes={}-{}", slice.range.start, slice.range.end))?;
        
        // 复制必要的原始请求头
        for (name, value) in original_headers.headers.iter() {
            if name != "Host" && name != "Range" && name != "Content-Length" {
                req.insert_header(name, value.to_str()?)?;
            }
        }
        
        // 发送请求
        http_stream.write_request_header(Box::new(req)).await?;
        
        // 读取响应
        let resp_header = http_stream.read_response_header().await?;
        
        // 验证状态码
        if resp_header.status != 206 {
            return Err(Error::new(ErrorType::InvalidHTTPHeader));
        }
        
        // 读取响应体
        let mut body_data = Vec::new();
        loop {
            match http_stream.read_response_body().await? {
                Some(chunk) => body_data.extend_from_slice(&chunk),
                None => break,
            }
        }
        
        Ok(SubrequestResult {
            slice_index: slice.index,
            data: Bytes::from(body_data),
            status: resp_header.status.as_u16(),
            headers: resp_header,
        })
    }
}
```

## Future Enhancements

1. **智能分片大小**
   - 根据文件大小动态调整分片大小
   - 根据网络条件自适应

2. **预取优化**
   - 预测客户端可能请求的范围
   - 提前获取相邻分片

3. **压缩支持**
   - 支持压缩传输
   - 分片级别的压缩

4. **多源支持**
   - 从多个源站并行获取分片
   - 负载均衡和故障转移

5. **更智能的缓存**
   - 基于访问模式的缓存策略
   - 分层缓存（内存 + 磁盘）

6. **HTTP/3 支持**
   - 利用 QUIC 的多路复用特性
   - 优化分片传输

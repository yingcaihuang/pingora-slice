# Pingora Slice 模块

[English](README.md) | [中文](README_zh.md)

一个为 Pingora 代理服务器设计的高性能分片模块，可自动将大文件请求拆分为多个小的 Range 请求，类似于 Nginx Slice 模块。使用 Rust 构建，确保安全性、性能和可靠性。

## 概述

Pingora Slice 模块透明地拦截大文件请求，并将其拆分为更小、更易管理的块（分片）。每个分片使用 HTTP Range 请求独立获取，单独缓存，然后重新组装成完整的响应返回给客户端。这种方法提供了以下好处：

- **提高缓存效率**：小分片更容易缓存和在不同请求间重用
- **减少源站负载**：部分缓存命中意味着需要从源站获取的字节更少
- **更好的可靠性**：失败的分片可以独立重试，无需重新获取整个文件
- **并发获取**：多个分片可以并行获取，加快响应时间
- **带宽优化**：只获取尚未缓存的分片

## 功能特性

- **自动请求分片**：透明地将大文件请求拆分为更小的块，客户端无感知
- **并发获取**：并行获取多个分片，可配置并发限制
- **智能缓存**：缓存单个分片以实现高效重用和部分缓存命中
- **Range 请求支持**：正确处理客户端 Range 请求（部分内容、字节范围）
- **重试逻辑**：失败的子请求自动重试，采用指数退避策略
- **指标端点**：以 Prometheus 格式暴露详细指标，用于监控和可观测性
- **灵活配置**：基于 YAML 的配置，可设置分片大小、并发数、缓存和 URL 模式
- **基于属性的测试**：全面的测试套件，包含基于属性的测试以保证正确性
- **错误处理**：健壮的错误处理，必要时回退到普通代理模式

## 目录

- [快速开始](#快速开始)
- [工作原理](#工作原理)
- [项目结构](#项目结构)
- [核心组件](#核心组件)
- [配置](#配置)
- [构建和运行](#构建和运行)
- [指标和监控](#指标和监控)
- [测试](#测试)
- [部署指南](#部署指南)
- [性能调优](#性能调优)
- [故障排除](#故障排除)
- [需求覆盖](#需求覆盖)
- [贡献](#贡献)
- [许可证](#许可证)

## 快速开始

```bash
# 1. 克隆仓库
git clone <repository-url>
cd pingora-slice

# 2. 构建项目
cargo build --release

# 3. 创建或编辑配置文件
cp examples/pingora_slice.yaml pingora_slice.yaml
# 编辑 pingora_slice.yaml 设置你的 upstream_address

# 4. 运行服务器
./target/release/pingora-slice

# 5. 测试请求
curl -v http://localhost:8080/large-file.bin

# 6. 检查指标（如果启用）
curl http://localhost:9090/metrics
```

## 工作原理

### 请求流程

1. **客户端请求**：客户端发送普通的 GET 请求获取文件
2. **请求分析**：模块根据 URL 模式检查请求是否应该被分片
3. **元数据获取**：向源站发送 HEAD 请求以获取文件大小并检查 Range 支持
4. **分片计算**：根据配置的分片大小将文件划分为分片
5. **缓存查找**：检查哪些分片已经被缓存
6. **并发获取**：使用 Range 请求并行获取缺失的分片
7. **响应组装**：按正确顺序组装分片并流式传输给客户端
8. **缓存存储**：存储新获取的分片以供将来使用

### 示例场景

```
客户端请求：GET /video.mp4（100MB 文件）
分片大小：1MB
需要的分片：100 个分片

缓存状态：
- 分片 0-49：已缓存（来自之前的请求）
- 分片 50-99：未缓存

操作：
1. 立即返回已缓存的分片 0-49
2. 从源站获取分片 50-99（4 个并发请求）
3. 按顺序将所有分片流式传输给客户端
4. 缓存分片 50-99 以供将来使用

结果：
- 仅从源站获取 50MB（50% 缓存命中率）
- 客户端接收完整的 100MB 文件
- 未来的请求可以使用所有 100 个缓存的分片
```

## 项目结构

```
pingora-slice/
├── src/
│   ├── lib.rs                  # 主库入口点和模块导出
│   ├── main.rs                 # 服务器二进制入口点
│   ├── config.rs               # 配置管理和验证
│   ├── models.rs               # 核心数据结构（ByteRange、SliceSpec、FileMetadata）
│   ├── error.rs                # 错误类型和处理
│   ├── proxy.rs                # 主 SliceProxy 实现（ProxyHttp trait）
│   ├── request_analyzer.rs     # 请求分析和模式匹配
│   ├── metadata_fetcher.rs     # 源站元数据获取（HEAD 请求）
│   ├── slice_calculator.rs     # 分片计算逻辑
│   ├── subrequest_manager.rs   # 并发子请求管理
│   ├── response_assembler.rs   # 响应组装和流式传输
│   ├── cache.rs                # 缓存管理（SliceCache）
│   ├── metrics.rs              # 指标收集（SliceMetrics）
│   └── metrics_endpoint.rs     # HTTP 指标端点服务器
├── tests/
│   ├── prop_*.rs               # 基于属性的测试（20 个属性）
│   └── test_*.rs               # 单元测试和集成测试
├── examples/
│   ├── pingora_slice.yaml      # 示例配置文件（详细注释）
│   ├── server_example.rs       # 服务器启动示例
│   ├── cache_example.rs        # 缓存使用示例
│   ├── metrics_example.rs      # 指标收集示例
│   └── *.rs                    # 其他组件示例
├── docs/
│   ├── *.md                    # 详细实现文档
│   ├── DEPLOYMENT.md           # 部署指南
│   ├── DEPLOYMENT_zh.md        # 部署指南（中文）
│   ├── CONFIGURATION.md        # 配置指南
│   └── API.md                  # API 文档
├── Cargo.toml                  # 项目依赖和元数据
├── pingora_slice.yaml          # 默认配置文件
├── README.md                   # 英文文档
└── README_zh.md                # 本文件（中文文档）
```

## 核心组件

### 1. SliceProxy
主代理实现，集成 Pingora 的 `ProxyHttp` trait。协调所有其他组件并管理请求生命周期。

**关键方法：**
- `new()`：使用配置创建新的代理实例
- `new_ctx()`：创建新的请求上下文
- `request_filter()`：决定是否为请求启用分片
- `handle_slice_request()`：处理完整的分片请求流程

### 2. RequestAnalyzer
分析传入请求以确定是否应启用分片。

**检查项：**
- 请求方法是否为 GET
- URL 是否匹配配置的模式
- 请求是否已包含 Range 头
- 返回决策并提取客户端范围（如果存在）

### 3. MetadataFetcher
使用 HEAD 请求从源站服务器获取文件元数据。

**获取内容：**
- Content-Length（文件大小）
- Accept-Ranges 头（Range 支持）
- Content-Type、ETag、Last-Modified
- 验证源站是否支持 Range 请求

### 4. SliceCalculator
根据文件大小和配置计算分片规格。

**功能：**
- 将文件划分为等大小的分片
- 处理最后一个分片（可能更小）
- 支持部分请求（客户端 Range）
- 确保没有间隙或重叠

### 5. SubrequestManager
管理从源站并发获取分片。

**特性：**
- 并发请求限制（基于信号量）
- 指数退避的重试逻辑
- Content-Range 验证
- 错误处理和传播

### 6. ResponseAssembler
组装分片并将响应流式传输给客户端。

**能力：**
- 有序流式传输（保持字节顺序）
- 缓冲乱序到达的分片
- 响应头生成
- 支持 200 和 206 响应

### 7. SliceCache
管理单个分片的缓存。

**操作：**
- 生成唯一的缓存键（URL + 字节范围）
- 使用 TTL 存储分片
- 查找单个或多个分片
- 优雅地处理缓存错误

### 8. SliceMetrics
收集和暴露运行时指标。

**跟踪内容：**
- 请求计数（总数、分片、透传）
- 缓存统计（命中、未命中、命中率）
- 子请求统计（总数、失败、失败率）
- 字节传输（源站、缓存、客户端）
- 延迟（请求、子请求、组装）

### 9. MetricsEndpoint
以 Prometheus 格式暴露指标的 HTTP 服务器。

**端点：**
- `/` - 带链接的索引页面
- `/metrics` - Prometheus 格式指标
- `/health` - 健康检查端点

## 核心数据结构

### ByteRange
表示 HTTP Range 请求的字节范围。

```rust
pub struct ByteRange {
    pub start: u64,  // 起始字节位置（包含）
    pub end: u64,    // 结束字节位置（包含）
}
```

**方法：**
- `new(start, end)`：创建带验证的新范围
- `size()`：返回范围的字节大小
- `is_valid()`：检查范围是否有效（start <= end）
- `from_header(header)`：从 HTTP Range 头解析
- `to_header()`：转换为 HTTP Range 头格式

### SliceSpec
单个分片的规格。

```rust
pub struct SliceSpec {
    pub index: usize,      // 此分片在序列中的索引
    pub range: ByteRange,  // 此分片的字节范围
    pub cached: bool,      // 此分片是否已缓存
}
```

### FileMetadata
来自源站服务器的文件元数据。

```rust
pub struct FileMetadata {
    pub content_length: u64,           // 文件的总字节大小
    pub supports_range: bool,          // 源站是否支持 Range 请求
    pub content_type: Option<String>,  // 文件的内容类型
    pub etag: Option<String>,          // 用于缓存验证的 ETag
    pub last_modified: Option<String>, // 最后修改时间戳
}
```

### SliceConfig
Slice 模块的配置。

```rust
pub struct SliceConfig {
    pub slice_size: usize,                    // 每个分片的大小（64KB - 10MB）
    pub max_concurrent_subrequests: usize,    // 最大并发子请求数
    pub max_retries: usize,                   // 最大重试次数
    pub slice_patterns: Vec<String>,          // 用于分片的 URL 模式
    pub enable_cache: bool,                   // 启用缓存
    pub cache_ttl: u64,                       // 缓存 TTL（秒）
    pub upstream_address: String,             // 上游源站服务器
    pub metrics_endpoint: Option<MetricsEndpointConfig>,  // 指标配置
}
```

**验证规则：**
- `slice_size`：必须在 64KB（65536）到 10MB（10485760）之间
- `max_concurrent_subrequests`：必须大于 0
- `max_retries`：必须大于或等于 0
- `cache_ttl`：启用缓存时必须大于 0

## 配置

模块使用 YAML 文件进行配置。完整的、带详细注释的配置示例请参见 `examples/pingora_slice.yaml`。

### 配置文件位置

服务器按以下顺序查找配置：
1. 命令行参数指定的路径：`./pingora-slice /path/to/config.yaml`
2. 当前目录中的 `pingora_slice.yaml`
3. 如果未找到文件，则回退到默认配置

### 基本配置示例

```yaml
# 分片大小（字节，64KB 到 10MB）
slice_size: 1048576  # 1MB

# 到源站的最大并发子请求数
max_concurrent_subrequests: 4

# 失败子请求的最大重试次数
max_retries: 3

# 启用分片的 URL 模式（正则表达式）
slice_patterns:
  - "^/large-files/.*"
  - "^/downloads/.*\\.bin$"

# 缓存配置
enable_cache: true
cache_ttl: 3600  # 1 小时（秒）

# 上游源站服务器
upstream_address: "origin.example.com:80"

# 可选的指标端点
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### 配置参数

| 参数 | 类型 | 默认值 | 有效范围 | 描述 |
|------|------|---------|----------|------|
| `slice_size` | 整数 | 1048576 | 65536 - 10485760 | 每个分片的字节大小 |
| `max_concurrent_subrequests` | 整数 | 4 | > 0 | 最大并发子请求数 |
| `max_retries` | 整数 | 3 | >= 0 | 最大重试次数 |
| `slice_patterns` | 数组 | [] | - | 用于分片的 URL 正则模式 |
| `enable_cache` | 布尔值 | true | - | 启用分片缓存 |
| `cache_ttl` | 整数 | 3600 | > 0 | 缓存 TTL（秒） |
| `upstream_address` | 字符串 | "127.0.0.1:8080" | - | 源站服务器地址 |
| `metrics_endpoint.enabled` | 布尔值 | false | - | 启用指标端点 |
| `metrics_endpoint.address` | 字符串 | "127.0.0.1:9090" | - | 指标服务器绑定地址 |

### 配置预设

#### 高性能设置（快速网络）
```yaml
slice_size: 4194304              # 4MB
max_concurrent_subrequests: 8    # 更高并发
max_retries: 2                   # 更少重试
cache_ttl: 86400                 # 24 小时
```

#### 保守设置（慢速/不可靠网络）
```yaml
slice_size: 262144               # 256KB
max_concurrent_subrequests: 2    # 更低并发
max_retries: 5                   # 更多重试
cache_ttl: 3600                  # 1 小时
```

#### 最小缓存（频繁变化的内容）
```yaml
slice_size: 1048576              # 1MB
max_concurrent_subrequests: 4    # 标准
max_retries: 3                   # 标准
cache_ttl: 300                   # 5 分钟
```

## 构建和运行

### 前置要求

- **Rust**：版本 1.70 或更高（从 [rustup.rs](https://rustup.rs) 安装）
- **Cargo**：随 Rust 安装一起提供
- **操作系统**：Linux、macOS 或 Windows（生产环境推荐 Linux）

### 构建

```bash
# 克隆仓库
git clone <repository-url>
cd pingora-slice

# 调试模式构建（编译更快，运行更慢）
cargo build

# 发布模式构建（性能优化）
cargo build --release

# 使用所有功能构建
cargo build --release --all-features

# 检查编译错误而不构建
cargo check
```

**构建产物：**
- 调试版：`target/debug/pingora-slice`
- 发布版：`target/release/pingora-slice`

### 运行服务器

#### 使用默认配置

服务器在当前目录中查找 `pingora_slice.yaml`：

```bash
# 使用 cargo 运行（调试模式）
cargo run

# 直接运行发布版二进制文件
./target/release/pingora-slice
```

#### 使用自定义配置

指定自定义配置文件路径：

```bash
# 使用 cargo
cargo run -- /path/to/config.yaml

# 使用发布版二进制文件
./target/release/pingora-slice /path/to/config.yaml

# 使用示例配置
cargo run -- examples/pingora_slice.yaml
```

#### 后台运行

```bash
# 使用 nohup
nohup ./target/release/pingora-slice &

# 使用 systemd（参见下面的部署指南）
systemctl start pingora-slice

# 使用 screen
screen -dmS pingora-slice ./target/release/pingora-slice

# 使用 tmux
tmux new-session -d -s pingora-slice './target/release/pingora-slice'
```

### 运行示例

项目包含演示不同组件的多个示例：

```bash
# 服务器启动示例
cargo run --example server_example

# 缓存使用示例
cargo run --example cache_example

# 指标收集示例
cargo run --example metrics_example

# 指标端点示例
cargo run --example metrics_endpoint_example

# 列出所有示例
cargo run --example
```

详细信息请参见[服务器启动指南](docs/server_startup.md)。

## 指标和监控

Slice 模块提供全面的指标用于监控性能和行为。

### 启用指标端点

在 YAML 文件中配置指标端点：

```yaml
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"  # 仅绑定到本地主机（推荐）
```

**安全提示：**绑定到 `127.0.0.1` 限制只能从本地机器访问。对于外部访问，请使用带身份验证的反向代理。

### 可用端点

| 端点 | 描述 |
|------|------|
| `http://127.0.0.1:9090/` | 带所有端点链接的索引页面 |
| `http://127.0.0.1:9090/metrics` | Prometheus 格式指标 |
| `http://127.0.0.1:9090/health` | 健康检查端点（返回 200 OK） |

### 暴露的指标

#### 请求指标
```
pingora_slice_requests_total              # 处理的总请求数
pingora_slice_sliced_requests_total       # 使用分片处理的请求数
pingora_slice_passthrough_requests_total  # 未使用分片透传的请求数
```

#### 缓存指标
```
pingora_slice_cache_hits_total            # 缓存命中数
pingora_slice_cache_misses_total          # 缓存未命中数
pingora_slice_cache_hit_rate              # 缓存命中率（0-100%）
```

#### 子请求指标
```
pingora_slice_subrequests_total           # 发送的总子请求数
pingora_slice_failed_subrequests_total    # 失败的子请求数
pingora_slice_subrequest_failure_rate     # 失败率（0-100%）
```

#### 字节传输指标
```
pingora_slice_bytes_from_origin_total     # 从源站获取的字节数
pingora_slice_bytes_from_cache_total      # 从缓存提供的字节数
pingora_slice_bytes_to_client_total       # 发送给客户端的字节数
```

#### 延迟指标
```
pingora_slice_request_duration_ms_avg     # 平均请求持续时间（毫秒）
pingora_slice_subrequest_duration_ms_avg  # 平均子请求持续时间（毫秒）
pingora_slice_assembly_duration_ms_avg    # 平均组装持续时间（毫秒）
```

### 使用指标

#### 手动检查

```bash
# 查看所有指标
curl http://127.0.0.1:9090/metrics

# 过滤特定指标
curl http://127.0.0.1:9090/metrics | grep cache_hit

# 检查健康状态
curl http://127.0.0.1:9090/health
```

#### Prometheus 集成

添加到你的 `prometheus.yml`：

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

#### Grafana 仪表板

创建包含以下面板的仪表板：
- 请求速率（请求/秒）
- 缓存命中率（%）
- 子请求失败率（%）
- 带宽使用（字节/秒）
- 延迟百分位数（p50、p95、p99）

详细信息请参见[指标端点文档](docs/metrics_endpoint_implementation.md)。

## 测试

项目包含全面的测试覆盖，包括单元测试和基于属性的测试。

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行所有测试并显示输出
cargo test -- --nocapture

# 仅运行库单元测试
cargo test --lib

# 运行特定测试文件
cargo test --test test_config_loading

# 仅运行基于属性的测试
cargo test --test 'prop_*'

# 使用特定测试名称模式运行
cargo test cache

# 并行运行测试（默认）
cargo test

# 顺序运行测试
cargo test -- --test-threads=1
```

### 测试类别

#### 单元测试
位于 `tests/test_*.rs` 文件中：
- `test_config_loading.rs` - 配置解析和验证
- `test_metadata_fetcher.rs` - 元数据获取逻辑
- `test_subrequest_manager.rs` - 子请求管理
- `test_cache_integration.rs` - 缓存操作
- `test_error_handling.rs` - 错误处理场景
- `test_handle_slice_request.rs` - 请求处理流程
- `test_metrics_endpoint.rs` - 指标端点功能
- `test_integration.rs` - 端到端集成测试

#### 基于属性的测试
位于 `tests/prop_*.rs` 文件中（20 个属性）：

项目使用 [proptest](https://github.com/proptest-rs/proptest) 进行基于属性的测试。每个属性测试：
- 使用随机输入运行 100+ 次迭代
- 验证所有输入的正确性属性
- 标记有相应的设计属性编号
- 引用规范中的特定需求

详细的测试列表请参见英文 README。

## 部署指南

### 生产部署

完整的部署说明请参见 [DEPLOYMENT_zh.md](docs/DEPLOYMENT_zh.md)，包括：
- 系统要求和依赖
- 安装步骤
- Systemd 服务配置
- Nginx 反向代理设置
- 安全加固
- 监控和日志
- 备份和恢复
- 性能调优

### 快速生产设置

```bash
# 1. 构建发布版二进制文件
cargo build --release

# 2. 创建部署目录
sudo mkdir -p /opt/pingora-slice
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo cp pingora_slice.yaml /opt/pingora-slice/

# 3. 创建 systemd 服务
sudo cp deployment/pingora-slice.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable pingora-slice
sudo systemctl start pingora-slice

# 4. 验证服务正在运行
sudo systemctl status pingora-slice
curl http://localhost:9090/health
```

## 性能调优

### 分片大小选择

根据你的使用场景选择分片大小：

| 使用场景 | 推荐大小 | 原因 |
|----------|----------|------|
| 小文件（< 10MB） | 256KB - 512KB | 更好的粒度，更多缓存命中 |
| 中等文件（10-100MB） | 1MB - 2MB | 良好平衡 |
| 大文件（> 100MB） | 2MB - 4MB | 更少请求，更少开销 |
| 超大文件（> 1GB） | 4MB - 10MB | 最小化请求开销 |
| 慢速网络 | 256KB - 512KB | 更小块，更好可靠性 |
| 快速网络 | 2MB - 4MB | 最大化吞吐量 |

### 并发调优

根据以下情况调整并发子请求：

```yaml
# 低端源站（共享主机，有限带宽）
max_concurrent_subrequests: 2

# 标准源站（专用服务器，中等带宽）
max_concurrent_subrequests: 4

# 高性能源站（CDN，高带宽）
max_concurrent_subrequests: 8

# 超高性能源站（多服务器，负载均衡）
max_concurrent_subrequests: 16
```

### 缓存调优

优化缓存设置：

```yaml
# 频繁变化的内容
cache_ttl: 300  # 5 分钟

# 中等稳定的内容
cache_ttl: 3600  # 1 小时

# 静态内容
cache_ttl: 86400  # 24 小时

# 很少变化的内容
cache_ttl: 604800  # 7 天
```

### 系统级调优

#### Linux 内核参数

```bash
# 增加文件描述符限制
ulimit -n 65536

# 高吞吐量的 TCP 调优
sudo sysctl -w net.core.rmem_max=134217728
sudo sysctl -w net.core.wmem_max=134217728
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"
```

### 监控性能

要监控的关键指标：
- **缓存命中率**：应 > 70% 以获得最佳性能
- **子请求失败率**：应 < 1%
- **平均请求持续时间**：建立基线并监控增长
- **缓存与源站的字节数**：更高的缓存比率更好

## 故障排除

### 常见问题

#### 1. 服务器无法启动

**症状：**服务器启动后立即退出

**可能原因：**
- 配置文件无效
- 端口已被占用
- 权限不足

**解决方案：**
```bash
# 检查配置有效性
cargo run -- --check-config

# 检查端口是否被占用
sudo lsof -i :8080
sudo lsof -i :9090

# 检查日志
journalctl -u pingora-slice -n 50

# 使用详细日志运行
RUST_LOG=debug ./target/release/pingora-slice
```

#### 2. 高缓存未命中率

**症状：**缓存命中率 < 50%

**可能原因：**
- 分片大小太大
- 缓存 TTL 太短
- 缓存存储不足
- URL 中的查询参数变化

**解决方案：**
```yaml
# 减小分片大小以获得更好的粒度
slice_size: 524288  # 512KB

# 增加缓存 TTL
cache_ttl: 7200  # 2 小时

# 规范化 URL（去除查询参数）
# 增加缓存存储容量
```

#### 3. 源站服务器过载

**症状：**许多失败的子请求，响应缓慢

**可能原因：**
- 并发子请求太多
- 重试退避不足
- 源站速率限制

**解决方案：**
```yaml
# 减少并发
max_concurrent_subrequests: 2

# 增加重试退避
max_retries: 5

# 增加分片大小以减少请求数
slice_size: 2097152  # 2MB
```

#### 4. 内存使用增长

**症状：**内存消耗随时间增加

**可能原因：**
- 包含许多分片的大文件
- 缓冲太多乱序分片
- 内存泄漏（报告为 bug）

**解决方案：**
```bash
# 监控内存使用
ps aux | grep pingora-slice

# 定期重启服务（临时）
sudo systemctl restart pingora-slice

# 减小分片大小
# 限制并发请求
# 报告问题并提供重现步骤
```

#### 5. 指标端点无法访问

**症状：**无法访问 http://localhost:9090/metrics

**可能原因：**
- 指标端点未启用
- 绑定地址错误
- 防火墙阻止端口

**解决方案：**
```yaml
# 确保指标已启用
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

```bash
# 检查端口是否在监听
sudo netstat -tlnp | grep 9090

# 检查防火墙
sudo ufw status
sudo firewall-cmd --list-ports

# 本地测试
curl http://127.0.0.1:9090/health
```

### 调试日志

启用调试日志进行故障排除：

```bash
# 通过环境变量设置日志级别
RUST_LOG=debug ./target/release/pingora-slice

# 不同的日志级别
RUST_LOG=trace  # 非常详细
RUST_LOG=debug  # 详细信息
RUST_LOG=info   # 一般信息（默认）
RUST_LOG=warn   # 仅警告
RUST_LOG=error  # 仅错误

# 模块特定日志
RUST_LOG=pingora_slice::proxy=debug,pingora_slice::cache=trace
```

### 获取帮助

如果遇到问题：
1. 检查此故障排除部分
2. 启用调试日志查看日志
3. 检查 GitHub issues 中的类似问题
4. 提交新 issue 并包含：
   - 配置文件（已脱敏）
   - 错误消息和日志
   - 重现步骤
   - 环境详情（操作系统、Rust 版本）

## 需求覆盖

此实现满足规范中的所有需求：

### 需求 1：配置管理
- ✓ 1.1：从配置文件加载分片大小
- ✓ 1.2：验证分片大小在 64KB 到 10MB 之间
- ✓ 1.3：如果未配置则使用默认值 1MB
- ✓ 1.4：记录错误并在配置无效时拒绝启动

### 需求 2：请求检测
- ✓ 2.1：检查请求方法是否为 GET
- ✓ 2.2：确定是否应启用分片
- ✓ 2.3：透传包含 Range 头的请求
- ✓ 2.4：将请求 URL 与配置的模式匹配

### 需求 3：元数据获取
- ✓ 3.1：向源站服务器发送 HEAD 请求
- ✓ 3.2：从响应中提取 Content-Length
- ✓ 3.3：检查 Accept-Ranges 头以确认 Range 支持
- ✓ 3.4：如果不支持 Range 则回退到普通代理
- ✓ 3.5：如果 Content-Length 缺失或无效则回退

### 需求 4：分片计算
- ✓ 4.1：计算所需的分片数量
- ✓ 4.2：生成具有正确字节范围的 Range 头
- ✓ 4.3：确保最后一个分片覆盖剩余字节
- ✓ 4.4：创建子请求规格列表

### 需求 5：并发获取
- ✓ 5.1：并发发送多个子请求
- ✓ 5.2：限制并发子请求数量
- ✓ 5.3：一个完成时启动下一个子请求
- ✓ 5.4：重试失败的子请求直到最大重试次数
- ✓ 5.5：如果所有重试都失败则中止整个请求

### 需求 6：响应组装
- ✓ 6.1：立即开始流式传输数据
- ✓ 6.2：保持正确的字节顺序
- ✓ 6.3：缓冲乱序到达的分片
- ✓ 6.4：收到所有分片时完成响应
- ✓ 6.5：设置适当的响应头

### 需求 7：缓存
- ✓ 7.1：使用唯一键将分片存储在缓存中
- ✓ 7.2：在缓存键中包含 URL 和字节范围
- ✓ 7.3：在创建子请求之前检查缓存
- ✓ 7.4：使用缓存数据并仅请求缺失的分片
- ✓ 7.5：缓存失败时记录警告并继续

### 需求 8：错误处理
- ✓ 8.1：将 4xx 错误返回给客户端
- ✓ 8.2：重试 5xx 错误直到配置的限制
- ✓ 8.3：验证 Content-Range 与请求匹配
- ✓ 8.4：将 Content-Range 不匹配视为错误
- ✓ 8.5：在意外状态码时返回 502 Bad Gateway

### 需求 9：监控
- ✓ 9.1：记录请求和缓存命中的指标
- ✓ 9.2：记录子请求计数和延迟
- ✓ 9.3：记录详细的错误信息
- ✓ 9.4：完成时记录摘要信息
- ✓ 9.5：通过 HTTP 端点暴露指标

### 需求 10：Range 请求支持
- ✓ 10.1：解析客户端 Range 头
- ✓ 10.2：计算请求范围的分片
- ✓ 10.3：仅请求和返回必要的分片
- ✓ 10.4：返回 206 和正确的 Content-Range
- ✓ 10.5：对无效范围返回 416

## 贡献

欢迎贡献！请遵循以下指南：

### 开发设置

```bash
# Fork 并克隆仓库
git clone https://github.com/yourusername/pingora-slice.git
cd pingora-slice

# 创建功能分支
git checkout -b feature/your-feature-name

# 进行更改并测试
cargo test
cargo clippy
cargo fmt

# 提交并推送
git commit -m "Add your feature"
git push origin feature/your-feature-name

# 打开 pull request
```

### 代码风格

- 遵循 Rust 标准风格（使用 `cargo fmt`）
- 运行 `cargo clippy` 并修复警告
- 为公共 API 添加文档注释
- 为新功能编写测试
- 根据需要更新文档

### Pull Request 流程

1. 确保所有测试通过
2. 如需要更新 README.md
3. 添加条目到 CHANGELOG.md
4. 请求维护者审查
5. 处理审查反馈
6. 如果要求则压缩提交

### 报告问题

报告 bug 时，请包含：
- Rust 版本（`rustc --version`）
- 操作系统和版本
- 配置文件（已脱敏）
- 重现步骤
- 预期与实际行为
- 相关日志

## 许可证

[在此添加你的许可证]

## 致谢

- 基于 [Cloudflare Pingora](https://github.com/cloudflare/pingora) 构建
- 灵感来自 [Nginx Slice Module](http://nginx.org/en/docs/http/ngx_http_slice_module.html)
- 使用 [proptest](https://github.com/proptest-rs/proptest) 进行基于属性的测试

## 相关项目

- [Pingora](https://github.com/cloudflare/pingora) - 基于 Rust 的 HTTP 代理框架
- [Nginx Slice Module](http://nginx.org/en/docs/http/ngx_http_slice_module.html) - 原始灵感来源
- [Varnish](https://varnish-cache.org/) - 具有类似功能的 HTTP 加速器

## 文档

- [English README](README.md) - 英文文档
- [部署指南（中文）](docs/DEPLOYMENT_zh.md) - 详细的部署说明
- [配置指南](docs/CONFIGURATION.md) - 配置参数详解（英文）
- [API 文档](docs/API.md) - API 参考（英文）

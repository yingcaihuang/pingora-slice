# Implementation Plan - Pingora Slice Module

- [x] 1. 设置项目结构和核心接口
  - 创建 Rust 项目结构（使用 Cargo）
  - 添加 Pingora 依赖和其他必要的 crate（tokio, bytes, serde 等）
  - 定义核心数据结构：`ByteRange`, `SliceSpec`, `FileMetadata`
  - 定义配置结构 `SliceConfig` 和配置加载逻辑
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2. 实现配置管理模块
  - 实现 `SliceConfig::from_file()` 从 YAML 文件加载配置
  - 实现 `SliceConfig::validate()` 验证配置参数
  - 实现配置默认值逻辑
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 2.1 编写配置验证的属性测试
  - **Property 1: 配置值范围验证**
  - **Validates: Requirements 1.2, 1.4**

- [x] 3. 实现 ByteRange 和相关数据模型
  - 实现 `ByteRange` 结构体及其方法（`new`, `size`, `is_valid`）
  - 实现 `ByteRange::from_header()` 解析 HTTP Range 头
  - 实现 `ByteRange::to_header()` 生成 HTTP Range 头
  - 实现 `SliceSpec` 和 `FileMetadata` 结构体
  - _Requirements: 10.1_

- [x] 3.1 编写 Range 解析的属性测试
  - **Property 17: Range 解析正确性**
  - **Validates: Requirements 10.1**

- [x] 4. 实现请求分析器（RequestAnalyzer）
  - 实现 `RequestAnalyzer::should_slice()` 判断是否启用分片
  - 检查请求方法是否为 GET
  - 检查 URL 是否匹配配置的模式
  - 检查请求是否已包含 Range 头（如有则不分片）
  - 实现 `RequestAnalyzer::extract_client_range()` 提取客户端 Range
  - _Requirements: 2.1, 2.2, 2.3, 2.4_

- [x] 4.1 编写请求分析的属性测试
  - **Property 2: Range 请求透传**
  - **Validates: Requirements 2.3**

- [x] 4.2 编写 URL 模式匹配的属性测试
  - **Property 3: URL 模式匹配一致性**
  - **Validates: Requirements 2.4**

- [x] 5. 实现元数据获取器（MetadataFetcher）
  - 使用 Pingora 的 `HttpConnector` 创建 HTTP 客户端
  - 实现 `fetch_metadata()` 发送 HEAD 请求到源站
  - 解析响应头获取 `Content-Length`
  - 检查 `Accept-Ranges` 头判断是否支持 Range 请求
  - 提取 `Content-Type`, `ETag`, `Last-Modified` 等元数据
  - 处理错误情况（4xx, 5xx, 缺失头部等）
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [x] 6. 实现分片计算器（SliceCalculator）
  - 实现 `calculate_slices()` 计算文件的所有分片
  - 根据文件大小和分片大小计算分片数量
  - 为每个分片生成正确的字节范围
  - 确保最后一个分片覆盖到文件末尾
  - 支持客户端 Range 请求的分片计算（只计算需要的部分）
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 10.2, 10.3_

- [x] 6.1 编写分片覆盖完整性的属性测试
  - **Property 4: 分片覆盖完整性**
  - **Validates: Requirements 4.1, 4.2**

- [x] 6.2 编写分片无重叠性的属性测试
  - **Property 5: 分片无重叠性**
  - **Validates: Requirements 4.2**

- [x] 6.3 编写 Range 头格式的属性测试
  - **Property 6: Range 头格式正确性**
  - **Validates: Requirements 4.2**

- [x] 6.4 编写部分请求分片计算的属性测试
  - **Property 18: 部分请求分片计算**
  - **Validates: Requirements 10.2, 10.3**

- [x] 7. 实现缓存管理器（SliceCache）
  - 集成 Pingora 的 `CacheStorage` trait
  - 实现 `generate_cache_key()` 为分片生成唯一缓存键
  - 实现 `lookup_slice()` 查找单个缓存分片
  - 实现 `store_slice()` 存储分片到缓存
  - 实现 `lookup_multiple()` 批量查找多个分片
  - 处理缓存错误（记录警告但继续处理）
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 7.1 编写缓存键唯一性的属性测试
  - **Property 12: 缓存键唯一性**
  - **Validates: Requirements 7.2**

- [x] 7.2 编写缓存命中正确性的属性测试
  - **Property 13: 缓存命中正确性**
  - **Validates: Requirements 7.4**

- [x] 7.3 编写部分缓存命中优化的属性测试
  - **Property 14: 部分缓存命中优化**
  - **Validates: Requirements 7.4**

- [x] 8. 实现子请求管理器（SubrequestManager）
  - 创建 `HttpConnector` 用于发起子请求
  - 实现 `build_range_request()` 构建 Range 请求头
  - 实现 `try_fetch_slice()` 发起单个分片请求
  - 实现重试逻辑和指数退避
  - 实现 `fetch_single_slice()` 带重试的分片获取
  - 验证响应状态码（期望 206）
  - 验证 `Content-Range` 头是否匹配请求
  - _Requirements: 5.4, 8.3, 8.4_

- [x] 8.1 编写重试次数限制的属性测试
  - **Property 8: 重试次数限制**
  - **Validates: Requirements 5.4**

- [x] 8.2 编写 Content-Range 验证的属性测试
  - **Property 16: Content-Range 验证**
  - **Validates: Requirements 8.3, 8.4**

- [x] 9. 实现并发子请求管理
  - 使用 Tokio 的 `Semaphore` 限制并发数
  - 实现 `fetch_slices()` 并发获取多个分片
  - 使用 `tokio::task::JoinSet` 或 `FuturesUnordered` 管理异步任务
  - 实现请求队列：完成一个后启动下一个
  - 处理单个分片失败：中止整个请求
  - _Requirements: 5.1, 5.2, 5.3, 5.5_

- [x] 9.1 编写并发限制遵守的属性测试
  - **Property 7: 并发限制遵守**
  - **Validates: Requirements 5.2**

- [x] 9.2 编写失败传播的属性测试
  - **Property 9: 失败传播**
  - **Validates: Requirements 5.5**

- [x] 10. 实现响应组装器（ResponseAssembler）
  - 使用 `BTreeMap<usize, Bytes>` 按索引存储分片
  - 实现 `build_response_header()` 构建响应头
  - 根据是否为 Range 请求设置正确的状态码（200 或 206）
  - 设置 `Content-Length`, `Content-Type`, `Content-Range` 等头部
  - 实现流式发送：按顺序发送分片数据
  - 处理乱序到达的分片（缓冲后续分片直到前面的到达）
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 10.4_

- [x] 10.1 编写字节顺序保持的属性测试
  - **Property 10: 字节顺序保持（关键属性）**
  - **Validates: Requirements 6.2**

- [x] 10.2 编写响应头完整性的属性测试
  - **Property 11: 响应头完整性**
  - **Validates: Requirements 6.5**

- [x] 10.3 编写 206 响应格式的属性测试
  - **Property 19: 206 响应格式**
  - **Validates: Requirements 10.4**

- [x] 11. 实现错误处理逻辑
  - 定义 `SliceError` 枚举包含所有错误类型
  - 实现 `should_retry()` 判断错误是否可重试
  - 实现 `to_http_status()` 将错误转换为 HTTP 状态码
  - 实现 `fallback_to_normal_proxy()` 判断是否回退到普通代理
  - 处理 4xx 错误：直接返回给客户端
  - 处理 5xx 错误：重试
  - 处理无效 Range：返回 416
  - _Requirements: 8.1, 8.2, 8.5, 10.5_

- [x] 11.1 编写 4xx 错误透传的属性测试
  - **Property 15: 4xx 错误透传**
  - **Validates: Requirements 8.1**

- [x] 11.2 编写无效 Range 错误处理的属性测试
  - **Property 20: 无效 Range 错误处理**
  - **Validates: Requirements 10.5**

- [x] 12. 实现指标收集器（SliceMetrics）
  - 定义 `SliceMetrics` 结构体包含各种计数器和直方图
  - 实现 `record_request()` 记录请求统计
  - 实现 `record_cache_hit()` 和 `record_cache_miss()` 记录缓存统计
  - 实现 `record_subrequest()` 记录子请求统计
  - 实现 `get_stats()` 获取指标快照
  - 使用原子操作确保线程安全
  - _Requirements: 9.1, 9.2_

- [x] 13. 实现 SliceProxy 主结构
  - 定义 `SliceProxy` 结构体包含配置、HTTP 客户端、缓存、指标
  - 定义 `SliceContext` 存储请求上下文信息
  - 实现 `new()` 构造函数
  - 实现 `new_ctx()` 创建请求上下文
  - _Requirements: 所有需求_

- [x] 14. 实现 ProxyHttp trait 的 request_filter
  - 实现 `request_filter()` 方法
  - 调用 `RequestAnalyzer` 判断是否启用分片
  - 调用 `MetadataFetcher` 获取文件元数据
  - 检查源站是否支持 Range 请求
  - 调用 `SliceCalculator` 计算分片
  - 检查缓存中已有的分片
  - 决定是否启用分片模式（返回 true 或 false）
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 4.4, 7.3_

- [x] 15. 实现 handle_slice_request 核心逻辑
  - 实现 `handle_slice_request()` 方法处理分片请求
  - 构建并发送响应头到客户端
  - 调用 `SubrequestManager` 并发获取未缓存的分片
  - 合并缓存的分片和新获取的分片
  - 将新获取的分片存储到缓存
  - 按顺序流式发送所有分片数据到客户端
  - 完成响应并记录指标
  - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 6.1, 6.2, 6.3, 6.4, 6.5, 7.1, 7.4, 7.5_

- [x] 16. 实现 ProxyHttp trait 的其他方法
  - 实现 `upstream_peer()` 返回上游服务器（用于非分片模式）
  - 实现 `logging()` 记录请求日志
  - _Requirements: 9.3, 9.4_

- [x] 17. 实现日志记录
  - 在关键步骤添加日志记录
  - 记录请求开始、元数据获取、分片计算、子请求、缓存命中/未命中
  - 记录错误详情
  - 记录请求完成和性能指标
  - _Requirements: 9.3, 9.4_

- [x] 18. 创建 Pingora 服务器启动代码
  - 实现 `main()` 函数
  - 加载配置文件
  - 创建缓存存储（文件或内存）
  - 创建 `SliceProxy` 实例
  - 创建 HTTP 代理服务
  - 配置监听地址和端口
  - 启动服务器
  - _Requirements: 1.1_

- [x] 19. 添加指标暴露端点（可选）
  - 实现 HTTP 端点暴露指标
  - 支持 Prometheus 格式
  - _Requirements: 9.5_

- [x] 20. 第一次检查点 - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

- [x] 21. 创建示例配置文件
  - 创建 `examples/pingora_slice.yaml` 配置示例
  - 添加注释说明各个配置项
  - _Requirements: 1.1_

- [x] 22. 编写集成测试
  - 创建模拟源站服务器
  - 测试完整的端到端流程
  - 测试缓存命中和未命中场景
  - 测试并发请求
  - 测试错误场景（源站错误、网络超时等）
  - 测试客户端 Range 请求
  - _Requirements: 所有需求_

- [x] 23. 编写文档
  - 编写 README.md 说明项目用途和使用方法
  - 编写配置文档
  - 编写部署指南
  - 添加代码注释和文档注释
  - _Requirements: 所有需求_

- [x] 24. 性能优化和调优
  - 分析内存使用情况
  - 优化缓冲区大小
  - 调整默认配置参数
  - 进行压力测试
  - _Requirements: 所有需求_

- [x] 25. 最终检查点 - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

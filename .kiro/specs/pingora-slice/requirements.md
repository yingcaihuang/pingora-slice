# Requirements Document

## Introduction

本文档定义了 Pingora Slice 模块的需求规范。该模块为 Pingora 代理服务器提供自动分片回源功能，类似于 Nginx Slice 模块。当客户端请求完整文件时，代理服务器自动将请求拆分为多个小的 Range 请求去回源，收到后分别存储，并拼装成完整文件返回给客户端。这种机制可以提高大文件的缓存效率，减少源站压力，并支持断点续传。

## Glossary

- **Pingora**: Cloudflare 开源的 Rust 编写的高性能代理服务器框架
- **Slice Module**: 分片模块，负责将大文件请求拆分为多个小的 Range 请求
- **Range Request**: HTTP Range 请求，用于请求文件的特定字节范围
- **Subrequest**: 子请求，Pingora 内部发起的额外 HTTP 请求
- **Origin Server**: 源站服务器，提供原始内容的服务器
- **Client**: 客户端，发起请求的用户端
- **Proxy Server**: 代理服务器，运行 Pingora 的服务器
- **Slice Size**: 分片大小，每个 Range 请求的字节数
- **Content-Length**: HTTP 响应头，表示内容的总字节数
- **Content-Range**: HTTP 响应头，表示返回内容的字节范围

## Requirements

### Requirement 1

**User Story:** 作为代理服务器管理员，我希望能够配置分片大小，以便根据网络环境和文件特性优化性能。

#### Acceptance Criteria

1. WHEN the Proxy Server starts THEN the Slice Module SHALL load the configured slice size from configuration file
2. WHERE slice size is configured, THE Slice Module SHALL validate that the value is between 64KB and 10MB
3. IF slice size is not configured THEN the Slice Module SHALL use a default value of 1MB
4. WHEN configuration is invalid THEN the Slice Module SHALL log an error and refuse to start

### Requirement 2

**User Story:** 作为代理服务器，我希望能够检测客户端请求是否适合分片处理，以便只对合适的请求启用分片功能。

#### Acceptance Criteria

1. WHEN a Client sends a request without Range header THEN the Slice Module SHALL check if the request method is GET
2. WHEN the request method is GET THEN the Slice Module SHALL determine if slicing should be enabled based on configuration
3. IF the Client request contains a Range header THEN the Slice Module SHALL pass the request through without slicing
4. WHEN the request URL matches configured slice patterns THEN the Slice Module SHALL mark the request for slice processing

### Requirement 3

**User Story:** 作为代理服务器，我希望能够向源站发送 HEAD 请求获取文件元信息，以便确定文件大小和是否支持 Range 请求。

#### Acceptance Criteria

1. WHEN the Slice Module processes a request THEN the Proxy Server SHALL send a HEAD request to the Origin Server
2. WHEN the Origin Server responds to HEAD request THEN the Slice Module SHALL extract Content-Length from response headers
3. IF the Origin Server response contains Accept-Ranges header with value "bytes" THEN the Slice Module SHALL proceed with slicing
4. IF the Origin Server does not support Range requests THEN the Slice Module SHALL fall back to normal proxy mode
5. WHEN Content-Length is missing or invalid THEN the Slice Module SHALL fall back to normal proxy mode

### Requirement 4

**User Story:** 作为代理服务器，我希望能够将完整文件请求拆分为多个 Range 子请求，以便实现分片回源。

#### Acceptance Criteria

1. WHEN file size and slice size are known THEN the Slice Module SHALL calculate the number of slices needed
2. WHEN creating subrequests THEN the Slice Module SHALL generate Range headers with correct byte ranges for each slice
3. WHEN generating the last slice THEN the Slice Module SHALL ensure the Range header covers remaining bytes to file end
4. WHEN all slices are calculated THEN the Slice Module SHALL create a list of subrequest specifications

### Requirement 5

**User Story:** 作为代理服务器，我希望能够并发发送多个子请求到源站，以便提高回源效率。

#### Acceptance Criteria

1. WHEN subrequests are ready THEN the Slice Module SHALL send multiple subrequests concurrently to the Origin Server
2. WHERE concurrency limit is configured, THE Slice Module SHALL limit the number of concurrent subrequests
3. WHEN a subrequest completes THEN the Slice Module SHALL initiate the next pending subrequest if any remain
4. WHEN a subrequest fails THEN the Slice Module SHALL retry the subrequest up to a configured maximum retry count
5. IF all retries for a subrequest fail THEN the Slice Module SHALL abort the entire request and return an error to the Client

### Requirement 6

**User Story:** 作为代理服务器，我希望能够按顺序将接收到的分片数据流式传输给客户端，以便客户端能够尽快开始接收数据。

#### Acceptance Criteria

1. WHEN the first slice response is received THEN the Slice Module SHALL immediately start streaming data to the Client
2. WHILE streaming data to Client, THE Slice Module SHALL maintain the correct byte order of slices
3. WHEN a later slice arrives before an earlier slice THEN the Slice Module SHALL buffer the later slice until earlier slices arrive
4. WHEN all slices for the file are received THEN the Slice Module SHALL complete the response to the Client
5. WHEN streaming to Client THEN the Slice Module SHALL set appropriate response headers including Content-Length and Content-Type

### Requirement 7

**User Story:** 作为代理服务器，我希望能够缓存每个分片，以便后续相同文件的请求可以直接从缓存获取。

#### Acceptance Criteria

1. WHEN a slice response is received from Origin Server THEN the Slice Module SHALL store the slice in cache with a unique key
2. WHEN generating cache keys THEN the Slice Module SHALL include file URL and byte range in the key
3. WHEN processing a new request THEN the Slice Module SHALL check cache for existing slices before creating subrequests
4. WHEN cached slices are found THEN the Slice Module SHALL use cached data and only request missing slices from Origin Server
5. WHEN cache storage fails THEN the Slice Module SHALL log a warning and continue without caching

### Requirement 8

**User Story:** 作为代理服务器，我希望能够处理源站返回的错误响应，以便向客户端提供合适的错误信息。

#### Acceptance Criteria

1. WHEN the Origin Server returns a 4xx error for HEAD request THEN the Slice Module SHALL return the same error to the Client
2. WHEN the Origin Server returns a 5xx error for HEAD request THEN the Slice Module SHALL retry the request up to configured limit
3. WHEN a subrequest receives a 206 Partial Content response THEN the Slice Module SHALL validate the Content-Range header matches the request
4. IF Content-Range does not match the requested range THEN the Slice Module SHALL treat it as an error and retry
5. WHEN the Origin Server returns unexpected status code for subrequest THEN the Slice Module SHALL abort and return 502 Bad Gateway to the Client

### Requirement 9

**User Story:** 作为系统管理员，我希望能够监控分片模块的运行状态，以便了解性能和排查问题。

#### Acceptance Criteria

1. WHEN the Slice Module processes requests THEN the Proxy Server SHALL record metrics including total requests, sliced requests, and cache hits
2. WHEN subrequests are sent THEN the Slice Module SHALL record the number of subrequests and their latencies
3. WHEN errors occur THEN the Slice Module SHALL log detailed error information including request URL and error type
4. WHEN slice processing completes THEN the Slice Module SHALL log summary information including total time and number of slices
5. WHERE metrics endpoint is configured, THE Proxy Server SHALL expose slice metrics via HTTP endpoint

### Requirement 10

**User Story:** 作为开发者，我希望分片模块能够正确处理客户端的 Range 请求，以便支持断点续传和部分内容请求。

#### Acceptance Criteria

1. WHEN a Client sends a request with Range header THEN the Slice Module SHALL parse the requested byte range
2. WHEN the requested range is valid THEN the Slice Module SHALL calculate which slices are needed to fulfill the request
3. WHEN fetching slices for a Range request THEN the Slice Module SHALL only request and return the necessary slices
4. WHEN responding to a Range request THEN the Slice Module SHALL return 206 Partial Content with correct Content-Range header
5. IF the requested range is invalid or unsatisfiable THEN the Slice Module SHALL return 416 Range Not Satisfiable to the Client

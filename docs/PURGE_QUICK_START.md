# Purge 功能快速开始

## 一句话说明

**Purge 是 Pingora Slice 的内置功能，只需在配置文件中启用即可使用。**

## 3 步启用 Purge

### 1. 编辑配置文件

在 `pingora_slice.yaml` 中添加：

```yaml
purge:
  enabled: true
  auth_token: "your-secret-token"
  enable_metrics: true
```

### 2. 启动服务

```bash
./pingora-slice -c pingora_slice.yaml
```

### 3. 使用 PURGE

```bash
curl -X PURGE http://your-server.com/file.dat \
  -H "Authorization: Bearer your-secret-token"
```

## 就这么简单！

- ✅ 不需要单独部署 Purge 服务
- ✅ 不需要额外的端口
- ✅ 不需要复杂的配置
- ✅ 启用 Pingora Slice = 自动获得 Purge 功能

## 完整配置示例

```yaml
# pingora_slice.yaml
slice_size: 1048576
enable_cache: true
cache_ttl: 3600
upstream_address: "origin.example.com:80"

# 启用 Purge（就这一个配置块）
purge:
  enabled: true
  auth_token: "my-secret-token-2024"
  enable_metrics: true
```

## 验证 Purge 是否启用

```bash
# 发送测试 PURGE 请求
curl -X PURGE http://localhost:8080/test \
  -H "Authorization: Bearer my-secret-token-2024"

# 如果返回 JSON 响应，说明 Purge 已启用
# 如果返回 404/405，说明 Purge 未启用
```

## 常用操作

```bash
# 清除单个文件
curl -X PURGE http://your-server.com/video.mp4 \
  -H "Authorization: Bearer your-token"

# 清除所有缓存
curl -X PURGE http://your-server.com/* \
  -H "X-Purge-All: true" \
  -H "Authorization: Bearer your-token"

# 查看 Purge 指标
curl http://your-server.com:9090/metrics | grep purge
```

## 架构图

```
┌──────────────────────────────────────┐
│      Pingora Slice 服务               │
│  (一个服务，两个功能)                  │
│                                       │
│  ┌─────────────┐  ┌──────────────┐  │
│  │ GET 请求    │  │ PURGE 请求   │  │
│  │ (Slice处理) │  │ (缓存清除)   │  │
│  └──────┬──────┘  └──────┬───────┘  │
│         │                 │          │
│         └────────┬────────┘          │
│                  ▼                   │
│         ┌────────────────┐           │
│         │  两层缓存系统   │           │
│         └────────────────┘           │
└──────────────────────────────────────┘
```

## 下一步

- 查看 [完整集成指南](PURGE_INTEGRATION_GUIDE.md)
- 查看 [HTTP PURGE 参考](HTTP_PURGE_REFERENCE.md)
- 查看 [配置和指标](PURGE_CONFIG_AND_METRICS.md)

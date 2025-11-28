# Pingora Slice 快速开始指南

## 安装

### 使用 RPM 包安装（推荐）

#### CentOS 8 / Rocky Linux 8 / AlmaLinux 8

```bash
# 使用安装脚本
curl -sSL https://raw.githubusercontent.com/your-username/pingora-slice/main/packaging/install.sh | sudo bash

# 或手动下载安装
VERSION=0.1.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el8.x86_64.rpm
sudo dnf install -y ./pingora-slice-${VERSION}-1.el8.x86_64.rpm
```

#### CentOS 9 / Rocky Linux 9 / AlmaLinux 9

```bash
# 使用安装脚本
curl -sSL https://raw.githubusercontent.com/your-username/pingora-slice/main/packaging/install.sh | sudo bash

# 或手动下载安装
VERSION=0.1.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el9.x86_64.rpm
sudo dnf install -y ./pingora-slice-${VERSION}-1.el9.x86_64.rpm
```

### 从源码编译安装

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆仓库
git clone https://github.com/your-username/pingora-slice.git
cd pingora-slice

# 编译
cargo build --release

# 安装
sudo cp target/release/pingora-slice /usr/local/bin/
sudo mkdir -p /etc/pingora-slice
sudo cp examples/pingora_slice.yaml /etc/pingora-slice/
```

## 配置

### 基础配置

编辑配置文件 `/etc/pingora-slice/pingora_slice.yaml`：

```yaml
# 监听地址
listen:
  address: "0.0.0.0:8080"

# 上游服务器
upstream:
  address: "origin.example.com:80"
  timeout: 30

# Slice 配置
slice:
  # 分片大小（1MB）
  slice_size: 1048576
  
  # 最大并发子请求数
  max_concurrent_subrequests: 4
  
  # 最大重试次数
  max_retries: 3
  
  # 启用分片的 URL 模式（正则表达式）
  slice_patterns:
    - "^/downloads/.*"
    - "^/files/.*\\.iso$"
    - "^/videos/.*\\.mp4$"
  
  # 缓存配置
  cache:
    enabled: true
    ttl: 3600  # 1小时
    storage: "file"
    cache_dir: "/var/cache/pingora-slice"
    max_cache_size: 10737418240  # 10GB

# 指标监控
metrics:
  enabled: true
  listen: "0.0.0.0:9091"
```

### 高级配置

```yaml
# 完整配置示例
listen:
  address: "0.0.0.0:8080"
  threads: 4

upstream:
  address: "origin.example.com:80"
  timeout: 30
  keepalive: true
  keepalive_timeout: 60

slice:
  slice_size: 2097152  # 2MB
  max_concurrent_subrequests: 8
  max_retries: 3
  
  slice_patterns:
    - "^/downloads/.*"
    - "^/files/.*\\.(iso|bin|exe|dmg)$"
    - "^/videos/.*\\.(mp4|mkv|avi)$"
  
  cache:
    enabled: true
    ttl: 7200
    storage: "file"
    cache_dir: "/var/cache/pingora-slice"
    max_cache_size: 53687091200  # 50GB
  
  retry:
    backoff_ms: [100, 200, 400, 800]

metrics:
  enabled: true
  listen: "0.0.0.0:9091"

logging:
  level: "info"
  format: "json"
```

## 启动服务

### 使用 systemd（RPM 安装）

```bash
# 启动服务
sudo systemctl start pingora-slice

# 设置开机自启
sudo systemctl enable pingora-slice

# 查看状态
sudo systemctl status pingora-slice

# 查看日志
sudo journalctl -u pingora-slice -f
```

### 直接运行

```bash
# 前台运行
pingora-slice /etc/pingora-slice/pingora_slice.yaml

# 后台运行
nohup pingora-slice /etc/pingora-slice/pingora_slice.yaml > /var/log/pingora-slice.log 2>&1 &
```

## 验证

### 测试基本功能

```bash
# 测试健康检查
curl http://localhost:8080/health

# 测试文件下载
curl -I http://localhost:8080/downloads/large-file.iso

# 测试 Range 请求
curl -H "Range: bytes=0-1023" http://localhost:8080/downloads/large-file.iso
```

### 查看指标

```bash
# 查看 Prometheus 指标
curl http://localhost:9091/metrics

# 查看关键指标
curl http://localhost:9091/metrics | grep pingora_slice
```

### 监控日志

```bash
# 实时查看日志
sudo journalctl -u pingora-slice -f

# 查看错误日志
sudo journalctl -u pingora-slice -p err

# 查看最近 100 条日志
sudo journalctl -u pingora-slice -n 100
```

## 性能测试

### 使用 ab (Apache Bench)

```bash
# 安装 ab
sudo dnf install -y httpd-tools

# 并发测试
ab -n 1000 -c 10 http://localhost:8080/downloads/test-file.bin
```

### 使用 wrk

```bash
# 安装 wrk
sudo dnf install -y wrk

# 压力测试
wrk -t4 -c100 -d30s http://localhost:8080/downloads/test-file.bin
```

### 使用内置压测脚本

```bash
# 运行压测
./scripts/stress_test.sh http://localhost:8080 /downloads/test-file.bin
```

## 常见问题

### 1. 服务无法启动

```bash
# 检查配置文件
sudo cat /etc/pingora-slice/pingora_slice.yaml

# 查看错误日志
sudo journalctl -u pingora-slice -n 50

# 检查端口占用
sudo netstat -tlnp | grep 8080
```

### 2. 缓存不工作

```bash
# 检查缓存目录权限
ls -la /var/cache/pingora-slice

# 修复权限
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chmod 755 /var/cache/pingora-slice
```

### 3. 性能不佳

```bash
# 增加并发数
sudo vi /etc/pingora-slice/pingora_slice.yaml
# 修改 max_concurrent_subrequests: 8

# 增加分片大小
# 修改 slice_size: 2097152  # 2MB

# 重启服务
sudo systemctl restart pingora-slice
```

### 4. 内存占用高

```bash
# 减少缓存大小
sudo vi /etc/pingora-slice/pingora_slice.yaml
# 修改 max_cache_size: 5368709120  # 5GB

# 清理缓存
sudo rm -rf /var/cache/pingora-slice/*

# 重启服务
sudo systemctl restart pingora-slice
```

## 监控集成

### Prometheus

在 Prometheus 配置中添加：

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9091']
```

### Grafana

导入 Pingora Slice 仪表板（待创建）或创建自定义面板监控以下指标：

- `pingora_slice_requests_total` - 总请求数
- `pingora_slice_cache_hits_total` - 缓存命中数
- `pingora_slice_cache_misses_total` - 缓存未命中数
- `pingora_slice_subrequests_total` - 子请求总数
- `pingora_slice_request_duration_seconds` - 请求延迟

## 生产环境建议

### 1. 资源配置

```yaml
# 根据服务器配置调整
listen:
  threads: 8  # CPU 核心数

slice:
  max_concurrent_subrequests: 16  # 根据带宽调整
  slice_size: 2097152  # 2MB，根据文件大小调整
  
  cache:
    max_cache_size: 107374182400  # 100GB，根据磁盘空间调整
```

### 2. 安全配置

```bash
# 配置防火墙
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --reload

# 限制指标端口访问
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="9091" protocol="tcp" accept'
sudo firewall-cmd --reload
```

### 3. 日志轮转

创建 `/etc/logrotate.d/pingora-slice`：

```
/var/log/pingora-slice/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0644 pingora-slice pingora-slice
    postrotate
        systemctl reload pingora-slice > /dev/null 2>&1 || true
    endscript
}
```

### 4. 监控告警

设置 Prometheus 告警规则：

```yaml
groups:
  - name: pingora-slice
    rules:
      - alert: HighErrorRate
        expr: rate(pingora_slice_errors_total[5m]) > 0.1
        for: 5m
        annotations:
          summary: "High error rate detected"
      
      - alert: LowCacheHitRate
        expr: rate(pingora_slice_cache_hits_total[5m]) / rate(pingora_slice_requests_total[5m]) < 0.5
        for: 10m
        annotations:
          summary: "Cache hit rate below 50%"
```

## 下一步

- 阅读[完整文档](README.md)
- 查看[配置说明](docs/CONFIGURATION.md)
- 了解[部署指南](docs/DEPLOYMENT.md)
- 查看[性能调优](docs/PERFORMANCE_TUNING.md)

## 获取帮助

- GitHub Issues: https://github.com/your-username/pingora-slice/issues
- 文档: https://github.com/your-username/pingora-slice/tree/main/docs

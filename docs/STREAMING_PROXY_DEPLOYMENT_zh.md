# Pingora 流式代理 - 生产环境部署指南

## 概述

本指南提供了在生产环境中部署 Pingora 流式代理的全面说明。流式代理提供边缘缓存和实时流式传输功能，支持基于文件和高性能原始磁盘缓存后端。

## 目录

- [架构概述](#架构概述)
- [系统要求](#系统要求)
- [部署前规划](#部署前规划)
- [安装](#安装)
- [配置](#配置)
- [服务管理](#服务管理)
- [监控和可观测性](#监控和可观测性)
- [性能调优](#性能调优)
- [安全](#安全)
- [高可用性](#高可用性)
- [故障排查](#故障排查)
- [维护](#维护)

## 架构概述

### 流式代理架构

```
┌─────────┐         ┌──────────────────┐         ┌──────────┐
│ 客户端  │────────▶│   流式代理       │────────▶│  源站    │
└─────────┘         │                  │         └──────────┘
                    │  ┌────────────┐  │
                    │  │ L1 (内存)  │  │
                    │  └────────────┘  │
                    │  ┌────────────┐  │
                    │  │ L2 (磁盘)  │  │
                    │  │ - 文件     │  │
                    │  │ - 原始磁盘 │  │
                    │  └────────────┘  │
                    └──────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │  Prometheus  │
                    │    指标      │
                    └──────────────┘
```

### 核心特性

- **实时流式传输**：边缘到客户端流式传输，TTFB < 1ms
- **后台缓存**：流式传输期间非阻塞缓存写入
- **分层缓存**：L1（内存）+ L2（文件或原始磁盘）
- **优雅降级**：缓存失败不会停止代理
- **健康检查**：内置健康检查端点
- **指标**：兼容 Prometheus 的指标

## 系统要求

### 最低要求

- **CPU**：2 核心（x86_64 或 ARM64）
- **内存**：2 GB
- **磁盘**：20 GB（10 GB 用于缓存）
- **操作系统**：Linux（Ubuntu 20.04+、CentOS 8+、Debian 11+）
- **网络**：100 Mbps

### 生产环境推荐配置

- **CPU**：4+ 核心（x86_64）
- **内存**：8+ GB
- **磁盘**：100+ GB NVMe SSD（用于原始磁盘缓存）
- **操作系统**：Ubuntu 22.04 LTS 或 Rocky Linux 9
- **网络**：1+ Gbps

### 软件依赖

- **Rust**：1.70+（从源码构建时需要）
- **systemd**：用于服务管理
- **OpenSSL**：1.1.1+（用于 HTTPS 支持）

## 部署前规划

### 1. 容量规划

#### 缓存大小计算

```
L1 缓存大小 = 热数据大小 × 1.2
L2 缓存大小 = (总内容大小 × 缓存命中率目标) × 1.5

示例：
- 热数据：50 MB → L1 = 60 MB
- 总内容：100 GB，80% 命中率 → L2 = 120 GB
```

#### 并发连接数

```
最大连接数 = (可用内存 - 操作系统 - L1 缓存) / 10 MB

示例：
- 8 GB 内存
- 操作系统：2 GB
- L1 缓存：100 MB
- 最大连接数：(8000 - 2000 - 100) / 10 = 590
```

### 2. 后端选择

| 后端 | 使用场景 | 性能 | 复杂度 |
|------|---------|------|--------|
| **文件** | 开发环境、小型部署 | 良好 | 低 |
| **原始磁盘** | 生产环境、高性能 | 优秀 | 中等 |

### 3. 网络规划

- **上游**：确保到源站的稳定、低延迟连接
- **防火墙**：规划端口访问（8080 用于代理，8081 用于健康检查，9090 用于指标）
- **负载均衡器**：考虑使用 HAProxy 或 Nginx 进行多实例部署

## 安装

### 方式 1：二进制安装（推荐）

```bash
# 下载最新版本
VERSION="0.2.3"
wget https://github.com/your-org/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-linux-x86_64.tar.gz

# 解压
tar -xzf pingora-slice-${VERSION}-linux-x86_64.tar.gz

# 安装
sudo cp pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice

# 验证
pingora-slice --version
```

### 方式 2：从源码构建

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 克隆仓库
git clone https://github.com/your-org/pingora-slice.git
cd pingora-slice

# 构建发布版本
cargo build --release

# 安装
sudo cp target/release/pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice
```

### 创建服务用户

```bash
# 创建专用用户
sudo useradd -r -s /bin/false -d /var/lib/pingora-slice pingora-slice

# 创建目录
sudo mkdir -p /etc/pingora-slice
sudo mkdir -p /var/lib/pingora-slice
sudo mkdir -p /var/cache/pingora-slice
sudo mkdir -p /var/log/pingora-slice

# 设置所有权
sudo chown -R pingora-slice:pingora-slice /var/lib/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/log/pingora-slice
```

## 配置

### 基础配置（基于文件的缓存）

创建 `/etc/pingora-slice/config.yaml`：

```yaml
# 缓存配置
enable_cache: true
cache_ttl: 3600  # 1 小时

# L1（内存）缓存
l1_cache_size_bytes: 104857600  # 100 MB

# L2（基于文件）缓存
enable_l2_cache: true
l2_backend: "file"
l2_cache_dir: "/var/cache/pingora-slice"

# 上游服务器
upstream_address: "origin.example.com:80"

# 指标端点
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### 生产配置（原始磁盘缓存）

创建 `/etc/pingora-slice/config.yaml`：

```yaml
# 缓存配置
enable_cache: true
cache_ttl: 3600  # 1 小时

# L1（内存）缓存 - 100 MB 用于热数据
l1_cache_size_bytes: 104857600

# L2（原始磁盘）缓存 - 高性能
enable_l2_cache: true
l2_backend: "raw_disk"

# 原始磁盘缓存配置
raw_disk_cache:
  # 缓存设备/文件路径
  device_path: "/var/cache/pingora-slice-raw"
  
  # 总缓存大小：100 GB
  total_size: 107374182400
  
  # 块大小：4 KB（适用于大多数工作负载）
  block_size: 4096
  
  # 性能优化
  use_direct_io: true        # 绕过操作系统页缓存
  enable_compression: true   # 压缩缓存数据
  enable_prefetch: true      # 预测性预取
  enable_zero_copy: true     # 减少内存拷贝

# 上游服务器
upstream_address: "origin.example.com:80"

# 指标端点
metrics_endpoint:
  enabled: true
  address: "0.0.0.0:9090"
```

### 创建原始磁盘缓存文件

```bash
# 创建 100 GB 缓存文件
sudo fallocate -l 100G /var/cache/pingora-slice-raw

# 设置所有权
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw

# 设置权限
sudo chmod 600 /var/cache/pingora-slice-raw
```

### 配置验证

```bash
# 测试配置
sudo -u pingora-slice /usr/local/bin/pingora-slice --check-config /etc/pingora-slice/config.yaml
```

## 服务管理

### Systemd 服务配置

创建 `/etc/systemd/system/pingora-slice.service`：

```ini
[Unit]
Description=Pingora Slice 流式代理
Documentation=https://github.com/your-org/pingora-slice
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=pingora-slice
Group=pingora-slice
WorkingDirectory=/var/lib/pingora-slice

# 主服务
ExecStart=/usr/local/bin/pingora-slice /etc/pingora-slice/config.yaml

# 优雅重载
ExecReload=/bin/kill -HUP $MAINPID

# 重启策略
Restart=on-failure
RestartSec=5s

# 日志
StandardOutput=journal
StandardError=journal
SyslogIdentifier=pingora-slice

# 安全加固
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/cache/pingora-slice /var/log/pingora-slice /var/lib/pingora-slice

# 资源限制
LimitNOFILE=65536
LimitNPROC=4096

# 环境变量
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

[Install]
WantedBy=multi-user.target
```

### 服务管理命令

```bash
# 重新加载 systemd 配置
sudo systemctl daemon-reload

# 启用服务开机自启
sudo systemctl enable pingora-slice

# 启动服务
sudo systemctl start pingora-slice

# 检查状态
sudo systemctl status pingora-slice

# 查看日志
sudo journalctl -u pingora-slice -f

# 停止服务
sudo systemctl stop pingora-slice

# 重启服务
sudo systemctl restart pingora-slice
```

## 监控和可观测性

### 健康检查端点

流式代理提供内置的健康检查端点：

```bash
# 健康状态
curl http://localhost:8081/health
# 响应：{"status":"healthy"}

# 就绪检查
curl http://localhost:8081/ready

# 存活检查
curl http://localhost:8081/live
```

### Prometheus 指标

配置 Prometheus 抓取指标：

```yaml
# /etc/prometheus/prometheus.yml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### 关键指标

| 指标 | 描述 | 告警阈值 |
|------|------|---------|
| `cache_hit_rate` | 缓存命中率百分比 | < 50% |
| `cache_l1_hits_total` | L1 缓存命中数 | - |
| `cache_l2_hits_total` | L2 缓存命中数 | - |
| `cache_misses_total` | 缓存未命中数 | - |
| `raw_disk_fragmentation_rate` | 磁盘碎片率 | > 0.4 |
| `raw_disk_used_blocks` | 已使用缓存块 | > 90% |

### Grafana 仪表板

导入提供的 Grafana 仪表板或创建面板：

1. **缓存性能**
   - 随时间变化的命中率
   - L1 vs L2 命中分布
   - 未命中率趋势

2. **原始磁盘健康**
   - 碎片率
   - 空间利用率
   - I/O 操作

3. **请求指标**
   - 请求速率
   - 响应时间（TTFB）
   - 错误率

4. **系统资源**
   - CPU 使用率
   - 内存使用率
   - 网络吞吐量

### 告警规则

创建 `/etc/prometheus/rules/pingora-slice.yml`：

```yaml
groups:
  - name: pingora_slice_alerts
    rules:
      - alert: 低缓存命中率
        expr: cache_hit_rate < 50
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "缓存命中率低"
          description: "缓存命中率为 {{ $value }}%（阈值：50%）"

      - alert: 高碎片率
        expr: raw_disk_fragmentation_rate > 0.4
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "磁盘碎片率高"
          description: "碎片率为 {{ $value }}（阈值：0.4）"

      - alert: 缓存几乎满
        expr: (raw_disk_used_blocks / raw_disk_total_blocks) > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "缓存几乎满"
          description: "缓存使用率为 {{ $value | humanizePercentage }}"

      - alert: 服务宕机
        expr: up{job="pingora-slice"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Pingora Slice 服务宕机"
          description: "服务已宕机超过 1 分钟"
```

## 性能调优

### L1 缓存大小

```yaml
# 小型部署（< 1000 req/s）
l1_cache_size_bytes: 52428800  # 50 MB

# 中型部署（1000-10000 req/s）
l1_cache_size_bytes: 104857600  # 100 MB

# 大型部署（> 10000 req/s）
l1_cache_size_bytes: 524288000  # 500 MB
```

### 原始磁盘缓存调优

#### 块大小选择

```yaml
# 小文件（平均 < 100 KB）
block_size: 2048  # 2 KB

# 中等文件（平均 100 KB - 10 MB）
block_size: 4096  # 4 KB（默认）

# 大文件（平均 > 10 MB）
block_size: 8192  # 8 KB
```

#### O_DIRECT 配置

```yaml
# 为专用缓存存储启用
use_direct_io: true

# 为共享存储或 NFS 禁用
use_direct_io: false
```

#### 压缩设置

```yaml
# 为文本内容启用（HTML、CSS、JS、JSON）
enable_compression: true

# 为已压缩内容禁用（JPEG、PNG、MP4、ZIP）
enable_compression: false
```

### 系统调优

#### 内核参数

添加到 `/etc/sysctl.conf`：

```bash
# 网络调优
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 8192
net.ipv4.tcp_tw_reuse = 1
net.ipv4.ip_local_port_range = 1024 65535

# 文件描述符限制
fs.file-max = 2097152

# 内存管理
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5

# 应用更改
sudo sysctl -p
```

#### 文件描述符限制

添加到 `/etc/security/limits.conf`：

```
pingora-slice soft nofile 65536
pingora-slice hard nofile 65536
pingora-slice soft nproc 4096
pingora-slice hard nproc 4096
```

## 安全

### 防火墙配置

```bash
# UFW（Ubuntu/Debian）
sudo ufw allow 8080/tcp comment 'Pingora Slice 代理'
sudo ufw allow from 10.0.0.0/8 to any port 8081 comment '健康检查（内部）'
sudo ufw allow from 10.0.0.0/8 to any port 9090 comment '指标（内部）'
sudo ufw enable

# firewalld（CentOS/RHEL）
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="8081" protocol="tcp" accept'
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="9090" protocol="tcp" accept'
sudo firewall-cmd --reload
```

### TLS/SSL 配置

使用反向代理（Nginx 或 HAProxy）进行 TLS 终止：

```nginx
# /etc/nginx/sites-available/pingora-slice
upstream pingora_backend {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name cdn.example.com;

    ssl_certificate /etc/ssl/certs/cdn.example.com.crt;
    ssl_certificate_key /etc/ssl/private/cdn.example.com.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://pingora_backend;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # 禁用缓冲以支持流式传输
        proxy_buffering off;
        proxy_request_buffering off;
    }
}
```

### 文件权限

```bash
# 配置文件
sudo chmod 640 /etc/pingora-slice/config.yaml
sudo chown root:pingora-slice /etc/pingora-slice/config.yaml

# 缓存文件
sudo chmod 700 /var/cache/pingora-slice
sudo chmod 600 /var/cache/pingora-slice-raw

# 日志文件
sudo chmod 755 /var/log/pingora-slice
```

## 高可用性

### 多实例部署

#### 实例配置

在不同端口上运行多个实例：

```yaml
# 实例 1：/etc/pingora-slice/config-1.yaml
# （使用端口 8080）

# 实例 2：/etc/pingora-slice/config-2.yaml
# （使用端口 8081）

# 实例 3：/etc/pingora-slice/config-3.yaml
# （使用端口 8082）
```

#### 负载均衡器配置（HAProxy）

```
# /etc/haproxy/haproxy.cfg
frontend http_front
    bind *:80
    default_backend pingora_backend

backend pingora_backend
    balance roundrobin
    option httpchk GET /health
    http-check expect status 200
    server slice1 127.0.0.1:8080 check inter 5s fall 3 rise 2
    server slice2 127.0.0.1:8081 check inter 5s fall 3 rise 2
    server slice3 127.0.0.1:8082 check inter 5s fall 3 rise 2
```

### Kubernetes 部署

#### Deployment 清单

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: pingora-slice
  labels:
    app: pingora-slice
spec:
  replicas: 3
  selector:
    matchLabels:
      app: pingora-slice
  template:
    metadata:
      labels:
        app: pingora-slice
    spec:
      containers:
      - name: pingora-slice
        image: your-registry/pingora-slice:0.2.3
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 8081
          name: health
        - containerPort: 9090
          name: metrics
        env:
        - name: RUST_LOG
          value: "info"
        livenessProbe:
          httpGet:
            path: /live
            port: 8081
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 8081
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 2
        resources:
          requests:
            memory: "2Gi"
            cpu: "1000m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
        volumeMounts:
        - name: config
          mountPath: /etc/pingora-slice
          readOnly: true
        - name: cache
          mountPath: /var/cache/pingora-slice
      volumes:
      - name: config
        configMap:
          name: pingora-slice-config
      - name: cache
        emptyDir:
          sizeLimit: 10Gi
```

#### Service 清单

```yaml
apiVersion: v1
kind: Service
metadata:
  name: pingora-slice
  labels:
    app: pingora-slice
spec:
  type: ClusterIP
  selector:
    app: pingora-slice
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: metrics
    port: 9090
    targetPort: 9090
```

#### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: pingora-slice-config
data:
  config.yaml: |
    enable_cache: true
    cache_ttl: 3600
    l1_cache_size_bytes: 104857600
    enable_l2_cache: true
    l2_backend: "file"
    l2_cache_dir: "/var/cache/pingora-slice"
    upstream_address: "origin.example.com:80"
    metrics_endpoint:
      enabled: true
      address: "0.0.0.0:9090"
```

## 故障排查

### 服务无法启动

```bash
# 检查服务状态
sudo systemctl status pingora-slice

# 查看详细日志
sudo journalctl -u pingora-slice -n 100 --no-pager

# 测试配置
sudo -u pingora-slice /usr/local/bin/pingora-slice --check-config /etc/pingora-slice/config.yaml

# 检查文件权限
ls -la /etc/pingora-slice/
ls -la /var/cache/pingora-slice/

# 检查端口可用性
sudo ss -tlnp | grep -E '8080|8081|9090'
```

### 缓存不工作

```bash
# 检查缓存统计
curl http://localhost:9090/metrics | grep cache

# 验证缓存目录
ls -la /var/cache/pingora-slice/

# 检查磁盘空间
df -h /var/cache

# 查看缓存错误日志
sudo journalctl -u pingora-slice | grep -i cache
```

### 内存使用率高

```bash
# 监控内存使用
watch -n 1 'ps aux | grep pingora-slice'

# 检查 L1 缓存大小
grep l1_cache_size_bytes /etc/pingora-slice/config.yaml

# 如需要，减少 L1 缓存大小
sudo vi /etc/pingora-slice/config.yaml
# 设置：l1_cache_size_bytes: 52428800  # 50 MB

# 重启服务
sudo systemctl restart pingora-slice
```

### 性能问题

```bash
# 检查系统资源
top
htop
iostat -x 1

# 检查网络连接
ss -s
netstat -an | grep 8080 | wc -l

# 查看指标
curl http://localhost:9090/metrics

# 临时启用调试日志
sudo systemctl stop pingora-slice
sudo -u pingora-slice RUST_LOG=debug /usr/local/bin/pingora-slice /etc/pingora-slice/config.yaml
```

### 原始磁盘缓存问题

```bash
# 检查原始磁盘缓存文件
ls -lh /var/cache/pingora-slice-raw

# 检查碎片率
curl http://localhost:9090/metrics | grep fragmentation

# 检查磁盘 I/O
iostat -x 1 10

# 如果损坏，重新创建缓存文件
sudo systemctl stop pingora-slice
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 100G /var/cache/pingora-slice-raw
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice
```

## 维护

### 更新服务

```bash
# 1. 下载新版本
wget https://github.com/your-org/pingora-slice/releases/download/v0.2.4/pingora-slice-0.2.4-linux-x86_64.tar.gz

# 2. 解压
tar -xzf pingora-slice-0.2.4-linux-x86_64.tar.gz

# 3. 备份当前二进制文件
sudo cp /usr/local/bin/pingora-slice /usr/local/bin/pingora-slice.backup

# 4. 停止服务
sudo systemctl stop pingora-slice

# 5. 安装新二进制文件
sudo cp pingora-slice /usr/local/bin/
sudo chmod +x /usr/local/bin/pingora-slice

# 6. 启动服务
sudo systemctl start pingora-slice

# 7. 验证
sudo systemctl status pingora-slice
curl http://localhost:8081/health

# 8. 如需要，回滚
# sudo cp /usr/local/bin/pingora-slice.backup /usr/local/bin/pingora-slice
# sudo systemctl restart pingora-slice
```

### 缓存维护

```bash
# 查看缓存统计
curl http://localhost:9090/metrics | grep -E '(cache|raw_disk)'

# 清理缓存（如需要）
sudo systemctl stop pingora-slice
sudo rm -rf /var/cache/pingora-slice/*
sudo rm /var/cache/pingora-slice-raw
sudo fallocate -l 100G /var/cache/pingora-slice-raw
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice-raw
sudo systemctl start pingora-slice

# 监控缓存增长
watch -n 5 'du -sh /var/cache/pingora-slice*'
```

### 日志管理

```bash
# 查看最近日志
sudo journalctl -u pingora-slice -n 100

# 查看特定时间以来的日志
sudo journalctl -u pingora-slice --since "1 hour ago"

# 实时跟踪日志
sudo journalctl -u pingora-slice -f

# 导出日志
sudo journalctl -u pingora-slice --since "2024-01-01" > pingora-slice.log

# 清理旧日志
sudo journalctl --vacuum-time=7d
sudo journalctl --vacuum-size=1G
```

### 备份和恢复

```bash
# 备份配置
sudo tar -czf /backup/pingora-slice-config-$(date +%Y%m%d).tar.gz \
    /etc/pingora-slice/ \
    /etc/systemd/system/pingora-slice.service

# 备份缓存（可选，较大）
sudo tar -czf /backup/pingora-slice-cache-$(date +%Y%m%d).tar.gz \
    /var/cache/pingora-slice/

# 恢复配置
sudo tar -xzf /backup/pingora-slice-config-20240101.tar.gz -C /

# 恢复缓存
sudo tar -xzf /backup/pingora-slice-cache-20240101.tar.gz -C /
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice/
```

## 最佳实践

### 1. 容量规划

- 监控缓存命中率（目标：>80%）
- 在原始磁盘缓存中规划 20% 的空闲空间
- 为热数据调整 L1 缓存大小（通常为 50-500 MB）
- 根据内容大小和命中率目标调整 L2 缓存大小

### 2. 性能优化

- 生产部署使用原始磁盘缓存
- 为专用缓存存储启用 O_DIRECT
- 为文本内容启用压缩
- 根据平均文件大小调整块大小
- 监控并处理碎片（目标：<20%）

### 3. 可靠性

- 在负载均衡器后部署多个实例
- 在负载均衡器中配置健康检查
- 设置监控和告警
- 实施自动故障转移
- 定期备份配置

### 4. 安全

- 以专用用户身份运行服务
- 将指标端点限制在内部网络
- 在反向代理处使用 TLS 终止
- 保持软件更新
- 定期安全审计

### 5. 监控

- 跟踪缓存命中率
- 监控碎片率
- 服务宕机时告警
- 监控资源使用（CPU、内存、磁盘）
- 跟踪错误率

## 快速参考

### 服务命令

```bash
# 启动/停止/重启
sudo systemctl start|stop|restart pingora-slice

# 查看状态
sudo systemctl status pingora-slice

# 查看日志
sudo journalctl -u pingora-slice -f

# 重新加载配置
sudo systemctl reload pingora-slice
```

### 健康检查

```bash
# 健康状态
curl http://localhost:8081/health

# 就绪检查
curl http://localhost:8081/ready

# 存活检查
curl http://localhost:8081/live
```

### 指标

```bash
# 所有指标
curl http://localhost:9090/metrics

# 缓存指标
curl http://localhost:9090/metrics | grep cache

# 原始磁盘指标
curl http://localhost:9090/metrics | grep raw_disk
```

## 支持和资源

### 文档

- [流式代理概述](STREAMING_PROXY.md)
- [配置指南](STREAMING_PROXY_CONFIG.md)
- [性能分析](STREAMING_PROXY_PERFORMANCE.md)
- [错误处理](STREAMING_PROXY_ERROR_HANDLING.md)
- [快速入门指南](STREAMING_PROXY_QUICK_START.md)

### 获取帮助

- **GitHub Issues**：https://github.com/your-org/pingora-slice/issues
- **文档**：https://github.com/your-org/pingora-slice/tree/main/docs
- **日志**：`sudo journalctl -u pingora-slice -f`
- **指标**：`curl http://localhost:9090/metrics`

## 附录

### 配置模板

查看 [examples/pingora_slice_raw_disk_full.yaml](../examples/pingora_slice_raw_disk_full.yaml) 获取包含所有可用选项的完整配置模板。

### 示例部署

- **开发环境**：基于文件的缓存，1 GB，单实例
- **小型生产环境**：原始磁盘缓存，10 GB，2 个实例
- **大型生产环境**：原始磁盘缓存，100 GB，3+ 个实例，负载均衡

### 性能基准

查看 [STREAMING_PROXY_PERFORMANCE.md](STREAMING_PROXY_PERFORMANCE.md) 获取详细的性能分析和基准测试。

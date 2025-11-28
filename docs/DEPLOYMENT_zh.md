# Pingora Slice 模块 - 部署指南

[English](DEPLOYMENT.md) | [中文](DEPLOYMENT_zh.md)

本指南提供在生产环境中部署 Pingora Slice 模块的全面说明。

## 目录

- [系统要求](#系统要求)
- [部署前检查清单](#部署前检查清单)
- [安装](#安装)
- [配置](#配置)
- [服务管理](#服务管理)
- [反向代理设置](#反向代理设置)
- [安全加固](#安全加固)
- [监控和日志](#监控和日志)
- [备份和恢复](#备份和恢复)
- [扩展和高可用性](#扩展和高可用性)
- [维护](#维护)
- [故障排除](#故障排除)

## 系统要求

### 最低要求

- **CPU**：2 核
- **内存**：2 GB
- **磁盘**：10 GB（加上缓存存储）
- **操作系统**：Linux（Ubuntu 20.04+、CentOS 8+、Debian 11+）
- **网络**：100 Mbps

### 推荐要求

- **CPU**：4+ 核
- **内存**：8+ GB
- **磁盘**：50+ GB SSD（用于缓存）
- **操作系统**：Linux（推荐 Ubuntu 22.04 LTS）
- **网络**：1 Gbps+

### 软件依赖

- **Rust**：1.70 或更高版本
- **Cargo**：随 Rust 一起提供
- **systemd**：用于服务管理
- **OpenSSL**：用于 HTTPS 支持（可选）

## 部署前检查清单

部署前，确保你已：

- [ ] 编译了发布版二进制文件（`cargo build --release`）
- [ ] 测试了配置文件
- [ ] 确定了上游源站服务器地址
- [ ] 确定了分片大小和并发设置
- [ ] 规划了缓存存储位置和大小
- [ ] 配置了防火墙规则
- [ ] 设置了监控基础设施
- [ ] 准备了备份策略
- [ ] 记录了回滚程序

## 安装

### 步骤 1：构建发布版二进制文件

```bash
# 在构建服务器或本地
git clone <repository-url>
cd pingora-slice

# 构建优化的发布版二进制文件
cargo build --release

# 验证二进制文件
./target/release/pingora-slice --version

# 运行测试以确保正确性
cargo test --release
```

### 步骤 2：创建部署用户

```bash
# 创建专用用户来运行服务
sudo useradd -r -s /bin/false -d /opt/pingora-slice pingora-slice

# 创建主目录
sudo mkdir -p /opt/pingora-slice
sudo chown pingora-slice:pingora-slice /opt/pingora-slice
```

### 步骤 3：安装二进制文件和配置

```bash
# 复制二进制文件到部署位置
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora-slice
sudo chmod 755 /opt/pingora-slice/pingora-slice

# 复制配置文件
sudo cp pingora_slice.yaml /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora_slice.yaml
sudo chmod 644 /opt/pingora-slice/pingora_slice.yaml

# 创建缓存目录
sudo mkdir -p /var/cache/pingora-slice
sudo chown pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chmod 755 /var/cache/pingora-slice

# 创建日志目录
sudo mkdir -p /var/log/pingora-slice
sudo chown pingora-slice:pingora-slice /var/log/pingora-slice
sudo chmod 755 /var/log/pingora-slice
```

### 步骤 4：验证安装

```bash
# 测试配置
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# 测试二进制文件执行（应该启动并可被终止）
sudo -u pingora-slice /opt/pingora-slice/pingora-slice &
sleep 2
sudo pkill pingora-slice
```

## 配置

### 生产配置模板

创建 `/opt/pingora-slice/pingora_slice.yaml`：

```yaml
# Pingora Slice 模块的生产配置

# 分片大小：1MB 以获得平衡性能
slice_size: 1048576

# 并发：4 个并发子请求
max_concurrent_subrequests: 4

# 重试：3 次尝试，采用指数退避
max_retries: 3

# URL 模式：根据你的内容调整
slice_patterns:
  - "^/downloads/.*"
  - "^/files/.*\\.(bin|iso|zip|tar\\.gz)$"

# 缓存：启用，TTL 为 1 小时
enable_cache: true
cache_ttl: 3600

# 上游：你的源站服务器
upstream_address: "origin.example.com:80"

# 指标：仅在本地主机上启用
metrics_endpoint:
  enabled: true
  address: "127.0.0.1:9090"
```

### 环境特定配置

#### 开发环境
```yaml
slice_size: 524288  # 512KB 用于更快测试
max_concurrent_subrequests: 2
max_retries: 1
cache_ttl: 300  # 5 分钟
```

#### 预发布环境
```yaml
slice_size: 1048576  # 1MB
max_concurrent_subrequests: 4
max_retries: 3
cache_ttl: 1800  # 30 分钟
```

#### 生产环境
```yaml
slice_size: 2097152  # 2MB 用于高性能
max_concurrent_subrequests: 8
max_retries: 3
cache_ttl: 7200  # 2 小时
```

## 服务管理

### Systemd 服务配置

创建 `/etc/systemd/system/pingora-slice.service`：

```ini
[Unit]
Description=Pingora Slice Module
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=pingora-slice
Group=pingora-slice
WorkingDirectory=/opt/pingora-slice
ExecStart=/opt/pingora-slice/pingora-slice /opt/pingora-slice/pingora_slice.yaml
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5s
StandardOutput=journal
StandardError=journal
SyslogIdentifier=pingora-slice

# 安全设置
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/cache/pingora-slice /var/log/pingora-slice

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

# 启用服务以在启动时自动启动
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

# 重新加载配置（如果支持）
sudo systemctl reload pingora-slice
```

## 反向代理设置

### Nginx 作为反向代理

创建 `/etc/nginx/sites-available/pingora-slice`：

```nginx
upstream pingora_slice {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 80;
    server_name cdn.example.com;

    # 重定向到 HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name cdn.example.com;

    # SSL 配置
    ssl_certificate /etc/ssl/certs/cdn.example.com.crt;
    ssl_certificate_key /etc/ssl/private/cdn.example.com.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # 日志
    access_log /var/log/nginx/pingora-slice-access.log;
    error_log /var/log/nginx/pingora-slice-error.log;

    # 代理设置
    location / {
        proxy_pass http://pingora_slice;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # 超时
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 300s;

        # 缓冲
        proxy_buffering off;
        proxy_request_buffering off;
    }

    # 指标端点（限制访问）
    location /metrics {
        proxy_pass http://127.0.0.1:9090/metrics;
        allow 10.0.0.0/8;  # 仅内部网络
        deny all;
    }

    # 健康检查
    location /health {
        proxy_pass http://127.0.0.1:9090/health;
        access_log off;
    }
}
```

启用站点：

```bash
sudo ln -s /etc/nginx/sites-available/pingora-slice /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### HAProxy 作为负载均衡器

创建 `/etc/haproxy/haproxy.cfg`：

```
global
    log /dev/log local0
    log /dev/log local1 notice
    chroot /var/lib/haproxy
    stats socket /run/haproxy/admin.sock mode 660 level admin
    stats timeout 30s
    user haproxy
    group haproxy
    daemon

defaults
    log     global
    mode    http
    option  httplog
    option  dontlognull
    timeout connect 5000
    timeout client  300000
    timeout server  300000

frontend http_front
    bind *:80
    default_backend pingora_slice_backend

backend pingora_slice_backend
    balance roundrobin
    option httpchk GET /health
    http-check expect status 200
    server slice1 127.0.0.1:8080 check
    server slice2 127.0.0.1:8081 check
    server slice3 127.0.0.1:8082 check
```

## 安全加固

### 防火墙配置

```bash
# UFW（Ubuntu）
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow from 10.0.0.0/8 to any port 9090  # 指标（仅内部）
sudo ufw enable

# firewalld（CentOS/RHEL）
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port port="9090" protocol="tcp" accept'
sudo firewall-cmd --reload
```

### 文件权限

```bash
# 确保正确的所有权
sudo chown -R pingora-slice:pingora-slice /opt/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/log/pingora-slice

# 限制权限
sudo chmod 755 /opt/pingora-slice
sudo chmod 755 /opt/pingora-slice/pingora-slice
sudo chmod 644 /opt/pingora-slice/pingora_slice.yaml
sudo chmod 700 /var/cache/pingora-slice
sudo chmod 755 /var/log/pingora-slice
```

### SELinux 配置（CentOS/RHEL）

```bash
# 设置 SELinux 上下文
sudo semanage fcontext -a -t bin_t "/opt/pingora-slice/pingora-slice"
sudo restorecon -v /opt/pingora-slice/pingora-slice

# 允许网络连接
sudo setsebool -P httpd_can_network_connect 1
```

### 速率限制

在 Nginx 中配置速率限制：

```nginx
# 在 http 块中
limit_req_zone $binary_remote_addr zone=slice_limit:10m rate=10r/s;

# 在 location 块中
location / {
    limit_req zone=slice_limit burst=20 nodelay;
    proxy_pass http://pingora_slice;
}
```

## 监控和日志

### Prometheus 配置

添加到 `/etc/prometheus/prometheus.yml`：

```yaml
scrape_configs:
  - job_name: 'pingora-slice'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Grafana 仪表板

导入提供的 Grafana 仪表板或为以下内容创建面板：

1. **请求速率**
   - 查询：`rate(pingora_slice_requests_total[5m])`
   - 类型：图表

2. **缓存命中率**
   - 查询：`pingora_slice_cache_hit_rate`
   - 类型：仪表

3. **子请求失败率**
   - 查询：`pingora_slice_subrequest_failure_rate`
   - 类型：仪表

4. **带宽使用**
   - 查询：`rate(pingora_slice_bytes_to_client_total[5m])`
   - 类型：图表

5. **延迟**
   - 查询：`pingora_slice_request_duration_ms_avg`
   - 类型：图表

### 日志轮转

创建 `/etc/logrotate.d/pingora-slice`：

```
/var/log/pingora-slice/*.log {
    daily
    rotate 14
    compress
    delaycompress
    notifempty
    create 0644 pingora-slice pingora-slice
    sharedscripts
    postrotate
        systemctl reload pingora-slice > /dev/null 2>&1 || true
    endscript
}
```

### 告警规则

创建 Prometheus 告警规则：

```yaml
groups:
  - name: pingora_slice
    rules:
      - alert: HighCacheMissRate
        expr: pingora_slice_cache_hit_rate < 50
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "高缓存未命中率"
          description: "缓存命中率为 {{ $value }}%"

      - alert: HighSubrequestFailureRate
        expr: pingora_slice_subrequest_failure_rate > 5
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "高子请求失败率"
          description: "失败率为 {{ $value }}%"

      - alert: ServiceDown
        expr: up{job="pingora-slice"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Pingora Slice 服务已停止"
```

## 备份和恢复

### 配置备份

```bash
# 创建备份脚本
cat > /opt/pingora-slice/backup.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/var/backups/pingora-slice"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR
tar -czf $BACKUP_DIR/config_$DATE.tar.gz \
    /opt/pingora-slice/pingora_slice.yaml \
    /etc/systemd/system/pingora-slice.service

# 仅保留最近 30 天
find $BACKUP_DIR -name "config_*.tar.gz" -mtime +30 -delete
EOF

chmod +x /opt/pingora-slice/backup.sh

# 添加到 crontab
echo "0 2 * * * /opt/pingora-slice/backup.sh" | sudo crontab -u root -
```

### 缓存备份（可选）

```bash
# 备份缓存目录
sudo tar -czf /var/backups/pingora-slice/cache_$(date +%Y%m%d).tar.gz \
    /var/cache/pingora-slice/

# 恢复缓存
sudo tar -xzf /var/backups/pingora-slice/cache_20240101.tar.gz -C /
```

### 灾难恢复

1. **备份关键文件：**
   - 配置：`/opt/pingora-slice/pingora_slice.yaml`
   - 服务文件：`/etc/systemd/system/pingora-slice.service`
   - 二进制文件：`/opt/pingora-slice/pingora-slice`

2. **恢复程序：**
   ```bash
   # 从备份恢复
   sudo tar -xzf config_backup.tar.gz -C /
   
   # 重新加载 systemd
   sudo systemctl daemon-reload
   
   # 启动服务
   sudo systemctl start pingora-slice
   
   # 验证
   sudo systemctl status pingora-slice
   curl http://localhost:9090/health
   ```

## 扩展和高可用性

### 水平扩展

在负载均衡器后运行多个实例：

```bash
# 实例 1
/opt/pingora-slice/pingora-slice --port 8080

# 实例 2
/opt/pingora-slice/pingora-slice --port 8081

# 实例 3
/opt/pingora-slice/pingora-slice --port 8082
```

配置负载均衡器（HAProxy、Nginx 等）以分发流量。

### 共享缓存

对于多个实例，使用共享缓存后端：
- Redis
- Memcached
- 分布式文件系统（NFS、GlusterFS）

### 健康检查

在负载均衡器中配置健康检查：

```
# HAProxy
option httpchk GET /health
http-check expect status 200

# Nginx
upstream pingora_slice {
    server 127.0.0.1:8080 max_fails=3 fail_timeout=30s;
    server 127.0.0.1:8081 max_fails=3 fail_timeout=30s;
}
```

## 维护

### 更新服务

```bash
# 1. 构建新版本
cargo build --release

# 2. 停止服务
sudo systemctl stop pingora-slice

# 3. 备份当前二进制文件
sudo cp /opt/pingora-slice/pingora-slice /opt/pingora-slice/pingora-slice.backup

# 4. 部署新二进制文件
sudo cp target/release/pingora-slice /opt/pingora-slice/
sudo chown pingora-slice:pingora-slice /opt/pingora-slice/pingora-slice

# 5. 启动服务
sudo systemctl start pingora-slice

# 6. 验证
sudo systemctl status pingora-slice
curl http://localhost:9090/health

# 7. 如有问题，回滚
# sudo cp /opt/pingora-slice/pingora-slice.backup /opt/pingora-slice/pingora-slice
# sudo systemctl restart pingora-slice
```

### 缓存维护

```bash
# 清除缓存
sudo rm -rf /var/cache/pingora-slice/*

# 检查缓存大小
du -sh /var/cache/pingora-slice/

# 在 systemd 服务中设置缓存大小限制
# 添加到 [Service] 部分：
# ReadWritePaths=/var/cache/pingora-slice
# LimitFSIZE=10G
```

### 日志维护

```bash
# 查看最近的日志
sudo journalctl -u pingora-slice -n 100

# 查看特定时间以来的日志
sudo journalctl -u pingora-slice --since "1 hour ago"

# 实时跟踪日志
sudo journalctl -u pingora-slice -f

# 清除旧的日志
sudo journalctl --vacuum-time=7d
```

## 故障排除

### 服务无法启动

```bash
# 检查服务状态
sudo systemctl status pingora-slice

# 查看详细日志
sudo journalctl -u pingora-slice -n 50 --no-pager

# 测试配置
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# 检查文件权限
ls -la /opt/pingora-slice/
ls -la /var/cache/pingora-slice/

# 检查端口可用性
sudo lsof -i :8080
sudo lsof -i :9090
```

### 高内存使用

```bash
# 监控内存使用
watch -n 1 'ps aux | grep pingora-slice'

# 检查内存泄漏
valgrind --leak-check=full /opt/pingora-slice/pingora-slice

# 重启服务以清除内存
sudo systemctl restart pingora-slice
```

### 性能问题

```bash
# 检查系统资源
top
htop
iostat -x 1

# 检查网络连接
netstat -an | grep 8080
ss -s

# 查看指标
curl http://localhost:9090/metrics | grep -E "(cache_hit_rate|failure_rate|duration)"

# 临时启用调试日志
sudo systemctl stop pingora-slice
sudo -u pingora-slice RUST_LOG=debug /opt/pingora-slice/pingora-slice
```

### 连接问题

```bash
# 测试到源站的连接
curl -I http://origin.example.com/test-file

# 测试代理
curl -v http://localhost:8080/test-file

# 检查防火墙
sudo iptables -L -n
sudo ufw status verbose

# 检查 DNS 解析
nslookup origin.example.com
dig origin.example.com
```

## 支持

如需额外支持：
- 查看日志：`sudo journalctl -u pingora-slice -f`
- 检查指标：`curl http://localhost:9090/metrics`
- 查阅文档：参见 README_zh.md 和 docs/
- 报告问题：在 GitHub issues 中提供日志和配置

## 附录

### 快速参考命令

```bash
# 服务管理
sudo systemctl start|stop|restart|status pingora-slice

# 查看日志
sudo journalctl -u pingora-slice -f

# 检查指标
curl http://localhost:9090/metrics

# 测试配置
sudo -u pingora-slice /opt/pingora-slice/pingora-slice --check-config

# 检查健康状态
curl http://localhost:9090/health
```

### 配置检查清单

- [ ] 分片大小适合文件大小
- [ ] 并发限制根据源站容量设置
- [ ] 缓存 TTL 根据内容更新频率配置
- [ ] URL 模式匹配目标内容
- [ ] 上游地址正确且可达
- [ ] 指标端点已启用且可访问
- [ ] 防火墙规则已配置
- [ ] 服务已启用以在启动时自动启动
- [ ] 监控和告警已配置
- [ ] 备份策略已就位

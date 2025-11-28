# Pingora Slice 完整设置指南

本指南将帮助你完成从开发到生产部署的完整流程。

## 目录

1. [开发环境设置](#开发环境设置)
2. [GitHub 仓库配置](#github-仓库配置)
3. [CI/CD 配置](#cicd-配置)
4. [RPM 包构建和发布](#rpm-包构建和发布)
5. [生产环境部署](#生产环境部署)
6. [监控配置](#监控配置)

## 开发环境设置

### 1. 安装必需工具

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装开发工具
cargo install cargo-watch cargo-tarpaulin cargo-audit
```

### 2. 克隆项目

```bash
git clone https://github.com/your-username/pingora-slice.git
cd pingora-slice
```

### 3. 构建和测试

```bash
# 构建
make build

# 运行测试
make test

# 代码检查
make check

# 运行服务
make run
```

## GitHub 仓库配置

### 1. 创建 GitHub 仓库

1. 访问 https://github.com/new
2. 创建新仓库 `pingora-slice`
3. 不要初始化 README（我们已经有了）

### 2. 推送代码

```bash
# 初始化 git（如果还没有）
git init

# 添加远程仓库
git remote add origin https://github.com/your-username/pingora-slice.git

# 添加所有文件
git add .

# 提交
git commit -m "Initial commit: Pingora Slice v0.1.0"

# 推送到 main 分支
git branch -M main
git push -u origin main
```

### 3. 更新配置文件中的仓库地址

需要更新以下文件中的 `your-username/pingora-slice`：

- `.github/workflows/release.yml`
- `packaging/install.sh`
- `packaging/pingora-slice.service`
- `packaging/README.md`
- `QUICKSTART.md`
- `CONTRIBUTING.md`

使用以下命令批量替换：

```bash
# 替换为你的 GitHub 用户名
YOUR_USERNAME="your-actual-username"

# 在所有相关文件中替换
find . -type f \( -name "*.yml" -o -name "*.yaml" -o -name "*.sh" -o -name "*.md" -o -name "*.service" \) \
  -exec sed -i "s/your-username\/pingora-slice/${YOUR_USERNAME}\/pingora-slice/g" {} +

# 提交更改
git add .
git commit -m "Update repository URLs"
git push
```

## CI/CD 配置

### 1. GitHub Actions 已配置

项目已包含两个 GitHub Actions workflow：

- `.github/workflows/ci.yml` - 持续集成（每次 push 和 PR 时运行）
- `.github/workflows/release.yml` - 发布流程（创建 tag 时触发）

### 2. 验证 CI

推送代码后，访问：
```
https://github.com/your-username/pingora-slice/actions
```

查看 CI 运行状态。

## RPM 包构建和发布

### 方法 1：通过 GitHub Actions（推荐）

#### 自动发布（通过 Git Tag）

```bash
# 更新版本号
vi Cargo.toml  # 修改 version = "0.1.0"

# 更新 CHANGELOG
vi CHANGELOG.md

# 提交更改
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.1.0"
git push

# 创建并推送 tag
git tag v0.1.0
git push origin v0.1.0
```

#### 手动触发

1. 访问 GitHub Actions 页面
2. 选择 "Build and Release RPM" workflow
3. 点击 "Run workflow"
4. 输入版本号（例如：0.1.0）
5. 点击 "Run workflow"

### 方法 2：本地构建

```bash
# 使用 Makefile
make rpm

# 或手动构建
cargo build --release
rpmdev-setuptree
# ... 按照 packaging/README.md 中的步骤操作
```

### 发布后

1. 访问 Releases 页面：
   ```
   https://github.com/your-username/pingora-slice/releases
   ```

2. 验证 RPM 文件已上传：
   - `pingora-slice-0.1.0-1.el8.x86_64.rpm`
   - `pingora-slice-0.1.0-1.el9.x86_64.rpm`

## 生产环境部署

### CentOS 8 / Rocky Linux 8 / AlmaLinux 8

```bash
# 下载并安装
VERSION=0.1.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el8.x86_64.rpm
sudo dnf install -y ./pingora-slice-${VERSION}-1.el8.x86_64.rpm

# 配置
sudo vi /etc/pingora-slice/pingora_slice.yaml

# 启动
sudo systemctl start pingora-slice
sudo systemctl enable pingora-slice

# 验证
sudo systemctl status pingora-slice
curl http://localhost:9091/health
```

### CentOS 9 / Rocky Linux 9 / AlmaLinux 9

```bash
# 下载并安装
VERSION=0.1.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el9.x86_64.rpm
sudo dnf install -y ./pingora-slice-${VERSION}-1.el9.x86_64.rpm

# 配置
sudo vi /etc/pingora-slice/pingora_slice.yaml

# 启动
sudo systemctl start pingora-slice
sudo systemctl enable pingora-slice

# 验证
sudo systemctl status pingora-slice
curl http://localhost:9091/health
```

### Docker 部署

```bash
# 使用 Docker Compose
docker-compose up -d

# 或使用 Docker
docker build -t pingora-slice:latest .
docker run -d -p 8080:8080 -p 9091:9091 pingora-slice:latest

# 验证
curl http://localhost:9091/health
```

## 监控配置

### 1. Prometheus

#### 使用 Docker Compose（推荐）

```bash
# 启动完整监控栈
docker-compose up -d

# 访问 Prometheus
open http://localhost:9090
```

#### 手动配置

```bash
# 安装 Prometheus
sudo dnf install -y prometheus

# 配置
sudo cp monitoring/prometheus.yml /etc/prometheus/
sudo cp monitoring/alerts.yml /etc/prometheus/

# 启动
sudo systemctl start prometheus
sudo systemctl enable prometheus
```

### 2. Grafana

#### 使用 Docker Compose

```bash
# 已包含在 docker-compose.yml 中
docker-compose up -d grafana

# 访问 Grafana
open http://localhost:3000
# 默认用户名/密码: admin/admin
```

#### 手动安装

```bash
# 安装 Grafana
sudo dnf install -y grafana

# 启动
sudo systemctl start grafana-server
sudo systemctl enable grafana-server

# 访问
open http://localhost:3000
```

#### 配置数据源

1. 登录 Grafana
2. 添加 Prometheus 数据源
   - URL: `http://prometheus:9090` (Docker) 或 `http://localhost:9090`
3. 导入仪表板（待创建）

### 3. 告警配置

编辑 `monitoring/alerts.yml` 配置告警规则，然后：

```bash
# 重新加载 Prometheus 配置
curl -X POST http://localhost:9090/-/reload

# 或重启服务
sudo systemctl restart prometheus
```

## 生产环境检查清单

### 部署前

- [ ] 更新配置文件中的上游服务器地址
- [ ] 设置合适的缓存大小和 TTL
- [ ] 配置 URL 匹配模式
- [ ] 调整并发限制和重试策略
- [ ] 配置防火墙规则
- [ ] 设置日志轮转
- [ ] 配置监控和告警

### 部署后

- [ ] 验证服务正常运行
- [ ] 测试基本功能
- [ ] 检查日志输出
- [ ] 验证指标收集
- [ ] 测试缓存功能
- [ ] 进行压力测试
- [ ] 配置备份策略

## 性能调优

### 系统级别

```bash
# 增加文件描述符限制
sudo vi /etc/security/limits.conf
```

添加：
```
pingora-slice soft nofile 65536
pingora-slice hard nofile 65536
```

### 应用级别

编辑 `/etc/pingora-slice/pingora_slice.yaml`：

```yaml
slice:
  slice_size: 2097152  # 2MB，根据文件大小调整
  max_concurrent_subrequests: 8  # 根据带宽调整
  
  cache:
    max_cache_size: 107374182400  # 100GB，根据磁盘空间调整
```

### 监控指标

关注以下关键指标：

- 缓存命中率（目标 > 70%）
- 请求延迟（P95 < 1s）
- 子请求失败率（< 5%）
- 内存使用率（< 80%）

## 故障排查

### 服务无法启动

```bash
# 查看详细日志
sudo journalctl -u pingora-slice -n 100 --no-pager

# 检查配置
sudo cat /etc/pingora-slice/pingora_slice.yaml

# 检查端口
sudo netstat -tlnp | grep -E '8080|9091'
```

### 性能问题

```bash
# 查看实时指标
curl http://localhost:9091/metrics

# 查看系统资源
top -p $(pgrep pingora-slice)

# 查看网络连接
sudo netstat -anp | grep pingora-slice
```

### 缓存问题

```bash
# 检查缓存目录
ls -lah /var/cache/pingora-slice/

# 检查权限
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice

# 清理缓存
sudo systemctl stop pingora-slice
sudo rm -rf /var/cache/pingora-slice/*
sudo systemctl start pingora-slice
```

## 升级流程

```bash
# 1. 下载新版本
VERSION=0.2.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el8.x86_64.rpm

# 2. 备份配置
sudo cp /etc/pingora-slice/pingora_slice.yaml /etc/pingora-slice/pingora_slice.yaml.backup

# 3. 升级
sudo dnf upgrade -y ./pingora-slice-${VERSION}-1.el8.x86_64.rpm

# 4. 检查配置
sudo diff /etc/pingora-slice/pingora_slice.yaml /etc/pingora-slice/pingora_slice.yaml.backup

# 5. 重启服务
sudo systemctl restart pingora-slice

# 6. 验证
sudo systemctl status pingora-slice
curl http://localhost:9091/health
```

## 备份和恢复

### 备份

```bash
# 备份配置
sudo tar czf pingora-slice-backup-$(date +%Y%m%d).tar.gz \
  /etc/pingora-slice/ \
  /var/cache/pingora-slice/

# 备份到远程
scp pingora-slice-backup-*.tar.gz backup-server:/backups/
```

### 恢复

```bash
# 恢复配置
sudo tar xzf pingora-slice-backup-20240101.tar.gz -C /

# 重启服务
sudo systemctl restart pingora-slice
```

## 安全建议

1. **最小权限原则**
   - 服务以非 root 用户运行
   - 限制文件系统访问

2. **网络安全**
   - 使用防火墙限制访问
   - 考虑使用 TLS/SSL

3. **监控和审计**
   - 启用详细日志
   - 定期审查访问日志
   - 配置告警

4. **定期更新**
   - 关注安全公告
   - 及时更新依赖
   - 定期升级系统

## 获取帮助

- 文档: https://github.com/your-username/pingora-slice/tree/main/docs
- Issues: https://github.com/your-username/pingora-slice/issues
- Discussions: https://github.com/your-username/pingora-slice/discussions

## 下一步

- 阅读 [QUICKSTART.md](QUICKSTART.md) 快速开始
- 查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何贡献
- 阅读 [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) 了解详细部署说明

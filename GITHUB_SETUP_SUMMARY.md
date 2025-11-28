# GitHub 环境配置总结

## 已完成的配置

### 1. GitHub Actions CI/CD

#### ✅ 持续集成 (`.github/workflows/ci.yml`)
- 自动运行测试（单元测试、集成测试、属性测试）
- 代码质量检查（rustfmt、clippy）
- 构建验证
- 在每次 push 和 PR 时触发

#### ✅ 发布流程 (`.github/workflows/release.yml`)
- 自动构建 CentOS 8 和 CentOS 9 的 RPM 包
- 创建 GitHub Release
- 上传 RPM 文件到 Release
- 生成详细的 Release 说明
- 支持两种触发方式：
  - Git Tag 推送（推荐）
  - 手动触发

### 2. RPM 打包

#### ✅ 打包文件
- `packaging/pingora-slice.spec.template` - RPM spec 模板
- `packaging/pingora-slice.service` - systemd 服务文件
- `packaging/install.sh` - 自动安装脚本
- `packaging/README.md` - 打包说明文档

#### ✅ 特性
- 自动创建 pingora-slice 用户和组
- systemd 服务集成
- 安全加固配置
- 自动权限设置
- 支持 CentOS 8/9, Rocky Linux 8/9, AlmaLinux 8/9

### 3. Docker 支持

#### ✅ Docker 文件
- `Dockerfile` - 多阶段构建，优化镜像大小
- `.dockerignore` - 排除不必要的文件
- `docker-compose.yml` - 完整的监控栈

#### ✅ 特性
- 非 root 用户运行
- 健康检查
- 包含 Prometheus 和 Grafana 监控

### 4. 监控配置

#### ✅ Prometheus
- `monitoring/prometheus.yml` - Prometheus 配置
- `monitoring/alerts.yml` - 告警规则
- 预配置的指标收集
- 完整的告警规则集

#### ✅ 告警规则
- 服务可用性监控
- 错误率告警
- 缓存命中率告警
- 请求延迟告警
- 子请求失败率告警
- 内存使用告警

### 5. 文档

#### ✅ 用户文档
- `QUICKSTART.md` - 快速开始指南
- `SETUP_GUIDE.md` - 完整设置指南
- `CONTRIBUTING.md` - 贡献指南
- `CHANGELOG.md` - 变更日志

#### ✅ 开发文档
- `packaging/README.md` - RPM 打包说明
- 完整的 API 文档
- 配置说明
- 部署指南

### 6. 开发工具

#### ✅ Makefile
提供便捷的开发命令：
- `make build` - 构建项目
- `make test` - 运行测试
- `make check` - 代码检查
- `make rpm` - 构建 RPM
- `make docker` - 构建 Docker 镜像
- 更多命令...

#### ✅ Git 配置
- `.gitignore` - 完善的忽略规则
- 清晰的提交信息规范

## 使用流程

### 首次设置

1. **更新仓库地址**
   ```bash
   # 替换所有文件中的 your-username
   YOUR_USERNAME="your-actual-username"
   find . -type f \( -name "*.yml" -o -name "*.yaml" -o -name "*.sh" -o -name "*.md" -o -name "*.service" \) \
     -exec sed -i "s/your-username\/pingora-slice/${YOUR_USERNAME}\/pingora-slice/g" {} +
   ```

2. **推送到 GitHub**
   ```bash
   git init
   git add .
   git commit -m "Initial commit: Pingora Slice v0.1.0"
   git branch -M main
   git remote add origin https://github.com/your-username/pingora-slice.git
   git push -u origin main
   ```

### 发布新版本

#### 方法 1：通过 Git Tag（推荐）

```bash
# 1. 更新版本号
vi Cargo.toml  # version = "0.1.0"

# 2. 更新 CHANGELOG
vi CHANGELOG.md

# 3. 提交
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.1.0"
git push

# 4. 创建并推送 tag
git tag v0.1.0
git push origin v0.1.0
```

#### 方法 2：手动触发

1. 访问 GitHub Actions
2. 选择 "Build and Release RPM"
3. 点击 "Run workflow"
4. 输入版本号
5. 运行

### 安装使用

#### 用户安装（CentOS 8）

```bash
# 使用安装脚本
curl -sSL https://raw.githubusercontent.com/your-username/pingora-slice/main/packaging/install.sh | sudo bash

# 或手动安装
VERSION=0.1.0
curl -LO https://github.com/your-username/pingora-slice/releases/download/v${VERSION}/pingora-slice-${VERSION}-1.el8.x86_64.rpm
sudo dnf install -y ./pingora-slice-${VERSION}-1.el8.x86_64.rpm
```

#### 配置和启动

```bash
# 编辑配置
sudo vi /etc/pingora-slice/pingora_slice.yaml

# 启动服务
sudo systemctl start pingora-slice
sudo systemctl enable pingora-slice

# 查看状态
sudo systemctl status pingora-slice
```

## 自动化流程

### CI 流程（每次 push/PR）

```
代码推送
  ↓
GitHub Actions 触发
  ↓
├─ 运行测试
│  ├─ 单元测试
│  ├─ 集成测试
│  └─ 属性测试
├─ 代码检查
│  ├─ rustfmt
│  └─ clippy
└─ 构建验证
```

### 发布流程（创建 tag）

```
创建 Git Tag (v0.1.0)
  ↓
GitHub Actions 触发
  ↓
并行构建 RPM
  ├─ CentOS 8 容器
  │  ├─ 安装依赖
  │  ├─ 构建二进制
  │  └─ 打包 RPM
  └─ CentOS 9 容器
     ├─ 安装依赖
     ├─ 构建二进制
     └─ 打包 RPM
  ↓
创建 GitHub Release
  ├─ 生成 Release 说明
  ├─ 上传 el8 RPM
  └─ 上传 el9 RPM
  ↓
发布完成
```

## 监控架构

```
Pingora Slice (:8080, :9091)
  ↓ (metrics)
Prometheus (:9090)
  ↓ (data source)
Grafana (:3000)
  ↓ (visualization)
用户仪表板
```

## 文件结构

```
pingora-slice/
├── .github/
│   └── workflows/
│       ├── ci.yml              # CI 流程
│       └── release.yml         # 发布流程
├── packaging/
│   ├── pingora-slice.spec.template
│   ├── pingora-slice.service
│   ├── install.sh
│   └── README.md
├── monitoring/
│   ├── prometheus.yml
│   └── alerts.yml
├── docs/                       # 文档目录
├── src/                        # 源代码
├── tests/                      # 测试
├── examples/                   # 示例
├── Dockerfile                  # Docker 构建
├── docker-compose.yml          # Docker Compose
├── Makefile                    # 开发工具
├── QUICKSTART.md              # 快速开始
├── SETUP_GUIDE.md             # 设置指南
├── CONTRIBUTING.md            # 贡献指南
├── CHANGELOG.md               # 变更日志
└── README.md                  # 项目说明
```

## 关键配置点

### 1. GitHub Actions Secrets

不需要额外配置，使用默认的 `GITHUB_TOKEN`。

### 2. RPM 构建环境

- 使用官方 CentOS Stream 容器镜像
- 自动安装所有依赖
- 完全自动化的构建流程

### 3. 版本管理

- 版本号在 `Cargo.toml` 中定义
- Git tag 格式：`v0.1.0`
- RPM 版本格式：`0.1.0-1.el8`

### 4. 发布说明

自动生成，包含：
- 版本信息
- 安装说明（CentOS 8/9）
- 配置和启动步骤
- 功能特性列表
- 文档链接

## 测试验证

### 本地测试

```bash
# 运行所有测试
make test

# 构建 RPM（需要 rpmbuild）
make rpm

# 构建 Docker 镜像
make docker
```

### CI 测试

推送代码后，在 GitHub Actions 中查看：
```
https://github.com/your-username/pingora-slice/actions
```

### 发布测试

创建测试 tag：
```bash
git tag v0.0.1-test
git push origin v0.0.1-test
```

## 生产环境部署

### 推荐配置

```yaml
# /etc/pingora-slice/pingora_slice.yaml
listen:
  address: "0.0.0.0:8080"
  threads: 8

upstream:
  address: "origin.example.com:80"
  timeout: 30

slice:
  slice_size: 2097152  # 2MB
  max_concurrent_subrequests: 8
  max_retries: 3
  
  slice_patterns:
    - "^/downloads/.*"
    - "^/files/.*\\.(iso|bin)$"
  
  cache:
    enabled: true
    ttl: 3600
    storage: "file"
    cache_dir: "/var/cache/pingora-slice"
    max_cache_size: 107374182400  # 100GB

metrics:
  enabled: true
  listen: "0.0.0.0:9091"
```

### 监控配置

1. 部署 Prometheus
2. 配置数据源指向 `:9091/metrics`
3. 导入告警规则
4. 配置 Grafana 仪表板

## 故障排查

### GitHub Actions 失败

1. 查看 Actions 日志
2. 检查 Rust 版本兼容性
3. 验证依赖是否可用

### RPM 安装失败

1. 检查系统版本（el8/el9）
2. 验证依赖已安装
3. 查看安装日志

### 服务启动失败

```bash
# 查看日志
sudo journalctl -u pingora-slice -n 50

# 检查配置
sudo cat /etc/pingora-slice/pingora_slice.yaml

# 验证权限
ls -la /var/cache/pingora-slice
```

## 下一步

1. ✅ 推送代码到 GitHub
2. ✅ 验证 CI 通过
3. ✅ 创建第一个 release
4. ✅ 测试 RPM 安装
5. ✅ 配置监控
6. ✅ 编写使用文档
7. ✅ 宣传项目

## 维护建议

### 定期任务

- 每周检查依赖更新
- 每月审查安全公告
- 季度性能评估
- 及时响应 Issues

### 版本发布

- 遵循语义化版本
- 更新 CHANGELOG
- 编写 Release Notes
- 通知用户升级

## 支持资源

- GitHub Issues: 问题报告和功能请求
- GitHub Discussions: 社区讨论
- Documentation: 完整文档
- Examples: 使用示例

---

## 总结

你现在拥有一个完整的、生产就绪的 CI/CD 环境：

✅ 自动化测试
✅ 自动化构建
✅ 自动化发布
✅ RPM 包分发
✅ Docker 支持
✅ 监控集成
✅ 完整文档

只需推送代码和创建 tag，其余的都是自动化的！

祝你的项目成功！🚀

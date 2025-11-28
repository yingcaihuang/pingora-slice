# Pingora Slice RPM 打包说明

本目录包含用于构建和分发 Pingora Slice RPM 包的文件。

## 文件说明

- `pingora-slice.spec.template` - RPM spec 文件模板
- `pingora-slice.service` - systemd 服务单元文件
- `install.sh` - 自动安装脚本

## GitHub Actions 自动构建

### 触发构建

有两种方式触发 RPM 构建：

#### 1. 通过 Git Tag（推荐）

```bash
# 创建并推送 tag
git tag v0.1.0
git push origin v0.1.0
```

#### 2. 手动触发

在 GitHub 仓库页面：
1. 进入 "Actions" 标签
2. 选择 "Build and Release RPM" workflow
3. 点击 "Run workflow"
4. 输入版本号（例如：0.1.0）
5. 点击 "Run workflow"

### 构建产物

构建完成后，会自动创建 GitHub Release，包含：

- `pingora-slice-{version}-1.el8.x86_64.rpm` - CentOS 8 / Rocky Linux 8 / AlmaLinux 8
- `pingora-slice-{version}-1.el9.x86_64.rpm` - CentOS 9 / Rocky Linux 9 / AlmaLinux 9

## 安装方法

### 方法 1：使用安装脚本（推荐）

```bash
# 下载安装脚本
curl -O https://raw.githubusercontent.com/your-username/pingora-slice/main/packaging/install.sh

# 赋予执行权限
chmod +x install.sh

# 安装最新版本
sudo ./install.sh

# 或安装指定版本
sudo ./install.sh 0.1.0
```

### 方法 2：手动下载安装

#### CentOS 8 / Rocky Linux 8 / AlmaLinux 8

```bash
# 下载 RPM
curl -LO https://github.com/your-username/pingora-slice/releases/download/v0.1.0/pingora-slice-0.1.0-1.el8.x86_64.rpm

# 安装
sudo dnf install -y ./pingora-slice-0.1.0-1.el8.x86_64.rpm
```

#### CentOS 9 / Rocky Linux 9 / AlmaLinux 9

```bash
# 下载 RPM
curl -LO https://github.com/your-username/pingora-slice/releases/download/v0.1.0/pingora-slice-0.1.0-1.el9.x86_64.rpm

# 安装
sudo dnf install -y ./pingora-slice-0.1.0-1.el9.x86_64.rpm
```

## 配置和使用

### 1. 编辑配置文件

```bash
sudo vi /etc/pingora-slice/pingora_slice.yaml
```

配置示例：

```yaml
# 上游服务器配置
upstream:
  address: "origin.example.com:80"

# Slice 模块配置
slice:
  slice_size: 1048576  # 1MB
  max_concurrent_subrequests: 4
  max_retries: 3
  
  slice_patterns:
    - "^/large-files/.*"
    - "^/downloads/.*\\.bin$"
  
  cache:
    enabled: true
    ttl: 3600
    storage: "file"
    cache_dir: "/var/cache/pingora-slice"
    max_cache_size: 10737418240  # 10GB
```

### 2. 启动服务

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

### 3. 验证服务

```bash
# 检查服务是否运行
curl http://localhost:8080/health

# 查看指标
curl http://localhost:9091/metrics
```

## 服务管理

### 重启服务

```bash
sudo systemctl restart pingora-slice
```

### 重新加载配置

```bash
sudo systemctl reload pingora-slice
```

### 停止服务

```bash
sudo systemctl stop pingora-slice
```

### 查看日志

```bash
# 实时查看日志
sudo journalctl -u pingora-slice -f

# 查看最近的日志
sudo journalctl -u pingora-slice -n 100

# 查看特定时间的日志
sudo journalctl -u pingora-slice --since "2024-01-01" --until "2024-01-02"
```

## 卸载

```bash
# 停止并禁用服务
sudo systemctl stop pingora-slice
sudo systemctl disable pingora-slice

# 卸载 RPM
sudo dnf remove -y pingora-slice

# 清理数据（可选）
sudo rm -rf /var/cache/pingora-slice
sudo rm -rf /var/log/pingora-slice
```

## 升级

```bash
# 下载新版本 RPM
curl -LO https://github.com/your-username/pingora-slice/releases/download/v0.2.0/pingora-slice-0.2.0-1.el8.x86_64.rpm

# 升级
sudo dnf upgrade -y ./pingora-slice-0.2.0-1.el8.x86_64.rpm

# 重启服务
sudo systemctl restart pingora-slice
```

## 故障排查

### 服务无法启动

```bash
# 查看详细错误信息
sudo journalctl -u pingora-slice -n 50 --no-pager

# 检查配置文件语法
pingora-slice --check-config /etc/pingora-slice/pingora_slice.yaml

# 检查端口占用
sudo netstat -tlnp | grep 8080
```

### 权限问题

```bash
# 确保目录权限正确
sudo chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
sudo chown -R pingora-slice:pingora-slice /var/log/pingora-slice
sudo chmod 755 /var/cache/pingora-slice
sudo chmod 755 /var/log/pingora-slice
```

### 性能调优

```bash
# 增加文件描述符限制
sudo vi /etc/systemd/system/pingora-slice.service.d/override.conf
```

添加：

```ini
[Service]
LimitNOFILE=100000
```

然后重新加载：

```bash
sudo systemctl daemon-reload
sudo systemctl restart pingora-slice
```

## 本地构建 RPM

如果需要本地构建 RPM：

```bash
# 安装构建工具
sudo dnf install -y rpm-build rpmdevtools

# 设置 RPM 构建环境
rpmdev-setuptree

# 构建二进制
cargo build --release

# 复制 spec 文件并替换变量
cp packaging/pingora-slice.spec.template ~/rpmbuild/SPECS/pingora-slice.spec

# 编辑 spec 文件，替换占位符
vi ~/rpmbuild/SPECS/pingora-slice.spec

# 构建 RPM
rpmbuild -bb ~/rpmbuild/SPECS/pingora-slice.spec

# RPM 文件位于
ls ~/rpmbuild/RPMS/x86_64/
```

## 支持的系统

- ✅ CentOS 8 / CentOS Stream 8
- ✅ Rocky Linux 8
- ✅ AlmaLinux 8
- ✅ CentOS 9 / CentOS Stream 9
- ✅ Rocky Linux 9
- ✅ AlmaLinux 9

## 许可证

MIT License

## 问题反馈

如有问题，请在 GitHub Issues 中反馈：
https://github.com/your-username/pingora-slice/issues

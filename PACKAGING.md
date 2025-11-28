# Pingora Slice 打包指南

本文档说明如何为不同平台构建和发布 Pingora Slice 包。

## 支持的平台

### Linux RPM
- CentOS 8 / Rocky Linux 8 / AlmaLinux 8 (el8)
- CentOS 9 / Rocky Linux 9 / AlmaLinux 9 (el9)

### Linux DEB
- Ubuntu 20.04 (Focal Fossa)
- Ubuntu 22.04 (Jammy Jellyfish)
- Debian 11 (Bullseye)
- Debian 12 (Bookworm)

### macOS
- macOS 11+ (Big Sur and later)
- Intel (x86_64) and Apple Silicon (arm64)

## 自动构建 (GitHub Actions)

### 触发构建

#### 方式 1: 推送 Tag
```bash
git tag -a v0.2.1 -m "Release v0.2.1"
git push origin v0.2.1
```

#### 方式 2: 手动触发
1. 访问 GitHub Actions 页面
2. 选择 "Build and Release Packages" workflow
3. 点击 "Run workflow"
4. 输入版本号（例如：0.2.1）
5. 点击 "Run workflow"

### 构建产物

构建完成后，会自动创建 GitHub Release，包含以下文件：

**RPM 包:**
- `pingora-slice-{version}-1.el8.x86_64.rpm`
- `pingora-slice-{version}-1.el9.x86_64.rpm`

**DEB 包:**
- `pingora-slice_{version}_focal_amd64.deb` (Ubuntu 20.04)
- `pingora-slice_{version}_jammy_amd64.deb` (Ubuntu 22.04)
- `pingora-slice_{version}_bullseye_amd64.deb` (Debian 11)
- `pingora-slice_{version}_bookworm_amd64.deb` (Debian 12)

**macOS 包:**
- `pingora-slice-{version}-macos-x86_64.tar.gz` (Intel)
- `pingora-slice-{version}-macos-arm64.tar.gz` (Apple Silicon)
- `install-macos.sh` (安装脚本)

## 手动构建

### RPM 包

#### 前置条件
```bash
# CentOS/Rocky/AlmaLinux
sudo dnf install -y gcc gcc-c++ make cmake openssl-devel rpm-build rpmdevtools

# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 构建步骤
```bash
# 1. 构建二进制文件
cargo build --release

# 2. 使用 Makefile 构建 RPM
make rpm

# 或者手动构建
rpmdev-setuptree
cp packaging/pingora-slice.spec.template ~/rpmbuild/SPECS/pingora-slice.spec
# 编辑 spec 文件，替换占位符
rpmbuild -bb ~/rpmbuild/SPECS/pingora-slice.spec
```

#### 安装
```bash
sudo dnf install -y ~/rpmbuild/RPMS/x86_64/pingora-slice-*.rpm
```

### DEB 包

#### 前置条件
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y build-essential libssl-dev pkg-config dpkg-dev debhelper

# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 构建步骤
```bash
# 1. 构建二进制文件
cargo build --release
strip target/release/pingora-slice

# 2. 创建包结构
mkdir -p debian-package/DEBIAN
mkdir -p debian-package/usr/bin
mkdir -p debian-package/etc/pingora-slice
mkdir -p debian-package/lib/systemd/system

# 3. 复制文件
cp target/release/pingora-slice debian-package/usr/bin/
cp examples/pingora_slice.yaml debian-package/etc/pingora-slice/

# 4. 创建 control 文件
cat > debian-package/DEBIAN/control << EOF
Package: pingora-slice
Version: 0.2.1
Section: net
Priority: optional
Architecture: amd64
Maintainer: Your Name <your.email@example.com>
Depends: libc6, libssl3 | libssl1.1
Description: Pingora Slice Module
 High-performance proxy module for automatic file slicing
EOF

# 5. 创建 postinst 脚本
cat > debian-package/DEBIAN/postinst << 'EOF'
#!/bin/bash
set -e
# 创建用户和组
if ! getent group pingora-slice >/dev/null; then
    addgroup --system pingora-slice
fi
if ! getent passwd pingora-slice >/dev/null; then
    adduser --system --ingroup pingora-slice --home /var/cache/pingora-slice \
            --no-create-home --shell /usr/sbin/nologin pingora-slice
fi
exit 0
EOF
chmod 755 debian-package/DEBIAN/postinst

# 6. 构建 DEB 包
dpkg-deb --build debian-package pingora-slice_0.2.1_amd64.deb
```

#### 安装
```bash
sudo dpkg -i pingora-slice_0.2.1_amd64.deb
sudo apt-get install -f  # 安装依赖
```

### macOS 包

#### 前置条件
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 构建步骤
```bash
# 1. 构建二进制文件
cargo build --release
strip target/release/pingora-slice

# 2. 创建包结构
mkdir -p macos-package/usr/local/bin
mkdir -p macos-package/usr/local/etc/pingora-slice
mkdir -p macos-package/Library/LaunchDaemons

# 3. 复制文件
cp target/release/pingora-slice macos-package/usr/local/bin/
cp examples/pingora_slice.yaml macos-package/usr/local/etc/pingora-slice/

# 4. 创建 LaunchDaemon plist
cat > macos-package/Library/LaunchDaemons/com.pingora.slice.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.pingora.slice</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/pingora-slice</string>
        <string>/usr/local/etc/pingora-slice/pingora_slice.yaml</string>
    </array>
    <key>RunAtLoad</key>
    <false/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
EOF

# 5. 创建 tarball
cd macos-package
tar czf ../pingora-slice-0.2.1-macos-$(uname -m).tar.gz *
cd ..
```

#### 安装
```bash
# 解压
sudo tar xzf pingora-slice-0.2.1-macos-*.tar.gz -C /

# 设置权限
sudo chmod +x /usr/local/bin/pingora-slice
sudo chmod 644 /Library/LaunchDaemons/com.pingora.slice.plist

# 启动服务
sudo launchctl load /Library/LaunchDaemons/com.pingora.slice.plist
```

## 包内容

### 文件位置

#### Linux (RPM/DEB)
```
/usr/bin/pingora-slice                          # 主程序
/etc/pingora-slice/pingora_slice.yaml           # 配置文件
/lib/systemd/system/pingora-slice.service       # Systemd 服务
/var/cache/pingora-slice/                       # 缓存目录
/var/log/pingora-slice/                         # 日志目录
```

#### macOS
```
/usr/local/bin/pingora-slice                              # 主程序
/usr/local/etc/pingora-slice/pingora_slice.yaml          # 配置文件
/Library/LaunchDaemons/com.pingora.slice.plist            # LaunchDaemon
/usr/local/var/log/pingora-slice.log                      # 日志文件
```

### 用户和权限

**Linux:**
- 用户: `pingora-slice`
- 组: `pingora-slice`
- 主目录: `/var/cache/pingora-slice`
- Shell: `/sbin/nologin` (RPM) 或 `/usr/sbin/nologin` (DEB)

**macOS:**
- 以 root 权限运行（通过 LaunchDaemon）

## 服务管理

### Linux (systemd)

```bash
# 启动服务
sudo systemctl start pingora-slice

# 停止服务
sudo systemctl stop pingora-slice

# 重启服务
sudo systemctl restart pingora-slice

# 开机自启
sudo systemctl enable pingora-slice

# 查看状态
sudo systemctl status pingora-slice

# 查看日志
sudo journalctl -u pingora-slice -f
```

### macOS (launchd)

```bash
# 启动服务
sudo launchctl load /Library/LaunchDaemons/com.pingora.slice.plist

# 停止服务
sudo launchctl unload /Library/LaunchDaemons/com.pingora.slice.plist

# 查看日志
tail -f /usr/local/var/log/pingora-slice.log
```

## 卸载

### RPM
```bash
sudo dnf remove pingora-slice
```

### DEB
```bash
# 卸载但保留配置
sudo apt-get remove pingora-slice

# 完全卸载（包括配置）
sudo apt-get purge pingora-slice
```

### macOS
```bash
# 停止服务
sudo launchctl unload /Library/LaunchDaemons/com.pingora.slice.plist

# 删除文件
sudo rm /usr/local/bin/pingora-slice
sudo rm /Library/LaunchDaemons/com.pingora.slice.plist
sudo rm -rf /usr/local/etc/pingora-slice
sudo rm -rf /usr/local/var/log/pingora-slice*
```

## 故障排查

### RPM 安装失败

**问题**: `%post scriptlet failed`

**解决**:
```bash
# 检查用户是否已存在
id pingora-slice

# 手动创建用户
sudo groupadd -r pingora-slice
sudo useradd -r -g pingora-slice -d /var/cache/pingora-slice -s /sbin/nologin pingora-slice

# 重新安装
sudo dnf install -y ./pingora-slice-*.rpm
```

### DEB 安装失败

**问题**: 依赖问题

**解决**:
```bash
# 安装依赖
sudo apt-get install -f

# 或手动安装依赖
sudo apt-get install libc6 libssl3
```

### macOS 权限问题

**问题**: `Operation not permitted`

**解决**:
```bash
# 使用 sudo
sudo launchctl load /Library/LaunchDaemons/com.pingora.slice.plist

# 检查文件权限
ls -l /Library/LaunchDaemons/com.pingora.slice.plist
```

## 版本管理

### 更新版本号

1. 更新 `Cargo.toml`:
```toml
[package]
version = "0.2.1"
```

2. 更新 `CHANGELOG.md`

3. 提交并打 tag:
```bash
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.2.1"
git tag -a v0.2.1 -m "Release v0.2.1"
git push origin master
git push origin v0.2.1
```

4. GitHub Actions 会自动构建所有平台的包

## 测试

### 本地测试

```bash
# 构建
cargo build --release

# 运行
./target/release/pingora-slice examples/pingora_slice.yaml

# 测试
curl http://localhost:8080/stats
```

### 包测试

```bash
# RPM
sudo dnf install -y ./pingora-slice-*.rpm
sudo systemctl start pingora-slice
curl http://localhost:8080/stats

# DEB
sudo dpkg -i pingora-slice_*.deb
sudo systemctl start pingora-slice
curl http://localhost:8080/stats

# macOS
sudo tar xzf pingora-slice-*-macos-*.tar.gz -C /
sudo launchctl load /Library/LaunchDaemons/com.pingora.slice.plist
curl http://localhost:8080/stats
```

## 发布检查清单

- [ ] 更新版本号 (Cargo.toml)
- [ ] 更新 CHANGELOG.md
- [ ] 运行所有测试 (`cargo test`)
- [ ] 本地构建测试 (`cargo build --release`)
- [ ] 提交更改
- [ ] 创建并推送 tag
- [ ] 等待 GitHub Actions 完成
- [ ] 验证 GitHub Release
- [ ] 测试下载的包
- [ ] 更新文档（如需要）

## 相关文档

- [BUILD_BINARY.md](BUILD_BINARY.md) - 二进制构建说明
- [RPM_SCRIPTLET_FIX.md](RPM_SCRIPTLET_FIX.md) - RPM 脚本修复
- [DEPLOYMENT.md](docs/DEPLOYMENT.md) - 部署指南
- [CONFIGURATION.md](docs/CONFIGURATION.md) - 配置指南

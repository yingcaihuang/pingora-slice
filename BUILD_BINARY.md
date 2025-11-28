# Pingora Slice 二进制构建说明

## 概述

Pingora Slice 项目使用 `examples/http_purge_server.rs` 作为主程序二进制文件。这个配置允许我们将功能完整的 HTTP PURGE 服务器作为默认的可执行文件。

## 配置

### Cargo.toml 配置

在 `Cargo.toml` 中添加了以下配置：

```toml
# Define the main binary using the http_purge_server example
[[bin]]
name = "pingora-slice"
path = "examples/http_purge_server.rs"
```

这个配置告诉 Cargo：
- 创建一个名为 `pingora-slice` 的二进制文件
- 使用 `examples/http_purge_server.rs` 作为源文件

### 依赖调整

将 `tempfile` 从 `dev-dependencies` 移到了 `dependencies`，因为主程序需要它：

```toml
[dependencies]
# ... 其他依赖 ...
tempfile = "3.0"
```

## 构建

### 开发构建

```bash
cargo build
./target/debug/pingora-slice
```

### 发布构建

```bash
cargo build --release
./target/release/pingora-slice
```

### 使用 Makefile

```bash
# 构建发布版本
make release

# 运行发布版本
make run-release

# 安装到系统
sudo make install
```

## 功能特性

编译后的 `pingora-slice` 二进制文件包含以下功能：

### 1. 两层缓存系统
- **L1 缓存**: 内存缓存 (默认 10MB)
- **L2 缓存**: 磁盘缓存 (使用临时目录)
- 自动从 L2 提升到 L1
- LRU 淘汰策略

### 2. HTTP PURGE 支持
- 清除特定 URL: `PURGE http://localhost:8080/path`
- 清除所有缓存: `PURGE http://localhost:8080/* -H "X-Purge-All: true"`
- 可选的 Token 认证 (通过 `PURGE_TOKEN` 环境变量)

### 3. 监控端点
- `/stats` - 缓存统计信息 (JSON)
- `/metrics` - Prometheus 指标
- `/health` - 健康检查 (隐含)

### 4. 测试数据
服务器启动时会自动填充测试数据：
- `http://localhost:8080/test.dat` (5 个切片)
- `http://localhost:8080/video.mp4` (10 个切片)
- `http://localhost:8080/image.jpg` (3 个切片)

## 使用示例

### 启动服务器

```bash
# 直接运行
./target/release/pingora-slice

# 使用认证
PURGE_TOKEN=secret ./target/release/pingora-slice
```

### 测试命令

```bash
# 查看缓存统计
curl http://localhost:8080/stats

# 查看 Prometheus 指标
curl http://localhost:8080/metrics

# 获取缓存文件 (应该命中)
curl http://localhost:8080/test.dat

# 清除特定 URL
curl -X PURGE http://localhost:8080/test.dat

# 验证已清除 (应该未命中)
curl http://localhost:8080/test.dat

# 清除所有缓存
curl -X PURGE http://localhost:8080/* -H 'X-Purge-All: true'

# 使用认证清除
curl -X PURGE http://localhost:8080/test.dat -H 'Authorization: Bearer secret'
```

## RPM 打包

RPM 打包配置已经正确设置，会使用编译后的 `pingora-slice` 二进制文件：

```bash
# 构建 RPM
make rpm

# 或者手动构建
cargo build --release
rpmbuild -bb packaging/pingora-slice.spec
```

RPM 包会安装：
- 二进制文件: `/usr/bin/pingora-slice`
- 配置文件: `/etc/pingora-slice/pingora_slice.yaml`
- Systemd 服务: `/usr/lib/systemd/system/pingora-slice.service`
- 缓存目录: `/var/cache/pingora-slice`
- 日志目录: `/var/log/pingora-slice`

## 与 src/main.rs 的关系

- `src/main.rs` 目前是一个占位文件或旧版本
- 实际的主程序是 `examples/http_purge_server.rs`
- 这个设计允许我们：
  - 保持 examples 目录的示例代码
  - 使用功能完整的服务器作为默认二进制
  - 在需要时轻松切换或修改主程序

## 未来改进

如果需要从配置文件读取设置而不是使用硬编码的测试数据，可以：

1. 修改 `examples/http_purge_server.rs` 以接受配置文件参数
2. 或者创建一个新的主程序文件并更新 `Cargo.toml` 中的 `path`
3. 或者将 `examples/http_purge_server.rs` 的内容移到 `src/main.rs`

## 验证

验证二进制文件是从正确的源文件编译的：

```bash
# 检查二进制文件中的字符串
strings target/release/pingora-slice | grep "HTTP PURGE server"

# 应该看到: "Starting HTTP PURGE server..."
```

## 总结

这个配置提供了一个功能完整的 HTTP PURGE 服务器作为 Pingora Slice 的默认二进制文件，包含：
- ✅ 两层缓存系统
- ✅ HTTP PURGE 支持
- ✅ Prometheus 指标
- ✅ 缓存统计
- ✅ 可选认证
- ✅ 测试数据预填充
- ✅ RPM 打包支持

所有构建和打包工具都已正确配置，可以直接使用。

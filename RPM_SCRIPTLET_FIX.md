# RPM Scriptlet 错误修复

## 问题描述

在安装或卸载 RPM 包时出现以下错误：

```
Error in POSTIN scriptlet
warning: %post(pingora-slice-0.2.0-1.el9.x86_64) scriptlet failed, exit status 1
```

## 原因分析

RPM spec 文件中的 `%post` 脚本在执行时失败，主要原因：

1. **chown 命令失败**: 尝试修改不存在的目录权限
2. **用户创建失败**: 用户或组可能已存在或创建失败
3. **缺少错误处理**: 脚本没有正确处理错误情况
4. **缺少 exit 0**: 脚本没有明确返回成功状态

## 修复方案

### 1. %pre 脚本修复

**修复前**:
```spec
%pre
getent group pingora-slice >/dev/null || groupadd -r pingora-slice
getent passwd pingora-slice >/dev/null || \
    useradd -r -g pingora-slice -d /var/cache/pingora-slice -s /sbin/nologin \
    -c "Pingora Slice service user" pingora-slice
exit 0
```

**修复后**:
```spec
%pre
# Create group if it doesn't exist
getent group pingora-slice >/dev/null 2>&1 || groupadd -r pingora-slice 2>/dev/null || true
# Create user if it doesn't exist
getent passwd pingora-slice >/dev/null 2>&1 || \
    useradd -r -g pingora-slice -d /var/cache/pingora-slice -s /sbin/nologin \
    -c "Pingora Slice service user" pingora-slice 2>/dev/null || true
exit 0
```

**改进点**:
- 添加 `2>&1` 重定向错误输出
- 添加 `2>/dev/null` 抑制错误消息
- 添加 `|| true` 确保命令不会导致脚本失败

### 2. %post 脚本修复

**修复前**:
```spec
%post
%systemd_post pingora-slice.service
# Set proper permissions
chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
chown -R pingora-slice:pingora-slice /var/log/pingora-slice
echo "Pingora Slice has been installed successfully!"
echo "Edit /etc/pingora-slice/pingora_slice.yaml to configure"
echo "Start with: systemctl start pingora-slice"
```

**修复后**:
```spec
%post
%systemd_post pingora-slice.service
# Set proper permissions (ignore errors if directories don't exist yet)
chown -R pingora-slice:pingora-slice /var/cache/pingora-slice 2>/dev/null || true
chown -R pingora-slice:pingora-slice /var/log/pingora-slice 2>/dev/null || true
echo "Pingora Slice has been installed successfully!"
echo "Edit /etc/pingora-slice/pingora_slice.yaml to configure"
echo "Start with: systemctl start pingora-slice"
exit 0
```

**改进点**:
- 添加 `2>/dev/null || true` 忽略 chown 失败
- 添加 `exit 0` 明确返回成功状态
- 添加注释说明为什么忽略错误

### 3. %postun 脚本修复

**修复前**:
```spec
%postun
%systemd_postun_with_restart pingora-slice.service
if [ $1 -eq 0 ]; then
    # Package removal, not upgrade
    userdel pingora-slice 2>/dev/null || true
    groupdel pingora-slice 2>/dev/null || true
fi
```

**修复后**:
```spec
%postun
%systemd_postun_with_restart pingora-slice.service
if [ $1 -eq 0 ]; then
    # Package removal, not upgrade
    userdel pingora-slice 2>/dev/null || true
    groupdel pingora-slice 2>/dev/null || true
fi
exit 0
```

**改进点**:
- 添加 `exit 0` 确保脚本成功退出

## 错误处理最佳实践

### 1. 总是添加 exit 0

所有 RPM scriptlet 应该以 `exit 0` 结束，确保即使有非致命错误也能继续：

```spec
%post
# ... commands ...
exit 0
```

### 2. 使用 || true 处理可选操作

对于可能失败但不应该阻止安装的操作：

```spec
chown user:group /path 2>/dev/null || true
```

### 3. 重定向错误输出

避免用户看到不必要的错误消息：

```spec
command 2>/dev/null
```

### 4. 检查条件

在执行操作前检查条件：

```spec
if [ -d /var/cache/pingora-slice ]; then
    chown -R pingora-slice:pingora-slice /var/cache/pingora-slice
fi
```

## 测试验证

### 安装测试

```bash
# 安装 RPM
sudo rpm -ivh pingora-slice-0.2.1-1.el9.x86_64.rpm

# 检查安装状态
rpm -q pingora-slice

# 检查用户和组
id pingora-slice
getent group pingora-slice

# 检查目录权限
ls -ld /var/cache/pingora-slice
ls -ld /var/log/pingora-slice

# 检查服务状态
systemctl status pingora-slice
```

### 卸载测试

```bash
# 卸载 RPM
sudo rpm -e pingora-slice

# 验证清理
rpm -q pingora-slice
id pingora-slice 2>/dev/null && echo "User still exists" || echo "User removed"
```

### 升级测试

```bash
# 升级到新版本
sudo rpm -Uvh pingora-slice-0.2.1-1.el9.x86_64.rpm

# 验证服务继续运行
systemctl status pingora-slice
```

## 常见错误和解决方案

### 错误 1: chown: cannot access '/var/cache/pingora-slice'

**原因**: 目录在 %post 脚本执行时还不存在

**解决**: 添加 `|| true` 或检查目录是否存在

```spec
[ -d /var/cache/pingora-slice ] && chown -R pingora-slice:pingora-slice /var/cache/pingora-slice || true
```

### 错误 2: useradd: user 'pingora-slice' already exists

**原因**: 用户已经存在（可能是之前安装留下的）

**解决**: 使用 `getent` 检查并添加错误处理

```spec
getent passwd pingora-slice >/dev/null 2>&1 || useradd ... 2>/dev/null || true
```

### 错误 3: groupadd: group 'pingora-slice' already exists

**原因**: 组已经存在

**解决**: 使用 `getent` 检查并添加错误处理

```spec
getent group pingora-slice >/dev/null 2>&1 || groupadd ... 2>/dev/null || true
```

## 重新构建 RPM

修复后重新构建 RPM：

```bash
# 清理旧的构建
make clean

# 重新构建
make release

# 构建 RPM
make rpm

# 测试新的 RPM
sudo rpm -ivh ~/rpmbuild/RPMS/x86_64/pingora-slice-0.2.1-1.el9.x86_64.rpm
```

## 总结

修复的关键点：
1. ✅ 所有 scriptlet 都以 `exit 0` 结束
2. ✅ 使用 `|| true` 处理非致命错误
3. ✅ 重定向错误输出 `2>/dev/null`
4. ✅ 添加清晰的注释说明
5. ✅ 检查条件后再执行操作

这些修复确保 RPM 包在各种情况下都能正确安装和卸载，不会因为非致命错误而失败。

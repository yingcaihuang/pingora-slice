# 贡献指南

感谢你对 Pingora Slice 项目的关注！我们欢迎各种形式的贡献。

## 如何贡献

### 报告 Bug

如果你发现了 bug，请在 GitHub Issues 中创建一个新的 issue，并包含以下信息：

- 清晰的标题和描述
- 重现步骤
- 预期行为和实际行为
- 系统环境（OS、版本等）
- 相关日志或错误信息

### 提出新功能

如果你有新功能的想法：

1. 先在 Issues 中搜索是否已有类似建议
2. 创建新的 issue，描述功能需求和使用场景
3. 等待维护者反馈

### 提交代码

#### 1. Fork 项目

```bash
# Fork 项目到你的账号
# 然后克隆到本地
git clone https://github.com/your-username/pingora-slice.git
cd pingora-slice
```

#### 2. 创建分支

```bash
# 从 main/master 分支创建新分支
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/your-bug-fix
```

#### 3. 开发

```bash
# 安装依赖
cargo build

# 运行测试
cargo test

# 运行 linter
cargo clippy
cargo fmt
```

#### 4. 提交代码

```bash
# 添加修改
git add .

# 提交（使用清晰的提交信息）
git commit -m "feat: add new feature"
# 或
git commit -m "fix: resolve issue #123"
```

提交信息格式：

- `feat:` 新功能
- `fix:` Bug 修复
- `docs:` 文档更新
- `style:` 代码格式调整
- `refactor:` 代码重构
- `test:` 测试相关
- `chore:` 构建或辅助工具变动

#### 5. 推送并创建 Pull Request

```bash
# 推送到你的 fork
git push origin feature/your-feature-name

# 在 GitHub 上创建 Pull Request
```

## 开发环境设置

### 必需工具

- Rust 1.70+
- Cargo
- Git

### 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 克隆项目

```bash
git clone https://github.com/your-username/pingora-slice.git
cd pingora-slice
```

### 构建项目

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name

# 运行集成测试
cargo test --test test_integration

# 运行属性测试
cargo test --test prop_slice_coverage
```

### 代码检查

```bash
# 运行 clippy
cargo clippy --all-targets --all-features -- -D warnings

# 格式化代码
cargo fmt

# 检查格式
cargo fmt -- --check
```

## 代码规范

### Rust 代码风格

- 遵循 Rust 官方代码风格指南
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 添加适当的文档注释

### 文档注释

```rust
/// 计算文件的分片
///
/// # Arguments
///
/// * `file_size` - 文件总大小（字节）
/// * `slice_size` - 每个分片的大小（字节）
///
/// # Returns
///
/// 返回分片规格的向量
///
/// # Examples
///
/// ```
/// let slices = calculate_slices(10000, 1000);
/// assert_eq!(slices.len(), 10);
/// ```
pub fn calculate_slices(file_size: u64, slice_size: usize) -> Vec<SliceSpec> {
    // 实现
}
```

### 测试要求

- 新功能必须包含单元测试
- 修复 bug 应添加回归测试
- 保持测试覆盖率在 80% 以上
- 属性测试用于验证正确性属性

### 错误处理

```rust
// 使用 Result 类型
pub fn fetch_metadata(&self, url: &str) -> Result<FileMetadata, SliceError> {
    // 实现
}

// 自定义错误类型
#[derive(Debug, thiserror::Error)]
pub enum SliceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("HTTP error: {0}")]
    HttpError(String),
}
```

## 测试指南

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_slices() {
        let slices = calculate_slices(10000, 1000);
        assert_eq!(slices.len(), 10);
        assert_eq!(slices[0].range.start, 0);
        assert_eq!(slices[0].range.end, 999);
    }
}
```

### 集成测试

放在 `tests/` 目录下：

```rust
// tests/test_integration.rs
use pingora_slice::*;

#[tokio::test]
async fn test_end_to_end_flow() {
    // 测试实现
}
```

### 属性测试

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_slice_coverage(
        file_size in 1u64..1000000,
        slice_size in 1usize..10000
    ) {
        let slices = calculate_slices(file_size, slice_size);
        // 验证属性
    }
}
```

## 文档

### 更新文档

如果你的更改影响了用户界面或行为：

1. 更新 README.md
2. 更新相关的文档文件（docs/ 目录）
3. 更新代码注释
4. 如有必要，更新 QUICKSTART.md

### 文档结构

```
docs/
├── API.md                  # API 文档
├── CONFIGURATION.md        # 配置说明
├── DEPLOYMENT.md          # 部署指南
└── PERFORMANCE_TUNING.md  # 性能调优
```

## Pull Request 检查清单

在提交 PR 之前，请确保：

- [ ] 代码通过所有测试 (`cargo test`)
- [ ] 代码通过 clippy 检查 (`cargo clippy`)
- [ ] 代码已格式化 (`cargo fmt`)
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] 提交信息清晰明确
- [ ] PR 描述详细说明了更改内容

## PR 模板

```markdown
## 描述
简要描述这个 PR 的目的和内容

## 相关 Issue
Fixes #123

## 更改类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 文档更新
- [ ] 性能优化
- [ ] 代码重构

## 测试
描述你如何测试这些更改

## 检查清单
- [ ] 代码通过所有测试
- [ ] 代码通过 clippy 检查
- [ ] 代码已格式化
- [ ] 添加了测试
- [ ] 更新了文档

## 截图（如适用）
```

## 发布流程

### 版本号规范

遵循语义化版本 (Semantic Versioning)：

- MAJOR.MINOR.PATCH (例如：1.2.3)
- MAJOR: 不兼容的 API 更改
- MINOR: 向后兼容的功能新增
- PATCH: 向后兼容的问题修正

### 发布步骤

1. 更新 `Cargo.toml` 中的版本号
2. 更新 CHANGELOG.md
3. 创建 git tag
4. 推送 tag 触发 CI/CD

```bash
# 更新版本
vi Cargo.toml

# 提交更改
git add Cargo.toml CHANGELOG.md
git commit -m "chore: bump version to 0.2.0"

# 创建 tag
git tag v0.2.0

# 推送
git push origin main
git push origin v0.2.0
```

## 社区

### 行为准则

- 尊重所有贡献者
- 保持友好和专业
- 接受建设性批评
- 关注对项目最有利的事情

### 获取帮助

- GitHub Issues: 报告问题和讨论
- GitHub Discussions: 一般性讨论和问答
- Pull Requests: 代码审查和讨论

## 许可证

通过贡献代码，你同意你的贡献将在 MIT 许可证下发布。

## 致谢

感谢所有为 Pingora Slice 做出贡献的开发者！

---

如有任何问题，欢迎在 Issues 中提问。

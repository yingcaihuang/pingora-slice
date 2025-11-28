.PHONY: help build test clean install uninstall run fmt lint check release rpm docker

# 默认目标
help:
	@echo "Pingora Slice - Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  build       - Build the project in debug mode"
	@echo "  release     - Build the project in release mode"
	@echo "  test        - Run all tests"
	@echo "  test-unit   - Run unit tests only"
	@echo "  test-int    - Run integration tests only"
	@echo "  test-prop   - Run property tests only"
	@echo "  fmt         - Format code with rustfmt"
	@echo "  lint        - Run clippy linter"
	@echo "  check       - Run fmt and lint checks"
	@echo "  clean       - Clean build artifacts"
	@echo "  install     - Install binary to /usr/local/bin"
	@echo "  uninstall   - Remove installed binary"
	@echo "  run         - Run the server with example config"
	@echo "  rpm         - Build RPM package (requires rpmbuild)"
	@echo "  docker      - Build Docker image"
	@echo "  bench       - Run benchmarks"
	@echo "  doc         - Generate and open documentation"

# 构建
build:
	cargo build

release:
	cargo build --release

# 测试
test:
	cargo test --all

test-unit:
	cargo test --lib --bins

test-int:
	cargo test --test test_integration
	cargo test --test test_config_loading
	cargo test --test test_error_handling
	cargo test --test test_cache_integration
	cargo test --test test_metadata_fetcher
	cargo test --test test_metrics_endpoint

test-prop:
	cargo test --test prop_config_validation
	cargo test --test prop_range_parsing
	cargo test --test prop_slice_coverage
	cargo test --test prop_slice_non_overlapping
	cargo test --test prop_cache_key_uniqueness
	cargo test --test prop_cache_hit_correctness
	cargo test --test prop_partial_cache_hit
	cargo test --test prop_range_header_format
	cargo test --test prop_partial_request_slicing
	cargo test --test prop_request_analysis
	cargo test --test prop_byte_order_preservation
	cargo test --test prop_response_header_completeness
	cargo test --test prop_206_response_format
	cargo test --test prop_4xx_error_passthrough
	cargo test --test prop_invalid_range_error
	cargo test --test prop_content_range_validation
	cargo test --test prop_failure_propagation

# 代码质量
fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt lint
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings

# 清理
clean:
	cargo clean
	rm -rf target/
	rm -f *.rpm
	rm -rf rpmbuild/

# 安装
install: release
	install -m 755 target/release/pingora-slice /usr/local/bin/
	mkdir -p /etc/pingora-slice
	install -m 644 examples/pingora_slice.yaml /etc/pingora-slice/
	@echo "Installed to /usr/local/bin/pingora-slice"
	@echo "Config at /etc/pingora-slice/pingora_slice.yaml"

uninstall:
	rm -f /usr/local/bin/pingora-slice
	@echo "Uninstalled from /usr/local/bin/pingora-slice"

# 运行
run: build
	./target/debug/pingora-slice examples/pingora_slice.yaml

run-release: release
	./target/release/pingora-slice examples/pingora_slice.yaml

# 基准测试
bench:
	cargo build --release --example benchmark
	./target/release/examples/benchmark

# 文档
doc:
	cargo doc --no-deps --open

# RPM 构建
rpm: release
	@echo "Building RPM package..."
	@if ! command -v rpmbuild >/dev/null 2>&1; then \
		echo "Error: rpmbuild not found. Install with: sudo dnf install rpm-build rpmdevtools"; \
		exit 1; \
	fi
	rpmdev-setuptree
	cp packaging/pingora-slice.spec.template ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__VERSION__|0.1.0|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__GITHUB_REPO__|your-username/pingora-slice|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__BINARY_PATH__|$(PWD)/target/release/pingora-slice|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__CONFIG_PATH__|$(PWD)/examples/pingora_slice.yaml|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__README_PATH__|$(PWD)/README.md|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i 's|__README_ZH_PATH__|$(PWD)/README_zh.md|g' ~/rpmbuild/SPECS/pingora-slice.spec
	sed -i "s|__DATE__|$$(date '+%a %b %d %Y')|g" ~/rpmbuild/SPECS/pingora-slice.spec
	rpmbuild -bb ~/rpmbuild/SPECS/pingora-slice.spec
	@echo "RPM built successfully!"
	@echo "Location: ~/rpmbuild/RPMS/x86_64/"
	@ls -lh ~/rpmbuild/RPMS/x86_64/pingora-slice-*.rpm

# Docker
docker:
	docker build -t pingora-slice:latest .

docker-run:
	docker run -p 8080:8080 -p 9091:9091 pingora-slice:latest

# 开发辅助
dev: build
	cargo watch -x 'run -- examples/pingora_slice.yaml'

# 压力测试
stress:
	./scripts/stress_test.sh http://localhost:8080 /test-file

# 代码覆盖率
coverage:
	cargo tarpaulin --out Html --output-dir coverage

# 依赖更新
update:
	cargo update

# 安全审计
audit:
	cargo audit

# 全面检查（CI 使用）
ci: check test
	@echo "All CI checks passed!"

# 发布准备
pre-release: clean check test
	@echo "Ready for release!"
	@echo "Next steps:"
	@echo "1. Update version in Cargo.toml"
	@echo "2. Update CHANGELOG.md"
	@echo "3. git tag v0.x.x"
	@echo "4. git push origin v0.x.x"

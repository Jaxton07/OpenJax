SHELL := /bin/zsh
.DEFAULT_GOAL := help

CARGO ?= cargo
PYTHON ?= python3
PREFIX ?= $(HOME)/.local/openjax
KEEP_USER_DATA ?= 0

export CARGO_NET_RETRY ?= 5
export CARGO_HTTP_MULTIPLEXING ?= false

.PHONY: \
	help doctor prefetch \
	run-tui run-web-dev build-release-mac package-mac build-release-linux package-linux package-windows install-local uninstall-local install-source \
	build-all test-rust clean-dist \
	python-setup python-dev python-test clean-python \
	setup setup-new dev dev-new test test-new lint format clean

help:
	@echo "OpenJax Deployment Commands:"
	@echo ""
	@echo "  Rust 主线（推荐）:"
	@echo "    make doctor            - 检查 cargo/rustup/zsh"
	@echo "    make prefetch          - 预拉取 Rust 依赖 (Cargo.lock)"
	@echo "    make run-tui           - 运行 Rust TUI (tui_next)"
	@echo "    make run-web-dev       - 同时启动 gateway + web 前端开发服务"
	@echo "    make build-release-mac - 构建 macOS ARM release 二进制"
	@echo "    make package-mac       - 打包预编译安装包"
	@echo "    make build-release-linux - 构建 Linux x86_64 release 二进制"
	@echo "    make package-linux     - 打包 Linux x86_64 预编译安装包"
	@echo "    make package-windows   - 打包 Windows x86_64 预编译安装包 (需在 Windows PowerShell 执行)"
	@echo "    make install-local     - 本机安装到 PREFIX (默认 ~/.local/openjax)"
	@echo "    make uninstall-local   - 本机卸载 (默认全清理, KEEP_USER_DATA=1 可保留 userdata)"
	@echo "    make install-source    - 源码安装（本地仓库，一键）(构建 + 安装)"
	@echo ""
	@echo "  校验与清理:"
	@echo "    make build-all         - 构建整个 Rust workspace"
	@echo "    make test-rust         - 运行 Rust workspace 测试"
	@echo "    make clean-dist        - 清理 dist 目录"
	@echo ""
	@echo "  Python (Optional / Deprecated as primary path):"
	@echo "    make python-setup      - 安装 Python SDK + TUI 开发依赖"
	@echo "    make python-dev        - 运行 Python TUI"
	@echo "    make python-test       - 运行 Python TUI 单元测试"
	@echo ""
	@echo "  Deprecated aliases: setup setup-new dev dev-new test test-new"


doctor:
	@command -v zsh >/dev/null || (echo "[doctor] missing zsh" && exit 1)
	@command -v $(CARGO) >/dev/null || (echo "[doctor] missing cargo" && exit 1)
	@command -v rustup >/dev/null || (echo "[doctor] missing rustup" && exit 1)
	@echo "[doctor] OK: zsh cargo rustup"

prefetch:
	$(CARGO) fetch --locked

run-tui:
	$(CARGO) run -q -p tui_next

run-web-dev:
	bash scripts/dev/start_gateway_web.sh

build-release-mac:
	$(CARGO) build --release --locked -p tui_next -p openjax-cli -p openjaxd

package-mac:
	bash scripts/release/package_macos.sh

build-release-linux:
	$(CARGO) build --release --locked -p tui_next -p openjax-cli -p openjaxd

package-linux:
	bash scripts/release/package_linux.sh

package-windows:
	powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1

install-local: package-mac
	bash scripts/release/install.sh --prefix "$(PREFIX)" -y

uninstall-local:
	@if [ "$(KEEP_USER_DATA)" = "1" ]; then \
		bash scripts/release/uninstall.sh --prefix "$(PREFIX)" --keep-user-data; \
	else \
		bash scripts/release/uninstall.sh --prefix "$(PREFIX)"; \
	fi

install-source:
	$(CARGO) build --release --locked -p tui_next -p openjax-cli -p openjaxd
	mkdir -p "$(PREFIX)/bin"
	cp target/release/tui_next "$(PREFIX)/bin/tui_next"
	cp target/release/openjax-cli "$(PREFIX)/bin/openjax-cli"
	cp target/release/openjaxd "$(PREFIX)/bin/openjaxd"
	chmod +x "$(PREFIX)/bin/tui_next" "$(PREFIX)/bin/openjax-cli" "$(PREFIX)/bin/openjaxd"
	@echo "Installed to $(PREFIX)/bin"
	@echo "If needed: export PATH=\"$(PREFIX)/bin:\$$PATH\""

build-all:
	$(CARGO) build --workspace --locked

test-rust:
	$(CARGO) test --workspace

clean-dist:
	rm -rf dist

python-setup:
	python3 -m venv .venv
	.venv/bin/pip install -U pip
	.venv/bin/pip install -e python/openjax_sdk
	.venv/bin/pip install -e python/tui

python-dev:
	PYTHONPATH=python/openjax_sdk/src:python/tui/src \
		.venv/bin/python -m openjax_tui

python-test:
	PYTHONPATH=python/openjax_sdk/src:python/tui/src \
		.venv/bin/python -m unittest discover -s python/tui/tests -v

clean-python:
	rm -rf .venv
	rm -rf python/*/build
	rm -rf python/*/*.egg-info
	find . -type d -name "__pycache__" -prune -exec rm -rf {} +
	find . -type f -name "*.pyc" -delete
	find . -type f -name "*.pyo" -delete

# Deprecated aliases (compat)
setup: python-setup
setup-new: python-setup
dev: python-dev
dev-new: python-dev
test: python-test
test-new: python-test
lint:
	@echo "Deprecated: use cargo clippy --workspace --all-targets -- -D warnings"
format:
	@echo "Deprecated: use cargo fmt -- --check"
clean: clean-python clean-dist

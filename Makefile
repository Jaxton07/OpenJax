SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO ?= cargo
PREFIX ?= $(HOME)/.local/openjax
KEEP_USER_DATA ?= 0

export CARGO_NET_RETRY ?= 5
export CARGO_HTTP_MULTIPLEXING ?= false

.PHONY: \
	help doctor prefetch \
	run-tui run-web-dev build-web-release build-release-mac package-mac build-release-linux package-linux package-windows install-local install-online upgrade-online upgrade-from-package uninstall-local install-source \
	build-all test-rust clean-dist \
	lint format clean \
	core-smoke core-feature-skills core-feature-tools core-feature-streaming core-feature-approval core-feature-history core-full core-baseline \
	gateway-smoke gateway-fast gateway-doc gateway-full gateway-baseline

help:
	@echo "OpenJax Deployment Commands:"
	@echo ""
	@echo "  Rust 主线（推荐）:"
	@echo "    make doctor            - 检查 cargo/rustup/bash (zsh 可选)"
	@echo "    make prefetch          - 预拉取 Rust 依赖 (Cargo.lock)"
	@echo "    make run-tui           - 运行 Rust TUI (tui_next)"
	@echo "    make run-web-dev       - 同时启动 gateway + web 前端开发服务"
	@echo "                            (gateway 默认 cwd: .local-dev-test，可用 OPENJAX_LOCAL_DEV_WORKDIR 覆盖)"
	@echo "    make build-web-release - 构建 web 静态资源 (ui/web/dist)"
	@echo "    make build-release-mac - 构建 macOS ARM release 二进制"
	@echo "    make package-mac       - 打包预编译安装包"
	@echo "    make build-release-linux - 构建 Linux x86_64 release 二进制"
	@echo "    make package-linux     - 打包 Linux x86_64 预编译安装包"
	@echo "    make package-windows   - 打包 Windows x86_64 预编译安装包 (需在 Windows PowerShell 执行)"
	@echo "    make install-local     - 本机安装到 PREFIX (默认 ~/.local/openjax)"
	@echo "    make install-online    - 从 GitHub Release 下载并安装 (macOS ARM / Linux x86_64)"
	@echo "    make upgrade-online    - 从 GitHub Release 在线升级到最新版本"
	@echo "    make upgrade-from-package PKG=<tar.gz> - 使用本地包离线升级"
	@echo "    make uninstall-local   - 本机卸载 (默认全清理, KEEP_USER_DATA=1 可保留 userdata)"
	@echo "    make install-source    - 源码安装（本地仓库，一键）(构建 + 安装)"
	@echo ""
	@echo "  openjax-core 测试入口:"
	@echo "    make core-smoke         - 运行 openjax-core 烟雾测试"
	@echo "    make core-feature-skills - 运行 openjax-core skills 特性测试"
	@echo "    make core-feature-tools  - 运行 openjax-core tools 特性测试"
	@echo "    make core-feature-streaming - 运行 openjax-core streaming 特性测试"
	@echo "    make core-feature-approval - 运行 openjax-core approval 特性测试"
	@echo "    make core-feature-history - 运行 openjax-core history 特性测试"
	@echo "    make core-full          - 运行 openjax-core 全量测试"
	@echo "    make core-baseline      - 运行 openjax-core 冷/热耗时统计与慢测榜"
	@echo ""
	@echo "  openjax-gateway 测试入口:"
	@echo "    make gateway-smoke      - 运行 openjax-gateway 烟雾测试"
	@echo "    make gateway-fast       - 运行 openjax-gateway 快速测试"
	@echo "    make gateway-doc        - 运行 openjax-gateway 文档相关测试"
	@echo "    make gateway-full       - 运行 openjax-gateway 全量测试"
	@echo "    make gateway-baseline   - 运行 openjax-gateway 冷/热耗时统计与慢测榜"
	@echo ""
	@echo "  校验与清理:"
	@echo "    make build-all         - 构建整个 Rust workspace"
	@echo "    make test-rust         - 运行 Rust workspace 测试"
	@echo "    make clean-dist        - 清理 dist 目录"
doctor:
	@command -v bash >/dev/null || (echo "[doctor] missing bash" && exit 1)
	@command -v $(CARGO) >/dev/null || (echo "[doctor] missing cargo" && exit 1)
	@command -v rustup >/dev/null || (echo "[doctor] missing rustup" && exit 1)
	@command -v zsh >/dev/null || echo "[doctor] note: zsh not found (optional)"
	@echo "[doctor] OK: bash cargo rustup"

prefetch:
	$(CARGO) fetch --locked

run-tui:
	$(CARGO) run -q -p tui_next

run-web-dev:
	bash scripts/dev/start_gateway_web.sh

build-web-release:
	@if ! command -v pnpm >/dev/null 2>&1; then \
		echo "[build-web-release] missing pnpm"; \
		exit 1; \
	fi
	@if [ ! -d "ui/web/node_modules" ]; then \
		echo "[build-web-release] ui/web/node_modules not found, running pnpm install..."; \
		(cd ui/web && pnpm install); \
	fi
	(cd ui/web && pnpm build)

build-release-mac:
	$(CARGO) build --release --locked -p tui_next -p openjaxd -p openjax-gateway -p openjax-cli

package-mac:
	$(MAKE) build-web-release
	bash scripts/release/package_macos.sh

build-release-linux:
	$(CARGO) build --release --locked -p tui_next -p openjaxd -p openjax-gateway -p openjax-cli

package-linux:
	$(MAKE) build-web-release
	bash scripts/release/package_linux.sh

package-windows:
	powershell -ExecutionPolicy Bypass -File scripts/release/package_windows.ps1

install-local: build-web-release package-mac
	bash scripts/release/install.sh --prefix "$(PREFIX)" -y

install-online:
	bash scripts/release/install_from_github.sh --prefix "$(PREFIX)"

upgrade-online:
	bash scripts/release/upgrade.sh --prefix "$(PREFIX)" --yes

upgrade-from-package:
	@if [ -z "$(PKG)" ]; then \
		echo "Usage: make upgrade-from-package PKG=/path/to/openjax-vX.Y.Z-<platform>.tar.gz"; \
		exit 1; \
	fi
	bash scripts/release/upgrade.sh --prefix "$(PREFIX)" --from-package "$(PKG)" --yes

uninstall-local:
	@if [ "$(KEEP_USER_DATA)" = "1" ]; then \
		bash scripts/release/uninstall.sh --prefix "$(PREFIX)" --keep-user-data; \
	else \
		bash scripts/release/uninstall.sh --prefix "$(PREFIX)"; \
	fi

install-source:
	$(CARGO) build --release --locked -p tui_next -p openjaxd -p openjax-gateway -p openjax-cli
	@if ! command -v pnpm >/dev/null 2>&1; then \
		echo "[install-source] missing pnpm for web build"; \
		exit 1; \
	fi
	@if [ ! -d "ui/web/node_modules" ]; then \
		echo "[install-source] ui/web/node_modules not found, running pnpm install..."; \
		(cd ui/web && pnpm install); \
	fi
	(cd ui/web && pnpm build)
	mkdir -p "$(PREFIX)/bin"
	mkdir -p "$(PREFIX)/web"
	cp target/release/tui_next "$(PREFIX)/bin/tui_next"
	cp target/release/openjaxd "$(PREFIX)/bin/openjaxd"
	cp target/release/openjax-gateway "$(PREFIX)/bin/openjax-gateway"
	cp target/release/openjax "$(PREFIX)/bin/openjax"
	cp -R ui/web/dist/. "$(PREFIX)/web/"
	chmod +x "$(PREFIX)/bin/tui_next" "$(PREFIX)/bin/openjaxd" "$(PREFIX)/bin/openjax-gateway" "$(PREFIX)/bin/openjax"
	@echo "Installed to $(PREFIX)/bin"
	@echo "Web assets installed to $(PREFIX)/web"
	@echo "If needed: export PATH=\"$(PREFIX)/bin:\$$PATH\""

build-all:
	$(CARGO) build --workspace --locked

test-rust:
	$(CARGO) test --workspace

core-smoke:
	bash scripts/test/core.sh core-smoke

core-feature-skills:
	bash scripts/test/core.sh core-feature-skills

core-feature-tools:
	bash scripts/test/core.sh core-feature-tools

core-feature-streaming:
	bash scripts/test/core.sh core-feature-streaming

core-feature-approval:
	bash scripts/test/core.sh core-feature-approval

core-feature-history:
	bash scripts/test/core.sh core-feature-history

core-full:
	bash scripts/test/core.sh core-full

core-baseline:
	bash scripts/test/core.sh core-baseline

gateway-smoke:
	bash scripts/test/gateway.sh gateway-smoke

gateway-fast:
	bash scripts/test/gateway.sh gateway-fast

gateway-doc:
	bash scripts/test/gateway.sh gateway-doc

gateway-full:
	bash scripts/test/gateway.sh gateway-full

gateway-baseline:
	bash scripts/test/gateway.sh gateway-baseline

clean-dist:
	rm -rf dist

lint:
	@echo "Deprecated: use cargo clippy --workspace --all-targets -- -D warnings"
format:
	@echo "Deprecated: use cargo fmt -- --check"
clean: clean-dist

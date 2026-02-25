# OpenJax Makefile
# 简化开发工作流程

.PHONY: help setup setup-new dev dev-new test test-new clean lint format

# 默认显示帮助
help:
	@echo "OpenJax 开发命令:"
	@echo ""
	@echo "  当前 TUI (prompt_toolkit):"
	@echo "    make setup      - 设置开发环境 (.venv + 安装依赖)"
	@echo "    make dev        - 运行当前 TUI"
	@echo "    make test       - 运行当前 TUI 测试"
	@echo ""
	@echo "  新 TUI (Textual - 开发中):"
	@echo "    make setup-new  - 设置新 TUI 开发环境"
	@echo "    make dev-new    - 运行新 TUI"
	@echo "    make test-new   - 运行新 TUI 测试"
	@echo ""
	@echo "  其他:"
	@echo "    make clean      - 清理虚拟环境和构建产物"
	@echo "    make lint       - 运行代码检查"
	@echo "    make format     - 格式化代码"
	@echo ""

# ============ 当前 TUI (prompt_toolkit) ============

setup:
	@echo "Setting up development environment..."
	python3 -m venv .venv
	.venv/bin/pip install -U pip
	.venv/bin/pip install -e python/openjax_sdk
	.venv/bin/pip install -e python/openjax_tui
	@echo "Done! Run 'make dev' to start."

dev:
	PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
		.venv/bin/python -m openjax_tui

test:
	PYTHONPATH=python/openjax_sdk/src:python/openjax_tui/src \
		.venv/bin/python -m unittest discover -s python/openjax_tui/tests -v

# ============ 新 TUI (Textual) ============

setup-new:
	@echo "Setting up new TUI development environment..."
	python3 -m venv .venv
	.venv/bin/pip install -U pip
	.venv/bin/pip install -e python/openjax_sdk
	.venv/bin/pip install -e python/tui
	@echo "Done! Run 'make dev-new' to start."

dev-new:
	PYTHONPATH=python/openjax_sdk/src:python/tui/src \
		.venv/bin/python -m openjax_tui

test-new:
	PYTHONPATH=python/openjax_sdk/src:python/tui/src \
		.venv/bin/python -m pytest python/tui/tests -v

# ============ 通用命令 ============

clean:
	rm -rf .venv
	rm -rf python/*/build
	rm -rf python/*/*.egg-info
	rm -rf python/*/__pycache__
	rm -rf python/*/*/__pycache__
	find . -type f -name "*.pyc" -delete
	find . -type f -name "*.pyo" -delete

lint:
	.venv/bin/python -m pyright python/openjax_sdk/src
	.venv/bin/python -m pyright python/openjax_tui/src

format:
	.venv/bin/python -m black python/openjax_sdk/src --line-length 100
	.venv/bin/python -m black python/openjax_tui/src --line-length 100

#!/bin/bash
set -euo pipefail

echo "=========================================="
echo "OpenJax apply_patch E2E 测试"
echo "=========================================="

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
CONFIG_FILE="$REPO_ROOT/.openjax/config/config.toml"
CLI_BIN="$REPO_ROOT/target/debug/openjax-cli"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "未找到配置文件: $CONFIG_FILE"
    exit 1
fi

if [ ! -x "$CLI_BIN" ]; then
    echo "未找到可执行文件: $CLI_BIN"
    echo "请先执行: zsh -lc \"cargo build -p openjax-cli\""
    exit 1
fi

WORK_DIR="$(mktemp -d "$SCRIPT_DIR/tmp_apply_patch_e2e.XXXXXX")"
TEST_FILE="$WORK_DIR/test.txt"
LOG_DIR="$WORK_DIR/.openjax/logs"
LOG_FILE="$LOG_DIR/openjax.log"

cleanup() {
    if [ "${KEEP_E2E_ARTIFACTS:-0}" = "1" ]; then
        echo "保留测试目录: $WORK_DIR"
        return
    fi
    rm -rf "$WORK_DIR"
}
trap cleanup EXIT

echo "测试目录: $WORK_DIR"
echo "日志目录: $LOG_DIR"

echo ""
echo "[1/4] 创建测试文件..."
cat >"$TEST_FILE" <<'EOF'
line1
line2
line3
EOF

echo ""
echo "[2/4] 准备输入并执行 CLI..."
INPUT_FILE="$(mktemp "$WORK_DIR/input.XXXXXX.txt")"
cat >"$INPUT_FILE" <<'INPUT_EOF'
tool:apply_patch patch='*** Begin Patch\n*** Update File: test.txt\n@@\n line1\n-line2\n+line2-updated\n line3\n*** End Patch'
/exit
INPUT_EOF

export OPENJAX_LOG_LEVEL=debug
export OPENJAX_APPROVAL_POLICY=never

(
    cd "$WORK_DIR"
    "$CLI_BIN" --config "$CONFIG_FILE" <"$INPUT_FILE" || true
)

rm -f "$INPUT_FILE"

echo ""
echo "[3/4] 检查结果..."
if [ ! -f "$TEST_FILE" ]; then
    echo "失败: 测试文件不存在: $TEST_FILE"
    exit 1
fi

if ! grep -q "line2-updated" "$TEST_FILE"; then
    echo "失败: test.txt 中未找到期望变更 line2-updated"
    echo "=== test.txt ==="
    cat "$TEST_FILE"
    exit 1
fi

if [ ! -f "$LOG_FILE" ]; then
    echo "失败: 未生成日志文件: $LOG_FILE"
    exit 1
fi

if grep -qi "freeform" "$LOG_FILE"; then
    echo "信息: 检测到 freeform 相关日志"
else
    echo "信息: 未检测到 freeform 关键字（当前脚本为直接 tool 调用，属于预期）"
fi

if ! grep -q "tool_name=apply_patch\\|tool start: apply_patch\\|tool done: apply_patch" "$LOG_FILE"; then
    echo "失败: 日志中未检索到 apply_patch 调用"
    echo "=== 日志末尾 ==="
    tail -200 "$LOG_FILE" || true
    exit 1
fi

echo ""
echo "[4/4] 输出摘要..."
echo "=== test.txt ==="
cat "$TEST_FILE"
echo ""
echo "=== 日志末尾(200行) ==="
tail -200 "$LOG_FILE" || true

echo ""
echo "=========================================="
echo "测试通过"
echo "=========================================="

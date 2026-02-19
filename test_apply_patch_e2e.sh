#!/bin/bash
set -e

echo "=========================================="
echo "OpenJax apply_patch 端到端测试"
echo "=========================================="

PROJECT_ROOT="/Users/ericw/work/code/ai/openJax"
TEST_DIR="$PROJECT_ROOT/test_apply_patch_e2e"
LOG_DIR="$TEST_DIR/.openjax/logs"
LOG_FILE="$LOG_DIR/openjax.log"
CONFIG_FILE="$PROJECT_ROOT/.openjax/config/config.toml"

cd "$PROJECT_ROOT"

echo ""
echo "[1/5] 清理测试环境..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
mkdir -p "$LOG_DIR"

cd "$TEST_DIR"

echo "[2/5] 创建测试文件..."
cat > test.txt << 'EOF'
《登岳阳楼》
昔闻洞庭水，今上岳阳楼。
吴楚东南坼，乾坤日夜浮。
亲朋无一字，老病有孤舟。
戎马关山北，凭轩涕泗流。
EOF

echo "测试文件内容:"
cat test.txt
echo ""

echo "[3/5] 清理旧日志..."
rm -f "$LOG_FILE"

echo "[4/5] 运行 CLI 测试 apply_patch..."
echo ""
echo "测试场景: 让 LLM 在 test.txt 中添加李白的《静夜思》"
echo ""

export OPENJAX_LOG_LEVEL=debug
export OPENJAX_APPROVAL_POLICY=never

INPUT_FILE=$(mktemp)
cat > "$INPUT_FILE" << 'INPUT_EOF'
请在 test.txt 文件末尾添加李白的《静夜思》，格式与现有内容保持一致。
/exit
INPUT_EOF

echo "输入内容:"
cat "$INPUT_FILE"
echo ""

echo "使用配置文件: $CONFIG_FILE"
echo "日志文件: $LOG_FILE"
echo ""

echo "----------------------------------------"
echo "开始执行 CLI..."
echo "----------------------------------------"

cd "$TEST_DIR"
"$PROJECT_ROOT/target/debug/openjax-cli" --config "$CONFIG_FILE" < "$INPUT_FILE" 2>&1 || true

rm -f "$INPUT_FILE"

echo ""
echo "----------------------------------------"
echo "执行完成"
echo "----------------------------------------"

echo ""
echo "[5/5] 检查结果..."

echo ""
echo "=== 最终文件内容 ==="
if [ -f test.txt ]; then
    cat test.txt
else
    echo "文件不存在!"
fi

echo ""
echo "=== 日志文件 (最后 200 行) ==="
if [ -f "$LOG_FILE" ]; then
    tail -200 "$LOG_FILE"
else
    echo "日志文件不存在于: $LOG_FILE"
    echo ""
    echo "检查其他可能的位置..."
    for dir in "$PROJECT_ROOT/.openjax/logs" "$HOME/.openjax/logs"; do
        if [ -f "$dir/openjax.log" ]; then
            echo "找到日志文件: $dir/openjax.log"
            tail -200 "$dir/openjax.log"
            break
        fi
    done
fi

echo ""
echo "=== 检查 Freeform 模式 ==="
if grep -q "Freeform\|freeform" "$LOG_FILE" 2>/dev/null; then
    echo "✓ 检测到 Freeform 相关日志"
    grep -i "freeform" "$LOG_FILE" || true
else
    echo "⚠ 未检测到 Freeform 关键字（可能需要检查工具规范）"
fi

echo ""
echo "=== 检查 apply_patch 调用 ==="
if grep -q "apply_patch" "$LOG_FILE" 2>/dev/null; then
    echo "✓ 检测到 apply_patch 调用"
    grep "apply_patch" "$LOG_FILE" | tail -30 || true
else
    echo "⚠ 未检测到 apply_patch 调用"
fi

echo ""
echo "=== 检查模型决策 ==="
if grep -q "model_decision\|model_raw_output" "$LOG_FILE" 2>/dev/null; then
    echo "✓ 检测到模型决策日志"
    grep -E "model_decision|model_raw_output" "$LOG_FILE" | tail -20 || true
else
    echo "⚠ 未检测到模型决策日志"
fi

echo ""
echo "=== 检查参数解析 ==="
if grep -q "parsing arguments\|parsed arguments" "$LOG_FILE" 2>/dev/null; then
    echo "✓ 检测到参数解析日志"
    grep -E "parsing arguments|parsed arguments" "$LOG_FILE" | tail -20 || true
else
    echo "⚠ 未检测到参数解析日志"
fi

echo ""
echo "=========================================="
echo "测试完成"
echo "=========================================="

cd "$PROJECT_ROOT"

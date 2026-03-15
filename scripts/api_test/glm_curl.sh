#!/usr/bin/env zsh
set -euo pipefail

ROOT_DIR="/Users/ericw/work/code/ai/openJax"
LOG_FILE="${ROOT_DIR}/scripts/api_test/response.log"
TMP_HEADERS="$(mktemp)"
TMP_BODY="$(mktemp)"
TMP_PAYLOAD="$(mktemp)"
trap 'rm -f "$TMP_HEADERS" "$TMP_BODY" "$TMP_PAYLOAD"' EXIT

API_KEY="${GLM_API_KEY:-${OPENJAX_GLM_API_KEY:-}}"
if [[ -z "$API_KEY" ]]; then
  echo "Missing API key. Set GLM_API_KEY or OPENJAX_GLM_API_KEY." >&2
  exit 1
fi

MODEL="${GLM_MODEL:-glm-5}"
ENDPOINT="${GLM_ENDPOINT:-https://open.bigmodel.cn/api/coding/paas/v4/chat/completions}"
PROMPT="${1:-你好，请介绍一下自己。}"
STREAM_FLAG="${GLM_STREAM:-true}"
PROMPT_ESCAPED="$(printf '%s' "$PROMPT" | sed 's/\\/\\\\/g; s/"/\\"/g')"

cat > "$TMP_PAYLOAD" <<EOF
{
  "model": "${MODEL}",
  "messages": [
    { "role": "system", "content": "你是一个有用的AI助手。" },
    { "role": "user", "content": "${PROMPT_ESCAPED}" }
  ],
  "temperature": 1.0,
  "stream": ${STREAM_FLAG}
}
EOF

{
  echo "=== GLM API TEST ==="
  echo "timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "endpoint: ${ENDPOINT}"
  echo "model: ${MODEL}"
  echo "stream: ${STREAM_FLAG}"
  echo "prompt: ${PROMPT}"
  echo "--- request payload ---"
  cat "$TMP_PAYLOAD"
  echo "--- response headers ---"
} > "$LOG_FILE"

HTTP_CODE="$(
  curl -sS -N \
    -D "$TMP_HEADERS" \
    -o "$TMP_BODY" \
    -w "%{http_code}" \
    -X POST "$ENDPOINT" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${API_KEY}" \
    --data-binary "@${TMP_PAYLOAD}"
)"

{
  cat "$TMP_HEADERS"
  echo "--- response body ---"
  cat "$TMP_BODY"
  echo
  echo "--- summary ---"
  echo "http_code: ${HTTP_CODE}"
  echo "body_bytes: $(wc -c < "$TMP_BODY" | tr -d ' ')"
} >> "$LOG_FILE"

echo "Wrote response to ${LOG_FILE}"
echo "HTTP ${HTTP_CODE}"

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_HOME="${ROOT_DIR}/.tmp/glm_chat_home"
CFG_DIR="${TMP_HOME}/.openjax"
CFG_FILE="${CFG_DIR}/config.toml"
ORIGINAL_HOME="${HOME}"

if [[ -z "${OPENJAX_GLM_API_KEY:-}" ]]; then
  echo "OPENJAX_GLM_API_KEY is required."
  echo "Example:"
  echo "  OPENJAX_GLM_API_KEY=xxx ${BASH_SOURCE[0]}"
  exit 1
fi

mkdir -p "${CFG_DIR}"

cat > "${CFG_FILE}" <<'EOF'
[model.routing]
planner = "glm_chat"
final_writer = "glm_chat"
tool_reasoning = "glm_chat"

[model.routing.fallbacks]
glm_chat = []

[model.models.glm_chat]
provider = "glm"
protocol = "chat_completions"
model = "glm-4.7-flash"
base_url = "https://open.bigmodel.cn/api/paas/v4"
api_key_env = "OPENJAX_GLM_API_KEY"
supports_stream = true
supports_reasoning = false
supports_tool_call = false
supports_json_mode = false

[sandbox]
mode = "workspace_write"
approval_policy = "on_request"

[agent]
max_tool_calls_per_turn = 10
max_planner_rounds_per_turn = 20
EOF

echo "Using isolated HOME=${TMP_HOME}"
echo "Using config ${CFG_FILE}"
echo "OPENJAX_DIRECT_PROVIDER_STREAM=${OPENJAX_DIRECT_PROVIDER_STREAM:-1}"

export HOME="${TMP_HOME}"
export OPENJAX_DIRECT_PROVIDER_STREAM="${OPENJAX_DIRECT_PROVIDER_STREAM:-1}"
# Keep Rust/Cargo cache in the real home to avoid re-downloading crates/toolchains.
export CARGO_HOME="${CARGO_HOME:-${ORIGINAL_HOME}/.cargo}"
export RUSTUP_HOME="${RUSTUP_HOME:-${ORIGINAL_HOME}/.rustup}"

cd "${ROOT_DIR}"
exec cargo run -p openjax-gateway

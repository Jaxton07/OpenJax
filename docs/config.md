
export OPENJAX_MINIMAX_API_KEY="sk-cp-QQkdVRLS6yKxlYj8jSUH_-XxtulUiM1OocxENTWuiStI6-N6OwIK6KhbIoRacapyaUw5nn9J52Jlwts1CxP6AnOOCPGAG8p6RkifjfCGNtt56BclvlokVfo"
export OPENJAX_MINIMAX_BASE_URL="https://api.minimaxi.com/v1"
export OPENJAX_MINIMAX_MODEL="codex-MiniMax-M2.1"

export OPENJAX_MINIMAX_BASE_URL="https://api.minimaxi.com/anthropic/v1"
export OPENJAX_MINIMAX_MODEL="MiniMax-M2.5"



export OPENJAX_APPROVAL_POLICY=always_ask   # or on_request / never
export OPENJAX_SANDBOX_MODE=workspace_write # or danger_full_access


# 方式1: 项目级配置 (推荐开发时使用)
cp openjax-cli/config.toml.example .openjax.toml
# 编辑 .openjax.toml 填入 api_key

# 方式2: 全局配置
mkdir -p ~/.openjax && cp openjax-cli/config.toml.example ~/.openjax/config.toml

# 方式3: 指定配置文件
openjax-cli --config /path/to/config.toml

# 临时覆盖: 环境变量优先
OPENAI_API_KEY="temp-key" openjax-cli
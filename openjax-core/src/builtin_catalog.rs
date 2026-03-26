/// 静态内置 LLM provider 目录。
/// 目录数据仅在前端配置阶段使用（渲染下拉、预填表单）；
/// 一旦用户配置写入 DB，运行时直接读 DB，不依赖此模块。
#[derive(Debug, Clone)]
pub struct CatalogModel {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub context_window: u32,
}

#[derive(Debug, Clone)]
pub struct CatalogProvider {
    pub catalog_key: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    /// "chat_completions" | "anthropic_messages"
    pub protocol: &'static str,
    pub request_profile: Option<&'static str>,
    pub default_model: &'static str,
    pub models: &'static [CatalogModel],
}

pub static BUILTIN_CATALOG: &[CatalogProvider] = &[
    CatalogProvider {
        catalog_key: "openai",
        display_name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        protocol: "chat_completions",
        request_profile: None,
        default_model: "gpt-5.3-codex",
        models: &[
            CatalogModel {
                model_id: "gpt-5.3-codex",
                display_name: "GPT-5.3 Codex",
                context_window: 200000,
            },
            CatalogModel {
                model_id: "gpt-5.4",
                display_name: "GPT-5.4",
                context_window: 200000,
            },
            CatalogModel {
                model_id: "gpt-4o",
                display_name: "GPT-4o",
                context_window: 128000,
            },
            CatalogModel {
                model_id: "gpt-4o-mini",
                display_name: "GPT-4o mini",
                context_window: 128000,
            },
            CatalogModel {
                model_id: "gpt-4.1",
                display_name: "GPT-4.1",
                context_window: 1047576,
            },
            CatalogModel {
                model_id: "gpt-4.1-mini",
                display_name: "GPT-4.1 mini",
                context_window: 1047576,
            },
        ],
    },
    CatalogProvider {
        catalog_key: "anthropic",
        display_name: "Claude (Anthropic)",
        base_url: "https://api.anthropic.com",
        protocol: "anthropic_messages",
        request_profile: None,
        default_model: "claude-sonnet-4-6",
        models: &[
            CatalogModel {
                model_id: "claude-opus-4-6",
                display_name: "Claude Opus 4.6",
                context_window: 200000,
            },
            CatalogModel {
                model_id: "claude-sonnet-4-6",
                display_name: "Claude Sonnet 4.6",
                context_window: 200000,
            },
            CatalogModel {
                model_id: "claude-haiku-4-5",
                display_name: "Claude Haiku 4.5",
                context_window: 200000,
            },
        ],
    },
    CatalogProvider {
        catalog_key: "glm_coding",
        display_name: "GLM Coding",
        base_url: "https://open.bigmodel.cn/api/anthropic",
        protocol: "anthropic_messages",
        request_profile: None,
        default_model: "glm-4.7",
        models: &[CatalogModel {
            model_id: "glm-4.7",
            display_name: "GLM-4.7",
            context_window: 200000,
        }],
    },
    CatalogProvider {
        catalog_key: "kimi_coding",
        display_name: "Kimi Coding",
        base_url: "https://api.kimi.com/coding/v1",
        protocol: "anthropic_messages",
        request_profile: None,
        default_model: "kimi-for-coding",
        models: &[CatalogModel {
            model_id: "kimi-for-coding",
            display_name: "Kimi for Coding",
            context_window: 200000,
        }],
    },
    CatalogProvider {
        catalog_key: "minimax_coding",
        display_name: "MiniMax Coding",
        base_url: "https://api.minimax.com/anthropic/v1",
        protocol: "anthropic_messages",
        request_profile: None,
        default_model: "MiniMax-M2.7",
        models: &[CatalogModel {
            model_id: "MiniMax-M2.7",
            display_name: "MiniMax M2.7",
            context_window: 200000,
        }],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_non_empty() {
        assert!(!BUILTIN_CATALOG.is_empty());
    }

    #[test]
    fn each_provider_has_default_model_in_list() {
        for provider in BUILTIN_CATALOG {
            let found = provider
                .models
                .iter()
                .any(|m| m.model_id == provider.default_model);
            assert!(
                found,
                "Provider '{}' default_model '{}' not found in models list",
                provider.catalog_key, provider.default_model
            );
        }
    }

    #[test]
    fn all_context_windows_positive() {
        for provider in BUILTIN_CATALOG {
            for model in provider.models {
                assert!(
                    model.context_window > 0,
                    "Model '{}' has zero context_window",
                    model.model_id
                );
            }
        }
    }
}

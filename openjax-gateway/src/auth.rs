use std::collections::HashSet;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    GatewayEnv,
    CompatEnv,
    Generated,
}

#[derive(Debug, Clone)]
pub struct ApiKeyConfig {
    pub keys: HashSet<String>,
    pub source: ApiKeySource,
    pub generated_key: Option<String>,
}

pub fn load_api_keys() -> ApiKeyConfig {
    let gateway_env = std::env::var("OPENJAX_GATEWAY_API_KEYS").ok();
    let compat_env = std::env::var("OPENJAX_API_KEYS").ok();
    resolve_api_keys(gateway_env.as_deref(), compat_env.as_deref())
}

pub fn load_api_keys_from_env() -> HashSet<String> {
    load_api_keys().keys
}

pub fn parse_bearer_token(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let prefix = "Bearer ";
    if !trimmed.starts_with(prefix) {
        return None;
    }
    let token = trimmed[prefix.len()..].trim();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn resolve_api_keys(gateway_env: Option<&str>, compat_env: Option<&str>) -> ApiKeyConfig {
    if let Some(raw) = gateway_env {
        let keys = parse_api_keys(raw);
        if !keys.is_empty() {
            return ApiKeyConfig {
                keys,
                source: ApiKeySource::GatewayEnv,
                generated_key: None,
            };
        }
    }

    if let Some(raw) = compat_env {
        let keys = parse_api_keys(raw);
        if !keys.is_empty() {
            return ApiKeyConfig {
                keys,
                source: ApiKeySource::CompatEnv,
                generated_key: None,
            };
        }
    }

    let generated_key = format!("ojx_{}", Uuid::new_v4().simple());
    let mut keys = HashSet::new();
    keys.insert(generated_key.clone());
    ApiKeyConfig {
        keys,
        source: ApiKeySource::Generated,
        generated_key: Some(generated_key),
    }
}

fn parse_api_keys(raw: &str) -> HashSet<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ApiKeySource, parse_bearer_token, resolve_api_keys};

    #[test]
    fn uses_gateway_env_keys_when_present() {
        let config = resolve_api_keys(Some("k1,k2"), Some("k3"));
        assert_eq!(config.source, ApiKeySource::GatewayEnv);
        assert!(config.generated_key.is_none());
        assert!(config.keys.contains("k1"));
        assert!(config.keys.contains("k2"));
        assert!(!config.keys.contains("k3"));
    }

    #[test]
    fn falls_back_to_compat_env_when_gateway_env_is_empty() {
        let config = resolve_api_keys(Some(" , "), Some("legacy-key"));
        assert_eq!(config.source, ApiKeySource::CompatEnv);
        assert!(config.generated_key.is_none());
        assert!(config.keys.contains("legacy-key"));
    }

    #[test]
    fn generates_key_when_no_env_key_is_available() {
        let config = resolve_api_keys(None, None);
        assert_eq!(config.source, ApiKeySource::Generated);
        let generated_key = config.generated_key.expect("generated key");
        assert!(generated_key.starts_with("ojx_"));
        assert!(config.keys.contains(&generated_key));
    }

    #[test]
    fn parse_bearer_token_rejects_empty_token() {
        assert!(parse_bearer_token("Bearer ").is_none());
        assert_eq!(parse_bearer_token("Bearer abc"), Some("abc"));
    }
}

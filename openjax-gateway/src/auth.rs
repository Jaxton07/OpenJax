use std::collections::HashSet;

pub fn load_api_keys_from_env() -> HashSet<String> {
    std::env::var("OPENJAX_GATEWAY_API_KEYS")
        .or_else(|_| std::env::var("OPENJAX_API_KEYS"))
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
        .collect()
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

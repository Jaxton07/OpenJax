const REFRESH_COOKIE_NAME: &str = "ojx_refresh_token";

pub fn parse_cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    cookie_header.split(';').map(str::trim).find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k.trim() == name {
            Some(v.trim().to_string())
        } else {
            None
        }
    })
}

pub fn refresh_cookie_name() -> &'static str {
    REFRESH_COOKIE_NAME
}

pub fn build_refresh_set_cookie(token: &str, secure: bool, max_age_seconds: i64) -> String {
    let mut parts = vec![
        format!("{}={}", REFRESH_COOKIE_NAME, token),
        "Path=/api/v1/auth".to_string(),
        format!("Max-Age={}", max_age_seconds),
        "HttpOnly".to_string(),
        "SameSite=Lax".to_string(),
    ];
    if secure {
        parts.push("Secure".to_string());
    }
    parts.join("; ")
}

pub fn build_refresh_clear_cookie(secure: bool) -> String {
    let mut parts = vec![
        format!("{}=", REFRESH_COOKIE_NAME),
        "Path=/api/v1/auth".to_string(),
        "Max-Age=0".to_string(),
        "HttpOnly".to_string(),
        "SameSite=Lax".to_string(),
    ];
    if secure {
        parts.push("Secure".to_string());
    }
    parts.join("; ")
}

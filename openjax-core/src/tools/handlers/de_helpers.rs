use serde::Deserialize;

// ── 内部辅助 ──────────────────────────────────────────────────────────────────

/// 将 serde_json::Value 中的数字或字符串解析为 u64。
/// 返回 None 表示类型不匹配（既不是数字也不是字符串）。
fn value_to_u64<E: serde::de::Error>(value: &serde_json::Value) -> Result<u64, E> {
    if let Some(num) = value.as_u64() {
        return Ok(num);
    }
    if let Some(text) = value.as_str() {
        return text
            .parse::<u64>()
            .map_err(|_| E::custom("expected non-negative integer"));
    }
    Err(E::custom("expected integer"))
}

// ── 公共反序列化函数 ───────────────────────────────────────────────────────────

/// 同时接受 JSON 数字和字符串形式的 usize。
/// 如 `"offset": 1` 和 `"offset": "1"` 均合法。
/// 注意：字段缺失由调用处的 `#[serde(default)]` 处理，本函数只在字段存在时调用。
pub fn de_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value_to_u64(&value).map(|n| n as usize)
}

/// 同时接受 JSON 数字和字符串形式的 u64。
/// 如 `"timeout_ms": 5000` 和 `"timeout_ms": "5000"` 均合法。
/// 注意：字段缺失由调用处的 `#[serde(default)]` 处理，本函数只在字段存在时调用。
pub fn de_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    value_to_u64(&value)
}

/// 同时接受 null / JSON 数字 / 字符串形式的 Option<usize>。
/// `null` → None；`1` 或 `"1"` → Some(1)。
/// 注意：字段缺失由调用处的 `#[serde(default)]` 处理，本函数只在字段存在时调用。
pub fn de_opt_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() {
        return Ok(None);
    }
    value_to_u64(&value).map(|n| Some(n as usize))
}

/// 同时接受 JSON bool、数字（0/1）和字符串（"true"/"false"/"yes"/"1" 等）形式的 bool。
///
/// - JSON bool：`true` → true，`false` → false
/// - JSON 数字：非零 → true，`0` → false
/// - JSON 字符串：`"true"` / `"yes"` / `"1"` → true；其余（含无法识别的字符串）→ false（宽松语义，有意设计）
///
/// 注意：字段缺失由调用处的 `#[serde(default)]` 处理，本函数只在字段存在时调用。
pub fn de_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(flag) = value.as_bool() {
        return Ok(flag);
    }
    if let Some(n) = value.as_u64() {
        return Ok(n != 0);
    }
    if let Some(n) = value.as_i64() {
        return Ok(n != 0);
    }
    if let Some(text) = value.as_str() {
        return Ok(matches!(
            text.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ));
    }
    Ok(false)
}

// ── 单元测试 ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    // de_usize

    #[test]
    fn de_usize_accepts_json_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_usize")]
            v: usize,
        }
        let s: S = serde_json::from_str(r#"{"v": 42}"#).unwrap();
        assert_eq!(s.v, 42);
    }

    #[test]
    fn de_usize_accepts_string_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_usize")]
            v: usize,
        }
        let s: S = serde_json::from_str(r#"{"v": "42"}"#).unwrap();
        assert_eq!(s.v, 42);
    }

    #[test]
    fn de_usize_rejects_negative_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_usize")]
            v: usize,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": "-1"}"#).is_err());
    }

    #[test]
    fn de_usize_rejects_non_numeric_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_usize")]
            v: usize,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": "abc"}"#).is_err());
    }

    #[test]
    fn de_usize_rejects_empty_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_usize")]
            v: usize,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": ""}"#).is_err());
    }

    // de_u64

    #[test]
    fn de_u64_accepts_json_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_u64")]
            v: u64,
        }
        let s: S = serde_json::from_str(r#"{"v": 30000}"#).unwrap();
        assert_eq!(s.v, 30000);
    }

    #[test]
    fn de_u64_accepts_string_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_u64")]
            v: u64,
        }
        let s: S = serde_json::from_str(r#"{"v": "30000"}"#).unwrap();
        assert_eq!(s.v, 30000);
    }

    #[test]
    fn de_u64_rejects_negative_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_u64")]
            v: u64,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": "-1"}"#).is_err());
    }

    // de_opt_usize

    #[test]
    fn de_opt_usize_accepts_json_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(default, deserialize_with = "de_opt_usize")]
            v: Option<usize>,
        }
        let s: S = serde_json::from_str(r#"{"v": 7}"#).unwrap();
        assert_eq!(s.v, Some(7));
    }

    #[test]
    fn de_opt_usize_accepts_string_number() {
        #[derive(Deserialize)]
        struct S {
            #[serde(default, deserialize_with = "de_opt_usize")]
            v: Option<usize>,
        }
        let s: S = serde_json::from_str(r#"{"v": "7"}"#).unwrap();
        assert_eq!(s.v, Some(7));
    }

    #[test]
    fn de_opt_usize_accepts_null() {
        #[derive(Deserialize)]
        struct S {
            #[serde(default, deserialize_with = "de_opt_usize")]
            v: Option<usize>,
        }
        let s: S = serde_json::from_str(r#"{"v": null}"#).unwrap();
        assert_eq!(s.v, None);
    }

    #[test]
    fn de_opt_usize_absent_field_is_none() {
        #[derive(Deserialize)]
        struct S {
            #[serde(default, deserialize_with = "de_opt_usize")]
            v: Option<usize>,
        }
        let s: S = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(s.v, None);
    }

    // de_bool

    #[test]
    fn de_bool_accepts_json_bool() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_bool")]
            v: bool,
        }
        let s: S = serde_json::from_str(r#"{"v": true}"#).unwrap();
        assert!(s.v);
    }

    #[test]
    fn de_bool_accepts_string_true() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_bool")]
            v: bool,
        }
        for input in &[r#"{"v": "true"}"#, r#"{"v": "yes"}"#, r#"{"v": "1"}"#] {
            let s: S = serde_json::from_str(input).unwrap();
            assert!(s.v, "expected true for input {input}");
        }
    }

    #[test]
    fn de_bool_accepts_string_false() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_bool")]
            v: bool,
        }
        for input in &[r#"{"v": "false"}"#, r#"{"v": "no"}"#, r#"{"v": "0"}"#] {
            let s: S = serde_json::from_str(input).unwrap();
            assert!(!s.v, "expected false for input {input}");
        }
    }

    #[test]
    fn de_bool_accepts_json_integer_one_as_true() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_bool")]
            v: bool,
        }
        let s: S = serde_json::from_str(r#"{"v": 1}"#).unwrap();
        assert!(s.v, "integer 1 should be true");
    }

    #[test]
    fn de_bool_accepts_json_integer_zero_as_false() {
        #[derive(Deserialize)]
        struct S {
            #[serde(deserialize_with = "de_bool")]
            v: bool,
        }
        let s: S = serde_json::from_str(r#"{"v": 0}"#).unwrap();
        assert!(!s.v, "integer 0 should be false");
    }

    #[test]
    fn de_u64_rejects_non_numeric_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_u64")]
            v: u64,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": "abc"}"#).is_err());
    }

    #[test]
    fn de_u64_rejects_empty_string() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct S {
            #[serde(deserialize_with = "de_u64")]
            v: u64,
        }
        assert!(serde_json::from_str::<S>(r#"{"v": ""}"#).is_err());
    }
}

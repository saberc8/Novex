use serde_json::Value;

pub(crate) fn json_i64_field(value: &Value, keys: &[&str]) -> Option<i64> {
    json_field(value, keys).and_then(json_i64)
}

pub(crate) fn json_f64_field(value: &Value, keys: &[&str]) -> Option<f64> {
    json_field(value, keys).and_then(json_f64)
}

pub(crate) fn json_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    json_field(value, keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub(crate) fn json_bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    json_field(value, keys).and_then(json_bool)
}

fn json_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(value) = object.get(*key) {
            return Some(value);
        }
    }

    let normalized_keys = keys
        .iter()
        .map(|key| normalize_json_key(key))
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        normalized_keys
            .iter()
            .any(|expected| *expected == normalize_json_key(key))
            .then_some(value)
    })
}

fn normalize_json_key(key: &str) -> String {
    key.chars()
        .filter(|ch| !matches!(ch, '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn json_i64(value: &Value) -> Option<i64> {
    let parsed = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str()?.trim().parse::<i64>().ok())?;
    Some(parsed.max(0))
}

fn json_f64(value: &Value) -> Option<f64> {
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    parsed.is_finite().then_some(parsed.max(0.0))
}

fn json_bool(value: &Value) -> Option<bool> {
    if let Some(value) = value.as_bool() {
        return Some(value);
    }
    if let Some(value) = value.as_i64() {
        return Some(value > 0);
    }
    if let Some(value) = value.as_u64() {
        return Some(value > 0);
    }

    match value.as_str()?.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "enabled" | "allow" | "allowed" => Some(true),
        "false" | "0" | "no" | "n" | "disabled" | "deny" | "denied" => Some(false),
        _ => None,
    }
}

pub(crate) fn normalize_registry_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

pub(crate) fn non_negative(value: i64) -> f64 {
    value.max(0) as f64
}

pub(crate) fn normalize_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_owned()
}

pub(crate) fn join_url(base_url: &str, path: &str) -> String {
    let base_url = normalize_base_url(base_url);
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        base_url
    } else {
        format!("{base_url}/{path}")
    }
}

pub fn mask_api_key(api_key: &str) -> String {
    let chars = api_key.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "****".to_owned();
    }

    let prefix = chars.iter().take(3).collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len().saturating_sub(4))
        .collect::<String>();
    format!("{prefix}****{suffix}")
}

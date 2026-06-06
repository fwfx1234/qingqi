use std::collections::HashMap;

use crate::model::ApiVariable;

/// Resolve `{{key}}` and `${key}` patterns in text.
///
/// Precedence (highest to lowest):
/// 1. Magic values ($timestamp, $uuid, etc.)
/// 2. temporary vars (from pre-ops `set key=value`)
/// 3. env_vars (current environment's variable map)
/// 4. environment store (DB-persisted env-scoped vars)
/// 5. module_vars
/// 6. module store
/// 7. globals_map
///
/// If no match, the original `{{key}}` text is left unresolved.
pub fn resolve_text(
    text: &str,
    temporary: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
    env_store: &[ApiVariable],
    module_vars: &HashMap<String, String>,
    module_store: &[ApiVariable],
    globals_map: &HashMap<String, String>,
    global_store: &[ApiVariable],
) -> String {
    let mut result = text.to_string();

    // Collect all {{key}} and ${key} patterns
    let keys = extract_template_keys(text);
    for key in keys {
        let value = resolve_key(
            &key,
            temporary,
            env_vars,
            env_store,
            module_vars,
            module_store,
            globals_map,
            global_store,
        );
        if let Some(val) = value {
            result = result.replace(&format!("{{{{{key}}}}}"), &val);
            result = result.replace(&format!("${{{key}}}"), &val);
            result = result.replace(&format!("${key}"), &val);
        }
    }

    result
}

/// Resolve a single key through the precedence chain.
pub fn resolve_key(
    key: &str,
    temporary: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
    env_store: &[ApiVariable],
    module_vars: &HashMap<String, String>,
    module_store: &[ApiVariable],
    globals_map: &HashMap<String, String>,
    global_store: &[ApiVariable],
) -> Option<String> {
    // 1. Magic values
    if let Some(val) = magic_value(key) {
        return Some(val);
    }

    // 2. temporary (from pre-ops)
    if let Some(val) = temporary.get(key) {
        return Some(val.clone());
    }

    // 3. env_vars (current environment's inline variables)
    if let Some(val) = env_vars.get(key) {
        return Some(val.clone());
    }

    // 4. environment store (DB-persisted)
    if let Some(var) = env_store
        .iter()
        .find(|v| v.var_key == key && !v.var_value.is_empty())
    {
        return Some(var.var_value.clone());
    }

    // 5. module_vars
    if let Some(val) = module_vars.get(key) {
        return Some(val.clone());
    }

    // 6. module store
    if let Some(var) = module_store
        .iter()
        .find(|v| v.var_key == key && !v.var_value.is_empty())
    {
        return Some(var.var_value.clone());
    }

    // 7. globals_map
    if let Some(val) = globals_map.get(key) {
        return Some(val.clone());
    }

    // 8. global store
    if let Some(var) = global_store
        .iter()
        .find(|v| v.var_key == key && !v.var_value.is_empty())
    {
        return Some(var.var_value.clone());
    }

    None
}

/// Current wall-clock time, preferring local offset and falling back to UTC.
fn local_now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc())
}

/// Return magic value for built-in dynamic variables.
pub fn magic_value(key: &str) -> Option<String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    match key {
        "timestamp" => Some(now.as_secs().to_string()),
        "timestamp_ms" => Some(now.as_millis().to_string()),
        "iso_datetime" => {
            let t = time::OffsetDateTime::now_utc();
            Some(format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                t.year(),
                u8::from(t.month()),
                t.day(),
                t.hour(),
                t.minute(),
                t.second(),
            ))
        }
        "date" => {
            let t = local_now();
            Some(format!(
                "{:04}-{:02}-{:02}",
                t.year(),
                u8::from(t.month()),
                t.day(),
            ))
        }
        "uuid" => {
            let bytes = generate_uuid_bytes();
            Some(format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                bytes[0],
                bytes[1],
                bytes[2],
                bytes[3],
                bytes[4],
                bytes[5],
                bytes[6],
                bytes[7],
                bytes[8],
                bytes[9],
                bytes[10],
                bytes[11],
                bytes[12],
                bytes[13],
                bytes[14],
                bytes[15],
            ))
        }
        "uuid_simple" => {
            let bytes = generate_uuid_bytes();
            Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
        }
        "time" => {
            let t = local_now();
            Some(format!(
                "{:02}:{:02}:{:02}",
                t.hour(),
                t.minute(),
                t.second(),
            ))
        }
        "datetime" => {
            let t = local_now();
            Some(format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                t.year(),
                u8::from(t.month()),
                t.day(),
                t.hour(),
                t.minute(),
                t.second(),
            ))
        }
        "year" => Some(local_now().year().to_string()),
        "month" => Some(u8::from(local_now().month()).to_string()),
        "day" => Some(local_now().day().to_string()),
        "random_int" => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            now.as_nanos().hash(&mut hasher);
            Some((hasher.finish() % 1_000_000).to_string())
        }
        "random_4" => Some(random_digits(4)),
        "random_6" => Some(random_digits(6)),
        "random_8" => Some(random_digits(8)),
        "random_bool" => {
            let val = now.as_nanos() % 2 == 0;
            Some(if val { "true" } else { "false" }.to_string())
        }
        "random_string" => Some(random_alphanumeric(12)),
        "random_hex" => Some(random_hex(16)),
        "random_email" => Some(format!("{}@example.com", random_alphanumeric(8))),
        "base64_random" => Some(base64_encode(&random_alphanumeric(18))),
        _ => None,
    }
}

/// Extract all template keys from text (both `{{key}}` and `${key}` patterns).
fn extract_template_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();

    // Match {{key}}
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next(); // consume second {
                let mut key = String::new();
                while let Some(kc) = chars.next() {
                    if kc == '}' {
                        if chars.peek() == Some(&'}') {
                            chars.next();
                            let trimmed = key.trim().to_string();
                            if !trimmed.is_empty() && !keys.contains(&trimmed) {
                                keys.push(trimmed);
                            }
                            break;
                        }
                    }
                    key.push(kc);
                }
            }
        }
    }

    // Match ${key} - simple alphanumeric key
    let mut chars = text.chars().enumerate().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '$' {
            let rest = &text[i + 1..];
            if rest.starts_with('{') {
                // ${key} pattern
                if let Some(end) = rest.find('}') {
                    let key = rest[1..end].trim().to_string();
                    if !key.is_empty() && !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            } else {
                // $key pattern (alphanumeric + underscore)
                let key: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !key.is_empty() && !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }
    }

    keys
}

fn generate_uuid_bytes() -> [u8; 16] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    let hash = hasher.finish();

    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hash.to_le_bytes());
    let mut hasher2 = DefaultHasher::new();
    (hash.wrapping_add(1)).hash(&mut hasher2);
    bytes[8..].copy_from_slice(&hasher2.finish().to_le_bytes());
    // Set version 4
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    // Set variant
    bytes[7] = (bytes[7] & 0x3f) | 0x80;
    bytes
}

fn random_digits(len: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    let mut val = hasher.finish();
    let mut result = String::new();
    for _ in 0..len {
        result.push(char::from_digit((val % 10) as u32, 10).unwrap_or('0'));
        val /= 10;
    }
    result
}

fn random_alphanumeric(len: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    let mut val = hasher.finish();
    let mut result = String::new();
    for _ in 0..len {
        result.push(CHARS[(val % CHARS.len() as u64) as usize] as char);
        val /= CHARS.len() as u64;
    }
    result
}

fn random_hex(len: usize) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.as_nanos().hash(&mut hasher);
    let mut val = hasher.finish();
    let mut result = String::new();
    for _ in 0..len {
        result.push(char::from_digit((val % 16) as u32, 16).unwrap_or('0'));
        val /= 16;
    }
    result
}

fn base64_encode(input: &str) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        result.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Resolve a mapping (both keys and values).
pub fn resolve_mapping(
    values: &HashMap<String, String>,
    temporary: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
    env_store: &[ApiVariable],
    module_vars: &HashMap<String, String>,
    module_store: &[ApiVariable],
    globals_map: &HashMap<String, String>,
    global_store: &[ApiVariable],
) -> HashMap<String, String> {
    values
        .iter()
        .map(|(k, v)| {
            let resolved_key = resolve_text(
                k,
                temporary,
                env_vars,
                env_store,
                module_vars,
                module_store,
                globals_map,
                global_store,
            );
            let resolved_val = resolve_text(
                v,
                temporary,
                env_vars,
                env_store,
                module_vars,
                module_store,
                globals_map,
                global_store,
            );
            (resolved_key, resolved_val)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_vars() -> HashMap<String, String> {
        HashMap::new()
    }
    fn empty_store() -> Vec<ApiVariable> {
        Vec::new()
    }

    #[test]
    fn resolves_magic_timestamp() {
        let result = resolve_text(
            "{{timestamp}}",
            &empty_vars(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert!(!result.is_empty());
        // Should be a number
        assert!(result.parse::<u64>().is_ok());
    }

    #[test]
    fn resolves_magic_uuid() {
        let result = resolve_text(
            "{{uuid}}",
            &empty_vars(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert!(result.contains('-'));
        assert_eq!(result.len(), 36);
    }

    #[test]
    fn resolves_temporary_over_env() {
        let mut temporary = HashMap::new();
        temporary.insert("TOKEN".into(), "temp-value".into());
        let mut env_vars = HashMap::new();
        env_vars.insert("TOKEN".into(), "env-value".into());

        let result = resolve_text(
            "Bearer {{TOKEN}}",
            &temporary,
            &env_vars,
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "Bearer temp-value");
    }

    #[test]
    fn resolves_env_over_store() {
        let mut env_vars = HashMap::new();
        env_vars.insert("DB_HOST".into(), "inline-host".into());
        let env_store = vec![ApiVariable {
            scope: crate::model::VariableScope::Environment,
            env_name: "dev".into(),
            var_key: "DB_HOST".into(),
            var_value: "store-host".into(),
            updated_at: String::new(),
        }];

        let result = resolve_text(
            "{{DB_HOST}}",
            &empty_vars(),
            &env_vars,
            &env_store,
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "inline-host");
    }

    #[test]
    fn resolves_store_when_no_inline() {
        let env_store = vec![ApiVariable {
            scope: crate::model::VariableScope::Environment,
            env_name: "dev".into(),
            var_key: "DB_HOST".into(),
            var_value: "store-host".into(),
            updated_at: String::new(),
        }];

        let result = resolve_text(
            "{{DB_HOST}}",
            &empty_vars(),
            &empty_vars(),
            &env_store,
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "store-host");
    }

    #[test]
    fn leaves_unresolved_key() {
        let result = resolve_text(
            "{{MISSING_KEY}}",
            &empty_vars(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "{{MISSING_KEY}}");
    }

    #[test]
    fn resolves_multiple_patterns() {
        let mut env_vars = HashMap::new();
        env_vars.insert("HOST".into(), "localhost".into());
        env_vars.insert("PORT".into(), "8080".into());

        let result = resolve_text(
            "http://{{HOST}}:{{PORT}}/api",
            &empty_vars(),
            &env_vars,
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "http://localhost:8080/api");
    }

    #[test]
    fn resolves_curly_brace_syntax() {
        let mut env_vars = HashMap::new();
        env_vars.insert("API_KEY".into(), "secret123".into());

        let result = resolve_text(
            "key=${API_KEY}",
            &empty_vars(),
            &env_vars,
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result, "key=secret123");
    }

    #[test]
    fn magic_value_date_format() {
        let val = magic_value("date").unwrap();
        // Should be YYYY-MM-DD format
        assert_eq!(val.len(), 10);
        assert!(val.contains('-'));
    }

    #[test]
    fn magic_value_dates_use_real_calendar() {
        // Year/month/day should reflect a real, current-era calendar date,
        // not the old 365-day/30-day approximation.
        let year: i32 = magic_value("year").unwrap().parse().unwrap();
        assert!(year >= 2025, "year {year} should be current era");
        let month: u8 = magic_value("month").unwrap().parse().unwrap();
        assert!((1..=12).contains(&month), "month {month} out of range");
        let day: u8 = magic_value("day").unwrap().parse().unwrap();
        assert!((1..=31).contains(&day), "day {day} out of range");
        // date == year-month-day
        let date = magic_value("date").unwrap();
        assert_eq!(date, format!("{year:04}-{month:02}-{day:02}"));
    }

    #[test]
    fn magic_value_random_int_range() {
        for _ in 0..20 {
            let val = magic_value("random_int").unwrap();
            let num: u64 = val.parse().unwrap();
            assert!(num < 1_000_000);
        }
    }

    #[test]
    fn magic_value_random_bool() {
        let val = magic_value("random_bool").unwrap();
        assert!(val == "true" || val == "false");
    }

    #[test]
    fn magic_value_random_string_length() {
        let val = magic_value("random_string").unwrap();
        assert_eq!(val.len(), 12);
        assert!(val.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn extract_template_keys_finds_both_syntaxes() {
        let keys = extract_template_keys("http://{{HOST}}:{{PORT}}/${PATH}");
        assert!(keys.contains(&"HOST".to_string()));
        assert!(keys.contains(&"PORT".to_string()));
        assert!(keys.contains(&"PATH".to_string()));
    }

    #[test]
    fn resolve_mapping_resolves_both() {
        let mut env_vars = HashMap::new();
        env_vars.insert("KEY".into(), "my-key".into());
        env_vars.insert("VAL".into(), "my-val".into());

        let mut input = HashMap::new();
        input.insert("{{KEY}}".into(), "{{VAL}}".into());

        let result = resolve_mapping(
            &input,
            &empty_vars(),
            &env_vars,
            &empty_store(),
            &empty_vars(),
            &empty_store(),
            &empty_vars(),
            &empty_store(),
        );
        assert_eq!(result.get("my-key").unwrap(), "my-val");
    }
}

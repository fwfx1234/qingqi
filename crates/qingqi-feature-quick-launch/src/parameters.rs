use std::{collections::HashMap, error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSpec {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingParameterError {
    pub missing: Vec<String>,
}

impl MissingParameterError {
    fn new(missing: Vec<String>) -> Self {
        Self { missing }
    }
}

impl fmt::Display for MissingParameterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "缺少参数: {}", self.missing.join(", "))
    }
}

impl Error for MissingParameterError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellParseError {
    message: String,
}

impl ShellParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ShellParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ShellParseError {}

pub fn extract_parameters<I, S>(texts: I) -> Vec<ParameterSpec>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen: Vec<String> = Vec::new();

    for text in texts {
        let bytes = text.as_ref().as_bytes();
        let mut index = 0;
        while index + 2 < bytes.len() {
            if bytes[index] != b'$' || bytes[index + 1] != b'{' {
                index += 1;
                continue;
            }

            let start = index + 2;
            let mut cursor = start;
            while cursor < bytes.len() && bytes[cursor] != b'}' {
                cursor += 1;
            }

            if cursor >= bytes.len() {
                break;
            }

            let candidate = &text.as_ref()[start..cursor];
            if is_valid_parameter_name(candidate) && !seen.iter().any(|name| name == candidate) {
                seen.push(candidate.to_string());
            }

            index = cursor + 1;
        }
    }

    seen.into_iter()
        .map(|name| ParameterSpec { name })
        .collect()
}

pub fn substitute(
    text: &str,
    values: &HashMap<String, String>,
    quote: bool,
    strict: bool,
) -> Result<String, MissingParameterError> {
    if text.is_empty() {
        return Ok(String::new());
    }

    let mut output = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut missing = Vec::new();

    while index < bytes.len() {
        if index + 2 < bytes.len() && bytes[index] == b'$' && bytes[index + 1] == b'{' {
            let start = index + 2;
            let mut cursor = start;
            while cursor < bytes.len() && bytes[cursor] != b'}' {
                cursor += 1;
            }

            if cursor < bytes.len() {
                let candidate = &text[start..cursor];
                if is_valid_parameter_name(candidate) {
                    if let Some(value) = values.get(candidate) {
                        if quote {
                            let resolved = shell_quote(value);
                            output.push_str(&resolved);
                        } else {
                            output.push_str(value);
                        }
                    } else if strict {
                        if !missing.iter().any(|name| name == candidate) {
                            missing.push(candidate.to_string());
                        }
                        output.push_str(&text[index..=cursor]);
                    }
                    index = cursor + 1;
                    continue;
                }
            }
        }

        let next_len = text[index..]
            .chars()
            .next()
            .map(|ch| ch.len_utf8())
            .unwrap_or(1);
        output.push_str(&text[index..index + next_len]);
        index += next_len;
    }

    if strict && !missing.is_empty() {
        return Err(MissingParameterError::new(missing));
    }

    Ok(output)
}

pub fn substitute_vec(
    values: &[String],
    params: &HashMap<String, String>,
    quote: bool,
    strict: bool,
) -> Result<Vec<String>, MissingParameterError> {
    values
        .iter()
        .map(|value| substitute(value, params, quote, strict))
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn substitute_mapping(
    mapping: &HashMap<String, String>,
    values: &HashMap<String, String>,
    quote: bool,
    strict: bool,
) -> Result<HashMap<String, String>, MissingParameterError> {
    mapping
        .iter()
        .map(|(key, value)| {
            substitute(value, values, quote, strict).map(|resolved| (key.clone(), resolved))
        })
        .collect()
}

pub fn split_shell_words(text: &str) -> Result<Vec<String>, ShellParseError> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;
    let mut started = false;

    while let Some(ch) = chars.next() {
        if escape {
            current.push(ch);
            started = true;
            escape = false;
            continue;
        }

        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            started = true;
            continue;
        }

        if in_double {
            match ch {
                '"' => {
                    in_double = false;
                }
                '\\' => {
                    let Some(next) = chars.next() else {
                        return Err(ShellParseError::new("双引号中的转义不完整"));
                    };
                    current.push(next);
                }
                _ => current.push(ch),
            }
            started = true;
            continue;
        }

        match ch {
            '\\' => {
                escape = true;
                started = true;
            }
            '\'' => {
                in_single = true;
                started = true;
            }
            '"' => {
                in_double = true;
                started = true;
            }
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(ch);
                started = true;
            }
        }
    }

    if escape {
        return Err(ShellParseError::new("末尾转义不完整"));
    }
    if in_single {
        return Err(ShellParseError::new("缺少单引号结束符"));
    }
    if in_double {
        return Err(ShellParseError::new("缺少双引号结束符"));
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

pub fn join_shell_words(values: &[String]) -> String {
    values
        .iter()
        .map(|value| shell_quote(value))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_valid_parameter_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return String::from("''");
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "_@%+=:,./-".contains(ch))
    {
        return value.to_string();
    }

    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_parameters_in_order() {
        let specs = extract_parameters([
            "echo ${name}",
            "open https://example.com/${name}",
            "${team}/${name}",
            "${1bad}",
        ]);
        assert_eq!(
            specs,
            vec![
                ParameterSpec {
                    name: String::from("name")
                },
                ParameterSpec {
                    name: String::from("team")
                }
            ]
        );
    }

    #[test]
    fn substitutes_with_shell_quoting() {
        let values = HashMap::from([(String::from("name"), String::from("Jane Doe"))]);
        let result =
            substitute("echo ${name}", &values, true, true).expect("substitution should succeed");
        assert_eq!(result, "echo 'Jane Doe'");
    }

    #[test]
    fn reports_missing_parameters() {
        let values = HashMap::new();
        let error = substitute("echo ${name} ${team}", &values, false, true)
            .expect_err("substitution should fail");
        assert_eq!(
            error.missing,
            vec![String::from("name"), String::from("team")]
        );
    }

    #[test]
    fn substitutes_mapping_and_vector() {
        let values = HashMap::from([(String::from("user"), String::from("codex"))]);
        let mapping = HashMap::from([(String::from("url"), String::from("https://x/${user}"))]);
        let args = vec![String::from("--user=${user}")];

        let resolved_mapping =
            substitute_mapping(&mapping, &values, false, true).expect("mapping should resolve");
        let resolved_args =
            substitute_vec(&args, &values, false, true).expect("args should resolve");

        assert_eq!(
            resolved_mapping.get("url"),
            Some(&String::from("https://x/codex"))
        );
        assert_eq!(resolved_args, vec![String::from("--user=codex")]);
    }

    #[test]
    fn splits_shell_words_with_quotes() {
        let words = split_shell_words("python3 -m app \"hello world\" 'two words'")
            .expect("split should succeed");
        assert_eq!(
            words,
            vec![
                String::from("python3"),
                String::from("-m"),
                String::from("app"),
                String::from("hello world"),
                String::from("two words")
            ]
        );
    }

    #[test]
    fn reports_unclosed_quotes() {
        let error = split_shell_words("\"oops").expect_err("split should fail");
        assert!(error.to_string().contains("缺少双引号"));
    }
}

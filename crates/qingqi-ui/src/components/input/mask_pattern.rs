//! MaskPattern — text formatting patterns for input fields.

use gpui::SharedString;

#[derive(Clone, PartialEq, Debug)]
pub enum MaskToken {
    Digit,
    Letter,
    LetterOrDigit,
    Sep(char),
    Any,
}

impl MaskToken {
    pub fn is_any(&self) -> bool {
        matches!(self, MaskToken::Any)
    }

    fn is_match(&self, ch: char) -> bool {
        match self {
            MaskToken::Digit => ch.is_ascii_digit(),
            MaskToken::Letter => ch.is_ascii_alphabetic(),
            MaskToken::LetterOrDigit => ch.is_ascii_alphanumeric(),
            MaskToken::Any => true,
            MaskToken::Sep(c) => *c == ch,
        }
    }

    fn is_sep(&self) -> bool {
        matches!(self, MaskToken::Sep(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, MaskToken::Digit)
    }

    pub fn placeholder(&self) -> char {
        match self {
            MaskToken::Sep(c) => *c,
            _ => '_',
        }
    }

    fn unmask_char(&self, ch: char) -> Option<char> {
        match self {
            MaskToken::Digit | MaskToken::Letter | MaskToken::LetterOrDigit | MaskToken::Any => {
                Some(ch)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
pub enum MaskPattern {
    #[default]
    None,
    Pattern {
        pattern: SharedString,
        tokens: Vec<MaskToken>,
    },
    Number {
        separator: Option<char>,
        fraction: Option<usize>,
    },
}

impl From<&str> for MaskPattern {
    fn from(pattern: &str) -> Self {
        Self::new(pattern)
    }
}

impl MaskPattern {
    pub fn new(pattern: &str) -> Self {
        let tokens = pattern
            .chars()
            .map(|ch| match ch {
                '9' => MaskToken::Digit,
                'A' => MaskToken::Letter,
                '#' => MaskToken::LetterOrDigit,
                '*' => MaskToken::Any,
                _ => MaskToken::Sep(ch),
            })
            .collect();

        Self::Pattern {
            pattern: pattern.to_owned().into(),
            tokens,
        }
    }

    pub fn pattern(&self) -> Option<&SharedString> {
        match self {
            Self::Pattern { pattern, .. } => Some(pattern),
            _ => None,
        }
    }

    pub fn tokens(&self) -> Option<&Vec<MaskToken>> {
        match self {
            Self::Pattern { tokens, .. } => Some(tokens),
            _ => None,
        }
    }

    pub fn number(sep: Option<char>) -> Self {
        Self::Number {
            separator: sep,
            fraction: None,
        }
    }

    pub fn number_with_fraction(sep: Option<char>, fraction: usize) -> Self {
        Self::Number {
            separator: sep,
            fraction: Some(fraction),
        }
    }

    pub fn placeholder(&self) -> Option<String> {
        match self {
            Self::Pattern { tokens, .. } => Some(tokens.iter().map(|t| t.placeholder()).collect()),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            Self::Pattern { tokens, .. } => tokens.is_empty(),
            Self::Number { .. } => false,
            Self::None => true,
        }
    }

    pub fn is_valid(&self, mask_text: &str) -> bool {
        match self {
            Self::None => true,
            Self::Pattern { tokens, .. } => {
                let chars: Vec<char> = mask_text.chars().collect();
                let mut text_index = 0;
                for token in tokens {
                    if text_index >= chars.len() {
                        break;
                    }
                    let ch = chars[text_index];
                    if token.is_sep() {
                        if ch != token.placeholder() {
                            return false;
                        }
                        text_index += 1;
                    } else if token.is_match(ch) {
                        text_index += 1;
                    } else {
                        return false;
                    }
                }
                text_index >= chars.len()
            }
            Self::Number { .. } => {
                let mut has_dot = false;
                for (i, ch) in mask_text.chars().enumerate() {
                    match ch {
                        '0'..='9' => {}
                        '-' | '+' => {
                            if i != 0 {
                                return false;
                            }
                        }
                        '.' => {
                            if has_dot {
                                return false;
                            }
                            has_dot = true;
                        }
                        ',' | ' ' => {}
                        _ => return false,
                    }
                }
                true
            }
        }
    }

    pub fn mask(&self, text: &str) -> String {
        match self {
            Self::None => text.to_string(),
            Self::Pattern { tokens, .. } => {
                let chars: Vec<char> = text.chars().collect();
                let mut result = String::new();
                let mut text_index = 0;
                for token in tokens {
                    if token.is_sep() {
                        if text_index == 0 || text_index >= chars.len() {
                            break;
                        }
                        result.push(token.placeholder());
                        if chars.get(text_index) == Some(&token.placeholder()) {
                            text_index += 1;
                        }
                    } else {
                        while chars.get(text_index).is_some_and(|ch| {
                            tokens.iter().any(|candidate| {
                                candidate.is_sep() && candidate.placeholder() == *ch
                            })
                        }) {
                            text_index += 1;
                        }
                        let Some(&ch) = chars.get(text_index) else {
                            break;
                        };
                        if token.is_match(ch) {
                            result.push(ch);
                        }
                        text_index += 1;
                    }
                }
                result
            }
            Self::Number {
                separator,
                fraction,
            } => {
                let cleaned: String = text
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
                    .collect();
                let is_negative = cleaned.starts_with('-');
                let is_positive = cleaned.starts_with('+');
                let num_str: String = cleaned
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.')
                    .collect();

                let parts: Vec<&str> = num_str.split('.').collect();
                let int_part = parts.first().unwrap_or(&"").to_string();
                let frac_part = parts.get(1);

                let mut result = String::new();
                if is_negative {
                    result.push('-');
                } else if is_positive {
                    result.push('+');
                }

                if let Some(sep) = separator {
                    let chars: Vec<char> = int_part.chars().collect();
                    let len = chars.len();
                    for (i, ch) in chars.iter().enumerate() {
                        if i > 0 && (len - i) % 3 == 0 {
                            result.push(*sep);
                        }
                        result.push(*ch);
                    }
                } else {
                    result.push_str(&int_part);
                }

                if let Some(frac) = frac_part {
                    result.push('.');
                    let frac_chars: Vec<char> = frac.chars().collect();
                    let frac_limit = fraction.unwrap_or(frac_chars.len());
                    for ch in frac_chars.iter().take(frac_limit) {
                        result.push(*ch);
                    }
                } else if fraction.is_some() && text.contains('.') {
                    result.push('.');
                }
                result
            }
        }
    }

    pub fn unmask(&self, masked_text: &str) -> String {
        match self {
            Self::None => masked_text.to_string(),
            Self::Pattern { tokens, .. } => {
                let chars: Vec<char> = masked_text.chars().collect();
                let mut result = String::new();
                let mut token_index = 0;
                for ch in chars {
                    if token_index >= tokens.len() {
                        break;
                    }
                    let token = &tokens[token_index];
                    if token.is_sep() {
                        if ch == token.placeholder() {
                            token_index += 1;
                        }
                    } else if let Some(raw_ch) = token.unmask_char(ch) {
                        result.push(raw_ch);
                        token_index += 1;
                    }
                }
                result
            }
            Self::Number { .. } => masked_text
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
                .collect(),
        }
    }

    pub fn max_length(&self) -> Option<usize> {
        match self {
            Self::Pattern { tokens, .. } => Some(tokens.len()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_inserts_separator_without_consuming_input() {
        let pattern = MaskPattern::new("99-99");
        assert_eq!(pattern.mask("1"), "1");
        assert_eq!(pattern.mask("12"), "12");
        assert_eq!(pattern.mask("123"), "12-3");
        assert_eq!(pattern.mask("12-34"), "12-34");
        assert_eq!(pattern.mask("1234"), "12-34");
        assert_eq!(pattern.unmask("12-34"), "1234");
    }
}

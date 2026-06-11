//! 客户端解析终端 `cd` 命令，用于「跟随终端」。

/// 从当前输入行解析 `cd` 目标路径；非 cd 命令返回 None。
pub fn resolve_cd_command(command: &str, current_dir: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let rest = command.strip_prefix("cd")?;
    let arg = rest.trim();
    if arg.is_empty() {
        return Some("~".into());
    }
    if arg == "-" {
        return None;
    }

    let arg = strip_outer_quotes(arg);
    if arg.starts_with('/') {
        return Some(normalize_remote_path(arg));
    }
    if arg == "~" || arg.starts_with("~/") {
        return Some(arg.to_string());
    }

    let base = if current_dir.is_empty() { "/" } else { current_dir };
    Some(normalize_remote_path(&join_remote_path(base, arg)))
}

fn strip_outer_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        if (bytes[0] == b'\'' && bytes[s.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[s.len() - 1] == b'"')
        {
            return &s[1..s.len() - 1];
        }
    }
    s
}

fn join_remote_path(base: &str, rel: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        rel.to_string()
    } else {
        format!("{base}/{rel}")
    }
}

fn normalize_remote_path(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            segment => stack.push(segment),
        }
    }
    if absolute {
        if stack.is_empty() {
            "/".into()
        } else {
            format!("/{}", stack.join("/"))
        }
    } else if stack.is_empty() {
        ".".into()
    } else {
        stack.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_cd_command;

    #[test]
    fn relative_cd() {
        assert_eq!(
            resolve_cd_command("cd logs", "/home/finance"),
            Some("/home/finance/logs".into())
        );
        assert_eq!(
            resolve_cd_command("cd backup", "/home/finance/logs"),
            Some("/home/finance/logs/backup".into())
        );
    }

    #[test]
    fn absolute_and_parent_cd() {
        assert_eq!(
            resolve_cd_command("cd /var/log", "/home/finance"),
            Some("/var/log".into())
        );
        assert_eq!(
            resolve_cd_command("cd ..", "/home/finance/logs"),
            Some("/home/finance".into())
        );
    }

    #[test]
    fn ignores_non_cd() {
        assert_eq!(resolve_cd_command("ls -la", "/home/finance"), None);
    }
}

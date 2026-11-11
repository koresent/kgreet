use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Wayland,
    X11,
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: String,
    pub name: String,
    pub exec: Vec<String>,
    pub kind: SessionKind,
}

pub fn discover_sessions(default_session: &str) -> Vec<SessionEntry> {
    let search_dirs = [
        ("/usr/share/wayland-sessions", SessionKind::Wayland),
        ("/usr/local/share/wayland-sessions", SessionKind::Wayland),
        ("/usr/share/xsessions", SessionKind::X11),
    ];

    let mut sessions = Vec::new();
    for (dir, kind) in search_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(session) = parse_desktop_file(&path, kind)
                && !sessions.iter().any(|s: &SessionEntry| s.id == session.id)
            {
                sessions.push(session);
            }
        }
    }

    sessions.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    if let Some(pos) = find_default_index(&sessions, default_session) {
        let preferred = sessions.remove(pos);
        sessions.insert(0, preferred);
    } else if !default_session.trim().is_empty() {
        let cmd = split_command(default_session);
        if !cmd.is_empty() {
            sessions.insert(
                0,
                SessionEntry {
                    id: default_session.to_string(),
                    name: default_session.to_string(),
                    exec: cmd,
                    kind: SessionKind::Wayland,
                },
            );
        }
    }

    sessions
}

fn find_default_index(sessions: &[SessionEntry], target: &str) -> Option<usize> {
    let target_lower = target.trim().to_lowercase();
    if target_lower.is_empty() {
        return None;
    }
    sessions
        .iter()
        .position(|s| s.id.to_lowercase() == target_lower || s.name.to_lowercase() == target_lower)
}

fn parse_desktop_file(path: &Path, kind: SessionKind) -> Option<SessionEntry> {
    let content = fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_str()?.to_string();

    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec_raw: Option<String> = None;
    let mut hidden = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            match key.trim() {
                "Name" if name.is_none() => name = Some(val.trim().to_string()),
                "Exec" => exec_raw = Some(val.trim().to_string()),
                "Hidden" | "NoDisplay" if val.trim().eq_ignore_ascii_case("true") => hidden = true,
                _ => {}
            }
        }
    }

    if hidden {
        return None;
    }

    let exec = split_command(exec_raw.as_deref()?);
    if exec.is_empty() {
        return None;
    }

    Some(SessionEntry {
        id: id.clone(),
        name: name.unwrap_or(id),
        exec,
        kind,
    })
}

pub fn split_command(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in raw.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_session_commands() {
        let cmd = split_command(r#"/usr/bin/umbriel --config "/etc/my dir/cfg.toml""#);
        assert_eq!(
            cmd,
            vec!["/usr/bin/umbriel", "--config", "/etc/my dir/cfg.toml"]
        );
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::UserConfig;

#[derive(Debug, Clone)]
pub struct UserEntry {
    pub username: String,
    pub display_name: String,
    pub avatar_path: Option<PathBuf>,
}

pub fn discover_users(cfg: &UserConfig) -> Vec<UserEntry> {
    let Ok(passwd) = fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };

    let mut users = Vec::new();
    for line in passwd.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(':').collect();
        if cols.len() < 7 {
            continue;
        }
        let Ok(uid) = cols[2].parse::<u32>() else {
            continue;
        };
        if !(1000..65534).contains(&uid) {
            continue;
        }
        let shell = cols[6];
        if shell.ends_with("/nologin")
            || shell.ends_with("/false")
            || shell.ends_with("/sync")
            || shell.ends_with("/halt")
            || shell.ends_with("/shutdown")
        {
            continue;
        }

        let username = cols[0].to_string();
        let gecos = cols[4].split(',').next().unwrap_or("").trim();
        let home = PathBuf::from(cols[5]);
        let (acct_name, acct_icon) = read_accounts_service(&username);

        let display_name = resolve_display_name(cfg, &username, acct_name.as_deref(), gecos);
        let avatar_path = resolve_avatar_path(&username, &home, acct_icon.as_deref());

        users.push(UserEntry {
            username,
            display_name,
            avatar_path,
        });
    }

    // Move configured default user to index 0 so initial state matches preference.
    if let Some(default_user) = &cfg.default
        && let Some(pos) = users.iter().position(|u| &u.username == default_user)
    {
        let entry = users.remove(pos);
        users.insert(0, entry);
    }

    users
}

fn resolve_display_name(
    cfg: &UserConfig,
    username: &str,
    acct_real_name: Option<&str>,
    gecos: &str,
) -> String {
    if !cfg.show_display_name {
        return username.to_string();
    }
    if let Some(override_name) = &cfg.display_name {
        let trimmed = override_name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(real_name) = acct_real_name {
        let trimmed = real_name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if !gecos.is_empty() {
        return gecos.to_string();
    }
    username.to_string()
}

fn resolve_avatar_path(
    username: &str,
    home: &Path,
    acct_icon: Option<&str>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(icon) = acct_icon {
        candidates.push(PathBuf::from(icon));
    }
    candidates.push(PathBuf::from(format!("/var/lib/AccountsService/icons/{username}")));
    candidates.push(home.join(".face"));
    candidates.push(home.join(".local/share/icons/user-avatar.png"));

    candidates
        .into_iter()
        .find(|path| path.is_file() && fs::File::open(path).is_ok())
}

fn read_accounts_service(username: &str) -> (Option<String>, Option<String>) {
    let path = PathBuf::from(format!("/var/lib/AccountsService/users/{username}"));
    let Ok(content) = fs::read_to_string(path) else {
        return (None, None);
    };

    let mut real_name = None;
    let mut icon = None;
    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("RealName=") {
            if !val.trim().is_empty() {
                real_name = Some(val.trim().to_string());
            }
        } else if let Some(val) = line.strip_prefix("Icon=")
            && !val.trim().is_empty()
        {
            icon = Some(val.trim().to_string());
        }
    }
    (real_name, icon)
}

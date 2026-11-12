use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub user: UserConfig,
    pub session: SessionConfig,
    pub monitor: MonitorConfig,
    pub card: CardConfig,
    pub input: InputConfig,
    pub appearance: AppearanceConfig,
    pub wallpaper: WallpaperConfig,
    pub clock: ClockConfig,
    pub power: PowerConfig,
    pub keyboard: KeyboardConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UserConfig {
    pub default: Option<String>,
    pub show_display_name: bool,
    pub display_name: Option<String>,
    pub show_username_subtitle: bool,
    pub allow_user_switch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub default: String,
    pub show_selector: bool,
    pub extra_env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MonitorConfig {
    pub target: String,
    pub secondary_mode: SecondaryMonitorMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecondaryMonitorMode {
    None,
    Black,
    Wallpaper,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CardConfig {
    pub background_color: String,
    pub opacity: f64,
    pub border_color: String,
    pub border_opacity: f64,
    pub border_radius: u32,
    pub width: i32,
    pub padding_x: i32,
    pub padding_y: i32,
    pub show_avatar: bool,
    pub avatar_size: i32,
    pub avatar_border_width: u32,
    pub avatar_border_color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    pub background_color: String,
    pub opacity: f64,
    pub border_color: String,
    pub border_opacity: f64,
    pub focus_border_color: Option<String>,
    pub border_radius: u32,
    pub height: i32,
    pub dot_char: String,
    pub dot_spacing: u32,
    pub placeholder: String,
    pub show_peek_icon: bool,
    pub cursor_glide: CursorGlideConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CursorGlideConfig {
    pub enabled: bool,
    pub duration_ms: u32,
    pub width: f64,
    pub corner_radius: f64,
    pub trail_smear: f64,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub font_family: String,
    pub text_color: String,
    pub muted_color: String,
    pub accent_color: String,
    pub accent_text_color: String,
    pub error_color: String,
    pub backdrop_dim: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WallpaperConfig {
    pub path: Option<PathBuf>,
    pub mode: WallpaperMode,
    pub fill_mode: FillMode,
    pub blur_sigma: f32,
    pub rotate_interval_secs: u64,
    pub video_fps: u32,
    pub video_loop: bool,
    pub solid_color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperMode {
    Auto,
    Image,
    Video,
    Directory,
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillMode {
    Cover,
    Contain,
    Fill,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PowerConfig {
    pub enabled: bool,
    pub position: PowerPosition,
    pub compact_icons: bool,
    pub button_size: i32,
    pub background_color: Option<String>,
    pub opacity: f64,
    pub show_suspend: bool,
    pub show_reboot: bool,
    pub show_poweroff: bool,
    pub require_confirm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PowerPosition {
    BottomCenter,
    BottomLeft,
    BottomRight,
    TopCenter,
    TopLeft,
    TopRight,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ClockConfig {
    pub enabled: bool,
    pub position: String,
    pub time_format: String,
    pub date_format: String,
    pub subtitle: Option<String>,
    pub time_font_size: f64,
    pub date_font_size: f64,
    pub subtitle_font_size: f64,
    pub time_color: Option<String>,
    pub date_color: Option<String>,
    pub subtitle_color: Option<String>,
    pub time_bold: bool,
}

impl ClockConfig {
    pub fn resolve_subtitle(&self, username: &str, session: &str) -> Option<String> {
        let raw = self.subtitle.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        let mut res = raw.to_string();
        if res.contains("{os}") {
            res = res.replace("{os}", &read_os_name());
        }
        if res.contains("{session}") {
            res = res.replace("{session}", session);
        }
        if res.contains("{host}") {
            res = res.replace("{host}", &read_hostname());
        }
        if res.contains("{user}") {
            res = res.replace("{user}", username);
        }
        Some(res)
    }
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: "top-center".to_string(),
            time_format: "%H:%M".to_string(),
            date_format: "%A, %d %B".to_string(),
            subtitle: None,
            time_font_size: 54.0,
            date_font_size: 18.0,
            subtitle_font_size: 13.0,
            time_color: None,
            date_color: None,
            subtitle_color: None,
            time_bold: false,
        }
    }
}

fn read_os_name() -> String {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for key in ["PRETTY_NAME=", "NAME="] {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix(key) {
                    let clean = val.trim().trim_matches('"').trim_matches('\'');
                    if !clean.is_empty() {
                        return clean.to_string();
                    }
                }
            }
        }
    }
    "Linux".to_string()
}

fn read_hostname() -> String {
    if let Ok(h) = fs::read_to_string("/etc/hostname") {
        let trimmed = h.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "localhost".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeyboardConfig {
    pub show_capslock_warning: bool,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            default: None,
            show_display_name: true,
            display_name: None,
            show_username_subtitle: false,
            allow_user_switch: false,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default: "umbriel".to_string(),
            show_selector: false,
            extra_env: HashMap::new(),
        }
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            target: "primary".to_string(),
            secondary_mode: SecondaryMonitorMode::None,
        }
    }
}

impl Default for CardConfig {
    fn default() -> Self {
        Self {
            background_color: "#282828".to_string(),
            opacity: 0.82,
            border_color: "#928374".to_string(),
            border_opacity: 0.45,
            border_radius: 18,
            width: 410,
            padding_x: 26,
            padding_y: 24,
            show_avatar: true,
            avatar_size: 96,
            avatar_border_width: 2,
            avatar_border_color: None,
        }
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            background_color: "#1d2021".to_string(),
            opacity: 0.38,
            border_color: "#928374".to_string(),
            border_opacity: 0.45,
            focus_border_color: None,
            border_radius: 999,
            height: 42,
            // Bullet point (U+2022) + spacing prevents monospace glyphs from touching.
            dot_char: "\u{2022}".to_string(),
            dot_spacing: 2,
            placeholder: "Enter password".to_string(),
            show_peek_icon: true,
            cursor_glide: CursorGlideConfig::default(),
        }
    }
}

impl Default for CursorGlideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: 150,
            width: 3.0,
            corner_radius: 1.5,
            trail_smear: 0.85,
            color: Some("#d4be98".to_string()),
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            font_family: "Adwaita Sans".to_string(),
            text_color: "#ebdbb2".to_string(),
            muted_color: "#bdae93".to_string(),
            accent_color: "#b8bb26".to_string(),
            accent_text_color: "#1d2021".to_string(),
            error_color: "#fb4934".to_string(),
            backdrop_dim: 0.12,
        }
    }
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        let default_path = PathBuf::from("/usr/share/backgrounds/greeter-wallpaper.jpg");
        Self {
            path: default_path.exists().then_some(default_path),
            mode: WallpaperMode::Auto,
            fill_mode: FillMode::Cover,
            blur_sigma: 0.0,
            rotate_interval_secs: 0,
            video_fps: 30,
            video_loop: true,
            solid_color: "#1d2021".to_string(),
        }
    }
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            position: PowerPosition::BottomCenter,
            compact_icons: true,
            button_size: 40,
            background_color: None,
            opacity: 0.86,
            show_suspend: true,
            show_reboot: true,
            show_poweroff: true,
            require_confirm: false,
        }
    }
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            show_capslock_warning: true,
        }
    }
}

impl Config {
    pub fn load(explicit_path: Option<&Path>) -> (Self, Option<String>) {
        let candidates: Vec<PathBuf> = if let Some(path) = explicit_path {
            vec![path.to_path_buf()]
        } else {
            let mut paths = vec![
                PathBuf::from("/etc/kgreet/config.yaml"),
                PathBuf::from("/etc/kgreet/config.yml"),
                PathBuf::from("/var/lib/kgreet/config.yaml"),
                PathBuf::from("/var/lib/kgreet/config.yml"),
            ];
            if let Ok(home) = std::env::var("HOME") {
                let base = PathBuf::from(home).join(".config/kgreet");
                paths.push(base.join("config.yaml"));
                paths.push(base.join("config.yml"));
            }
            paths
        };

        for path in candidates {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(raw) => match serde_yaml_ng::from_str::<Config>(&raw) {
                        Ok(mut cfg) => {
                            cfg.sanitize();
                            return (cfg, None);
                        }
                        Err(err) => {
                            let mut cfg = Config::default();
                            cfg.sanitize();
                            return (
                                cfg,
                                Some(format!("Invalid YAML config at {}: {}", path.display(), err)),
                            );
                        }
                    },
                    Err(err) => {
                        let mut cfg = Config::default();
                        cfg.sanitize();
                        return (
                            cfg,
                            Some(format!("Failed to read config at {}: {}", path.display(), err)),
                        );
                    }
                }
            }
        }

        if let Some(path) = explicit_path {
            let mut cfg = Config::default();
            cfg.sanitize();
            return (
                cfg,
                Some(format!("Config file not found: {}", path.display())),
            );
        }

        let mut cfg = Config::default();
        cfg.sanitize();
        (cfg, None)
    }

    pub fn resolved_dot_char(&self) -> char {
        self.input
            .dot_char
            .chars()
            .next()
            .unwrap_or('\u{2022}')
    }

    fn sanitize(&mut self) {
        self.card.opacity = self.card.opacity.clamp(0.0, 1.0);
        self.card.border_opacity = self.card.border_opacity.clamp(0.0, 1.0);
        self.card.width = self.card.width.clamp(280, 720);
        self.card.padding_x = self.card.padding_x.clamp(12, 64);
        self.card.padding_y = self.card.padding_y.clamp(12, 64);
        self.card.avatar_size = self.card.avatar_size.clamp(48, 160);
        self.card.avatar_border_width = self.card.avatar_border_width.clamp(0, 8);
        self.input.opacity = self.input.opacity.clamp(0.0, 1.0);
        self.input.border_opacity = self.input.border_opacity.clamp(0.0, 1.0);
        self.input.height = self.input.height.clamp(32, 64);
        self.input.dot_spacing = self.input.dot_spacing.clamp(0, 16);
        self.input.cursor_glide.duration_ms = self.input.cursor_glide.duration_ms.clamp(20, 600);
        self.input.cursor_glide.width = self.input.cursor_glide.width.clamp(1.5, 24.0);
        self.input.cursor_glide.corner_radius = self.input.cursor_glide.corner_radius.clamp(0.0, 12.0);
        self.input.cursor_glide.trail_smear = self.input.cursor_glide.trail_smear.clamp(0.0, 1.0);
        self.appearance.backdrop_dim = self.appearance.backdrop_dim.clamp(0.0, 0.9);
        self.wallpaper.blur_sigma = self.wallpaper.blur_sigma.clamp(0.0, 80.0);
        self.wallpaper.video_fps = self.wallpaper.video_fps.clamp(1, 60);
        self.power.button_size = self.power.button_size.clamp(30, 64);
        self.power.opacity = self.power.opacity.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_yaml_config_parses_cleanly() {
        let raw = include_str!("../config.example.yaml");
        let parsed: Config = serde_yaml_ng::from_str(raw).expect("example YAML config must parse");
        assert!(!parsed.session.show_selector);
        assert_eq!(parsed.power.position, PowerPosition::BottomCenter);
        assert_eq!(parsed.monitor.secondary_mode, SecondaryMonitorMode::None);
        assert!(parsed.user.show_display_name);
        assert_eq!(parsed.card.avatar_size, 96);
        assert_eq!(parsed.input.dot_spacing, 2);
    }

    #[test]
    fn sanitize_clamps_out_of_range_values() {
        let mut cfg = Config::default();
        cfg.card.opacity = 5.0;
        cfg.input.dot_spacing = 100;
        cfg.wallpaper.blur_sigma = -10.0;
        cfg.wallpaper.video_fps = 0;
        cfg.sanitize();
        assert_eq!(cfg.card.opacity, 1.0);
        assert_eq!(cfg.input.dot_spacing, 16);
        assert_eq!(cfg.wallpaper.blur_sigma, 0.0);
        assert_eq!(cfg.wallpaper.video_fps, 1);
    }
}

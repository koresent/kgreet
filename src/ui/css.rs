use crate::config::Config;

pub fn build_css(cfg: &Config) -> String {
    let card_rgba = hex_to_rgba(&cfg.card.background_color, cfg.card.opacity);
    let card_border_rgba = hex_to_rgba(&cfg.card.border_color, cfg.card.border_opacity);
    let input_rgba = hex_to_rgba(&cfg.input.background_color, cfg.input.opacity);
    let input_border_rgba = hex_to_rgba(&cfg.input.border_color, cfg.input.border_opacity);
    let input_focus = cfg
        .input
        .focus_border_color
        .as_deref()
        .unwrap_or(&cfg.appearance.accent_color);
    let avatar_border = cfg
        .card
        .avatar_border_color
        .as_deref()
        .unwrap_or(&cfg.appearance.accent_color);
    let dock_bg = cfg
        .power
        .background_color
        .as_deref()
        .unwrap_or(&cfg.card.background_color);
    let dim_rgba = format!("rgba(0, 0, 0, {:.3})", cfg.appearance.backdrop_dim);
    let dock_rgba = hex_to_rgba(dock_bg, cfg.power.opacity);
    let dock_hover_rgba = hex_to_rgba(&cfg.input.background_color, (cfg.power.opacity + 0.08).min(1.0));
    let error_bg = hex_to_rgba(&cfg.appearance.error_color, 0.16);
    let error_border = hex_to_rgba(&cfg.appearance.error_color, 0.72);
    let warn_bg = hex_to_rgba(&cfg.appearance.accent_color, 0.16);
    let warn_border = hex_to_rgba(&cfg.appearance.accent_color, 0.72);

    let clock_time_color = cfg
        .clock
        .time_color
        .as_deref()
        .map(|c| hex_to_rgba(c, 1.0))
        .unwrap_or_else(|| hex_to_rgba(&cfg.appearance.text_color, 0.92));
    let clock_date_color = cfg
        .clock
        .date_color
        .as_deref()
        .map(|c| hex_to_rgba(c, 1.0))
        .unwrap_or_else(|| hex_to_rgba(&cfg.appearance.muted_color, 0.85));
    let clock_sub_color = cfg
        .clock
        .subtitle_color
        .as_deref()
        .map(|c| hex_to_rgba(c, 1.0))
        .unwrap_or_else(|| hex_to_rgba(&cfg.appearance.muted_color, 0.65));
    let clock_time_weight = if cfg.clock.time_bold { "700" } else { "400" };

    let caret_color = if cfg.input.cursor_glide.enabled {
        "transparent"
    } else {
        input_focus
    };

    format!(
        r#"
window.kgreet-window {{
    background-color: {solid_bg};
    color: {text};
    font-family: "{font}", "Adwaita Sans", "Inter", sans-serif;
    font-size: 14px;
}}

window.kgreet-secondary-black {{
    background-color: #000000;
}}

.backdrop-scrim {{
    background-color: {dim_rgba};
}}

.login-card {{
    background-color: {card_rgba};
    border: 1px solid {card_border_rgba};
    border-radius: {card_radius}px;
    padding: {pad_y}px {pad_x}px {pad_y}px {pad_x}px;
}}

.avatar-frame {{
    border-radius: 999px;
    border: {avatar_bw}px solid {avatar_border};
    background-color: {input_bg};
}}

.avatar-initial {{
    font-size: 28px;
    font-weight: 700;
    color: {text};
}}

.display-name {{
    font-size: 18px;
    font-weight: 700;
    color: {text};
}}

.username-subtitle {{
    font-size: 12px;
    color: {muted};
}}

.field-label {{
    font-size: 12px;
    font-weight: 600;
    color: {muted};
}}

entry.password-input {{
    background-color: {input_rgba};
    background-image: none;
    box-shadow: none;
    color: {text};
    border: 1.5px solid {input_border_rgba};
    border-radius: {input_radius}px;
    padding: 0 44px 0 16px;
    min-height: 0;
    caret-color: {caret_color};
    outline: none;
}}

entry.password-input:focus-within {{
    border: 1.5px solid {input_focus};
    box-shadow: none;
    outline: none;
}}

entry.password-input.input-error,
entry.password-input.input-error:focus-within {{
    border: 2px solid {error};
    box-shadow: none;
    outline: none;
}}

button.peek-inline {{
    background-color: transparent;
    background-image: none;
    box-shadow: none;
    border: none;
    border-radius: 999px;
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    margin: 0 7px 0 0;
}}

button.peek-inline:hover {{
    background-color: rgba(235, 219, 178, 0.14);
}}

entry.password-input text {{
    background: transparent;
}}

entry.password-input.masked text {{
    font-family: sans-serif;
    font-size: 15px;
    letter-spacing: {dot_spacing}px;
}}

entry.password-input.masked text placeholder {{
    font-family: "{font}", "Adwaita Sans", "Inter", sans-serif;
    font-size: 14px;
    font-weight: 500;
    letter-spacing: 0px;
    padding-left: 6px;
}}

entry.password-input text placeholder {{
    font-weight: 500;
    color: {muted};
    opacity: 0.88;
    padding-left: 6px;
}}

dropdown.kgreet-dropdown > button {{
    background-color: {input_rgba};
    background-image: none;
    box-shadow: none;
    color: {text};
    border: 1px solid {input_border_rgba};
    border-radius: 10px;
    min-height: 38px;
    padding: 4px 12px;
}}

dropdown.kgreet-dropdown > button:hover {{
    border-color: {accent};
}}

dropdown.kgreet-dropdown > button:focus-visible {{
    outline: 2px solid {accent};
    outline-offset: 2px;
}}

button:disabled {{
    opacity: 0.58;
}}

button.submit-inline {{
    background-color: {accent};
    background-image: none;
    box-shadow: none;
    color: {accent_text};
    font-weight: 700;
    border: none;
    border-radius: 999px;
    min-width: {input_height}px;
    min-height: {input_height}px;
    padding: 0;
}}

button.submit-inline:hover:not(:disabled) {{
    filter: brightness(1.12);
    box-shadow: 0 0 0 1.5px rgba(235, 219, 178, 0.35);
}}

button.submit-inline:active:not(:disabled) {{
    filter: brightness(0.92);
}}

button.submit-inline:focus-visible {{
    outline: 2px solid {text};
    outline-offset: 2px;
}}

.status-banner {{
    border-radius: 10px;
    padding: 8px 11px;
    font-size: 12px;
}}

.status-banner.error {{
    background-color: {error_bg};
    border: 1px solid {error_border};
    color: {text};
}}

.status-banner.warning {{
    background-color: {warn_bg};
    border: 1px solid {warn_border};
    color: {text};
}}

.status-icon-error {{
    color: {error};
}}

.status-icon-warning {{
    color: {accent};
}}

.power-dock-bar {{
    background-color: {dock_rgba};
    border: 1px solid {card_border_rgba};
    border-radius: 12px;
    padding: 6px;
}}

button.power-circle {{
    background-color: {dock_rgba};
    background-image: none;
    box-shadow: none;
    color: {text};
    border: 1px solid {card_border_rgba};
    border-radius: 999px;
    min-width: {power_size}px;
    min-height: {power_size}px;
    padding: 0;
}}

button.power-circle:hover:not(:disabled) {{
    background-color: {dock_hover_rgba};
    background-image: none;
    border-color: {accent};
}}

button.power-circle:active:not(:disabled) {{
    filter: brightness(0.9);
}}

button.power-circle:focus-visible {{
    outline: 2px solid {accent};
    outline-offset: 2px;
}}

button.power-button {{
    background-color: transparent;
    background-image: none;
    box-shadow: none;
    color: {text};
    border: 1px solid transparent;
    border-radius: 8px;
    padding: 6px 12px;
    min-height: 36px;
    font-size: 13px;
    font-weight: 600;
}}

button.power-button:hover:not(:disabled) {{
    background-color: {dock_hover_rgba};
    background-image: none;
    border-color: {card_border_rgba};
}}

button.power-button:focus-visible {{
    outline: 2px solid {accent};
    outline-offset: 2px;
}}

button.confirm-danger {{
    background-color: {error};
    background-image: none;
    box-shadow: none;
    color: {accent_text};
    font-weight: 700;
    border: none;
    border-radius: 8px;
    padding: 6px 12px;
    min-height: 32px;
}}

button.confirm-danger:focus-visible {{
    outline: 2px solid {text};
    outline-offset: 2px;
}}

button.confirm-cancel {{
    background-color: {input_bg};
    background-image: none;
    box-shadow: none;
    color: {text};
    border: 1px solid {card_border_rgba};
    border-radius: 8px;
    padding: 6px 12px;
    min-height: 32px;
}}

button.confirm-cancel:focus-visible {{
    outline: 2px solid {accent};
    outline-offset: 2px;
}}

.kgreet-clock-time {{
    font-size: {clock_time_size}px;
    font-weight: {clock_time_weight};
    color: {clock_time_color};
}}

.kgreet-clock-date {{
    font-size: {clock_date_size}px;
    color: {clock_date_color};
}}

.kgreet-clock-subtitle {{
    font-size: {clock_sub_size}px;
    color: {clock_sub_color};
}}
"#,
        solid_bg = cfg.wallpaper.solid_color,
        text = cfg.appearance.text_color,
        muted = cfg.appearance.muted_color,
        accent = cfg.appearance.accent_color,
        accent_text = cfg.appearance.accent_text_color,
        error = cfg.appearance.error_color,
        input_bg = cfg.input.background_color,
        input_rgba = input_rgba,
        input_border_rgba = input_border_rgba,
        input_focus = input_focus,
        caret_color = caret_color,
        input_radius = cfg.input.border_radius,
        input_height = cfg.input.height,
        dot_spacing = cfg.input.dot_spacing,
        font = cfg.appearance.font_family.replace('"', ""),
        card_radius = cfg.card.border_radius,
        pad_x = cfg.card.padding_x,
        pad_y = cfg.card.padding_y,
        avatar_bw = cfg.card.avatar_border_width,
        avatar_border = avatar_border,
        dim_rgba = dim_rgba,
        card_rgba = card_rgba,
        card_border_rgba = card_border_rgba,
        dock_rgba = dock_rgba,
        dock_hover_rgba = dock_hover_rgba,
        error_bg = error_bg,
        error_border = error_border,
        warn_bg = warn_bg,
        warn_border = warn_border,
        power_size = cfg.power.button_size,
        clock_time_size = cfg.clock.time_font_size,
        clock_time_weight = clock_time_weight,
        clock_time_color = clock_time_color,
        clock_date_size = cfg.clock.date_font_size,
        clock_date_color = clock_date_color,
        clock_sub_size = cfg.clock.subtitle_font_size,
        clock_sub_color = clock_sub_color,
    )
}

fn hex_to_rgba(hex: &str, alpha: f64) -> String {
    let trimmed = hex.trim().trim_start_matches('#');
    if trimmed.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        )
    {
        return format!("rgba({r}, {g}, {b}, {:.3})", alpha.clamp(0.0, 1.0));
    }
    format!("rgba(40, 40, 40, {:.3})", alpha.clamp(0.0, 1.0))
}

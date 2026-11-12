use gtk4::gdk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::MonitorConfig;

pub struct MonitorSelection {
    pub primary: Option<gdk::Monitor>,
    pub secondary: Vec<gdk::Monitor>,
}

pub fn select_monitors(display: &gdk::Display, cfg: &MonitorConfig) -> MonitorSelection {
    let list = display.monitors();
    let mut all = Vec::new();
    for i in 0..list.n_items() {
        if let Some(obj) = list.item(i)
            && let Ok(mon) = obj.downcast::<gdk::Monitor>()
        {
            all.push(mon);
        }
    }

    if all.is_empty() {
        return MonitorSelection {
            primary: None,
            secondary: Vec::new(),
        };
    }

    let target = cfg.target.trim();
    let primary_idx = if target.is_empty() || target.eq_ignore_ascii_case("primary") {
        // Prefer landscape outputs over rotated portrait monitors when no connector is specified.
        all.iter()
            .position(|m| {
                let g = m.geometry();
                g.width() >= g.height()
            })
            .unwrap_or(0)
    } else {
        all.iter()
            .position(|m| {
                m.connector()
                    .map(|c| c.eq_ignore_ascii_case(target))
                    .unwrap_or(false)
            })
            .unwrap_or(0)
    };

    let primary = all.remove(primary_idx);
    MonitorSelection {
        primary: Some(primary),
        secondary: all,
    }
}

pub fn configure_layer_window(
    window: &gtk4::ApplicationWindow,
    monitor: Option<&gdk::Monitor>,
    interactive: bool,
) -> bool {
    if !gtk4_layer_shell::is_supported() {
        return false;
    }

    window.init_layer_shell();
    window.set_namespace(Some("kgreet"));
    window.set_layer(Layer::Overlay);
    window.set_exclusive_zone(-1);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);
    window.set_keyboard_mode(if interactive {
        KeyboardMode::Exclusive
    } else {
        KeyboardMode::None
    });

    if let Some(mon) = monitor {
        window.set_monitor(Some(mon));
    }

    true
}

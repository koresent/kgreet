use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gtk4::cairo;
use gtk4::glib;
use gtk4::graphene;
use gtk4::prelude::*;

use crate::config::CursorGlideConfig;

#[derive(Debug, Clone, Copy)]
struct CursorRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct GlideState {
    prev: Option<CursorRect>,
    cur: Option<CursorRect>,
    changed_at: Instant,
    tick_id: Option<gtk4::TickCallbackId>,
}

pub fn attach_cursor_glide(
    password_entry: &gtk4::PasswordEntry,
    text_widget: &gtk4::Text,
    cfg: &CursorGlideConfig,
    fallback_color_hex: &str,
) -> gtk4::Overlay {
    let overlay = gtk4::Overlay::new();
    overlay.set_hexpand(true);
    overlay.set_child(Some(password_entry));

    if !cfg.enabled {
        return overlay;
    }

    let drawing_area = gtk4::DrawingArea::builder()
        .can_target(false)
        .can_focus(false)
        .hexpand(true)
        .vexpand(true)
        .build();
    overlay.add_overlay(&drawing_area);

    let color_hex = cfg.color.as_deref().unwrap_or(fallback_color_hex);
    let rgb = parse_hex_rgb(color_hex);
    let duration_secs = (cfg.duration_ms.max(10) as f64) / 1000.0;
    let bar_width = cfg.width;
    let corner_radius = cfg.corner_radius;
    let trail_smear = cfg.trail_smear;

    let state = Rc::new(RefCell::new(GlideState {
        prev: None,
        cur: None,
        changed_at: Instant::now(),
        tick_id: None,
    }));

    // Draw a rounded vertical bar cursor and 150ms glide smear.
    let state_draw = Rc::clone(&state);
    let entry_draw = password_entry.clone();
    let text_draw = text_widget.clone();
    drawing_area.set_draw_func(move |da, cr, _, _| {
        if !(text_draw.has_focus() || text_draw.is_focus()) {
            return;
        }
        let mut st = state_draw.borrow_mut();
        if st.cur.is_none() {
            let pos = entry_draw.position().max(0) as usize;
            let (strong, _) = text_draw.compute_cursor_extents(pos);
            let local_pt = graphene::Point::new(strong.x(), strong.y());
            if let Some(mapped) = text_draw.compute_point(da, &local_pt) {
                let rect = CursorRect {
                    x: (mapped.x() as f64).max(1.0),
                    y: mapped.y() as f64 - 1.0,
                    w: bar_width,
                    h: (strong.height() as f64 + 2.0).max(18.0),
                };
                st.prev = Some(rect);
                st.cur = Some(rect);
            }
        }
        let Some(cur) = st.cur else {
            return;
        };
        let prev = st.prev.unwrap_or(cur);

        let elapsed = st.changed_at.elapsed().as_secs_f64();
        let t = (elapsed / duration_secs).clamp(0.0, 1.0);
        let move_dist = ((cur.x - prev.x).powi(2) + (cur.y - prev.y).powi(2)).sqrt();

        let (r, g, b) = rgb;
        let alpha = 0.96;

        if t >= 1.0 || move_dist < 1.0 {
            let rad = corner_radius.min(cur.w * 0.5).min(cur.h * 0.5);
            draw_rounded_rect(cr, cur.x, cur.y, cur.w, cur.h, rad);
            cr.set_source_rgba(r, g, b, alpha);
            let _ = cr.fill();
            return;
        }

        let t_lead = ease_leading(t);
        let t_trail = t_lead + (ease_trailing(t) - t_lead) * trail_smear;

        let lead_x = prev.x + (cur.x - prev.x) * t_lead;
        let trail_x = prev.x + (cur.x - prev.x) * t_trail;
        let left_x = lead_x.min(trail_x);
        let right_x = (lead_x + cur.w).max(trail_x + cur.w);
        let smear_w = (right_x - left_x).max(cur.w);

        let y = prev.y + (cur.y - prev.y) * t_lead;
        let h = prev.h + (cur.h - prev.h) * t_lead;
        let rad = corner_radius.min(cur.w * 0.5).min(h * 0.5);

        draw_rounded_rect(cr, left_x, y, smear_w, h, rad);

        // Fade trailing tail to 0.75 while keeping the leading head at full alpha.
        let (grad_start, grad_end) = if cur.x >= prev.x {
            (trail_x, lead_x + cur.w)
        } else {
            (trail_x + cur.w, lead_x)
        };
        let grad = cairo::LinearGradient::new(grad_start, y, grad_end, y);
        grad.add_color_stop_rgba(0.0, r, g, b, 0.75 * alpha);
        grad.add_color_stop_rgba(1.0, r, g, b, alpha);
        let _ = cr.set_source(&grad);
        let _ = cr.fill();
    });

    let update_cursor = {
        let entry = password_entry.clone();
        let text = text_widget.clone();
        let da = drawing_area.clone();
        let state = Rc::clone(&state);

        Rc::new(move |animate: bool| {
            let pos = entry.position().max(0) as usize;
            let (strong, _) = text.compute_cursor_extents(pos);
            let local_pt = graphene::Point::new(strong.x(), strong.y());
            let Some(mapped) = text.compute_point(&da, &local_pt) else {
                return;
            };

            let next_rect = CursorRect {
                x: (mapped.x() as f64).max(1.0),
                y: mapped.y() as f64 - 1.0,
                w: bar_width,
                h: (strong.height() as f64 + 2.0).max(18.0),
            };

            let mut st = state.borrow_mut();

            if !animate || st.cur.is_none() {
                st.prev = Some(next_rect);
                st.cur = Some(next_rect);
                st.changed_at = Instant::now() - std::time::Duration::from_secs(1);
                da.queue_draw();
                return;
            }

            let cur_snapshot = st.cur.unwrap_or(next_rect);
            if (cur_snapshot.x - next_rect.x).abs() < 0.5
                && (cur_snapshot.y - next_rect.y).abs() < 0.5
            {
                st.cur = Some(next_rect);
                da.queue_draw();
                return;
            }

            st.prev = Some(cur_snapshot);
            st.cur = Some(next_rect);
            st.changed_at = Instant::now();

            if st.tick_id.is_none() {
                let da_tick = da.clone();
                let state_tick = Rc::clone(&state);
                let id = da.add_tick_callback(move |_, _| {
                    let mut inner = state_tick.borrow_mut();
                    da_tick.queue_draw();
                    if inner.changed_at.elapsed().as_secs_f64() >= duration_secs {
                        inner.tick_id = None;
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                });
                st.tick_id = Some(id);
            }
            da.queue_draw();
        })
    };

    let upd_pos = Rc::clone(&update_cursor);
    password_entry.connect_notify_local(Some("cursor-position"), move |_, _| {
        upd_pos(true);
    });

    let upd_changed = Rc::clone(&update_cursor);
    password_entry.connect_changed(move |_| {
        upd_changed(true);
    });

    let upd_vis = Rc::clone(&update_cursor);
    text_widget.connect_visibility_notify(move |_| {
        let upd = Rc::clone(&upd_vis);
        glib::idle_add_local_once(move || {
            upd(true);
        });
    });

    let upd_focus = Rc::clone(&update_cursor);
    text_widget.connect_has_focus_notify(move |_| {
        upd_focus(false);
    });

    let upd_state_flags = Rc::clone(&update_cursor);
    password_entry.connect_state_flags_changed(move |_, _| {
        upd_state_flags(false);
    });

    let da_map = drawing_area.clone();
    text_widget.connect_map(move |_| {
        da_map.queue_draw();
    });

    let upd_resize = Rc::clone(&update_cursor);
    drawing_area.connect_resize(move |_, _, _| {
        upd_resize(false);
    });

    let upd_init = Rc::clone(&update_cursor);
    glib::idle_add_local_once(move || {
        upd_init(false);
    });

    overlay
}

fn ease_leading(x: f64) -> f64 {
    1.0 - (1.0 - x).powi(4)
}

fn ease_trailing(x: f64) -> f64 {
    x.powf(2.2)
}

fn draw_rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let deg = std::f64::consts::PI / 180.0;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90.0 * deg, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90.0 * deg);
    cr.arc(x + r, y + h - r, r, 90.0 * deg, 180.0 * deg);
    cr.arc(x + r, y + r, r, 180.0 * deg, 270.0 * deg);
    cr.close_path();
}

fn parse_hex_rgb(hex: &str) -> (f64, f64, f64) {
    let trimmed = hex.trim().trim_start_matches('#');
    if trimmed.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        )
    {
        return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    }
    (0.831, 0.745, 0.596)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glide_easing_boundaries_match_shader() {
        assert!((ease_leading(0.0) - 0.0).abs() < 1e-9);
        assert!((ease_leading(1.0) - 1.0).abs() < 1e-9);
        assert!((ease_trailing(0.0) - 0.0).abs() < 1e-9);
        assert!((ease_trailing(1.0) - 1.0).abs() < 1e-9);
        assert!(ease_leading(0.5) > ease_trailing(0.5));
    }
}

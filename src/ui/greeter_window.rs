use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use gtk4::gdk;
use gtk4::gdk_pixbuf;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;

use crate::auth::{AuthParams, authenticate_and_start};
use crate::config::{Config, PowerPosition, SecondaryMonitorMode};
use crate::i18n::STRINGS;
use crate::media::wallpaper::attach_wallpaper;
use crate::system::power::{PowerAction, execute_power_action};
use crate::system::sessions::{SessionEntry, SessionKind, discover_sessions};
use crate::system::users::{UserEntry, discover_users};
use crate::ui::css::build_css;
use crate::ui::cursor_glide::attach_cursor_glide;
use crate::ui::monitor::{configure_layer_window, select_monitors};

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
    pub windowed: bool,
    pub mock: bool,
    pub config_warning: Option<String>,
}

pub fn launch_ui(app: &gtk4::Application, cfg: Config, opts: RuntimeOptions) {
    let display = gdk::Display::default().expect("Wayland/X11 display is required");
    apply_css(&display, &cfg);

    let users = Rc::new(discover_users(&cfg.user));
    let sessions = Rc::new(discover_sessions(&cfg.session.default));

    let selection = select_monitors(&display, &cfg.monitor);
    let primary_mon = selection.primary;
    let target_size = primary_mon
        .as_ref()
        .map(|m| {
            let geom = m.geometry();
            (geom.width(), geom.height())
        })
        .unwrap_or((1920, 1080));

    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("kgreet")
        .build();
    window.add_css_class("kgreet-window");

    let use_layer_shell = if opts.windowed {
        window.set_default_size(1280, 800);
        false
    } else {
        let enabled = configure_layer_window(&window, primary_mon.as_ref(), true);
        if !enabled {
            window.set_default_size(1280, 800);
        }
        enabled
    };

    let mut wallpaper_pictures = Vec::new();
    let mut secondary_windows = Vec::new();

    if use_layer_shell && cfg.monitor.secondary_mode != SecondaryMonitorMode::None {
        for mon in &selection.secondary {
            let sec_win = gtk4::ApplicationWindow::builder()
                .application(app)
                .title("kgreet-secondary")
                .build();
            if cfg.monitor.secondary_mode == SecondaryMonitorMode::Black {
                sec_win.add_css_class("kgreet-secondary-black");
            } else {
                sec_win.add_css_class("kgreet-window");
                let sec_pic = gtk4::Picture::builder()
                    .hexpand(true)
                    .vexpand(true)
                    .can_shrink(true)
                    .build();
                let sec_overlay = gtk4::Overlay::new();
                sec_overlay.set_child(Some(&sec_pic));
                let sec_scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                sec_scrim.add_css_class("backdrop-scrim");
                sec_scrim.set_hexpand(true);
                sec_scrim.set_vexpand(true);
                sec_overlay.add_overlay(&sec_scrim);
                sec_win.set_child(Some(&sec_overlay));
                wallpaper_pictures.push(sec_pic);
            }
            configure_layer_window(&sec_win, Some(mon), false);
            secondary_windows.push(sec_win);
        }
    }

    let root_overlay = gtk4::Overlay::new();
    root_overlay.set_hexpand(true);
    root_overlay.set_vexpand(true);

    let bg_picture = gtk4::Picture::builder()
        .hexpand(true)
        .vexpand(true)
        .can_shrink(true)
        .build();
    root_overlay.set_child(Some(&bg_picture));
    wallpaper_pictures.insert(0, bg_picture);

    let scrim = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    scrim.add_css_class("backdrop-scrim");
    scrim.set_hexpand(true);
    scrim.set_vexpand(true);
    root_overlay.add_overlay(&scrim);

    let wallpaper_ctrl = attach_wallpaper(wallpaper_pictures, cfg.wallpaper.clone(), target_size);
    // Keep controller alive for the window lifetime without leaking on exit.
    let wallpaper_guard = Rc::new(RefCell::new(wallpaper_ctrl));

    let card = build_login_card(
        app,
        &cfg,
        &users,
        &sessions,
        &opts,
        Rc::clone(&wallpaper_guard),
    );
    root_overlay.add_overlay(&card);

    let confirm_state: Rc<RefCell<Option<gtk4::Box>>> = Rc::new(RefCell::new(None));
    if cfg.power.enabled && (cfg.power.show_suspend || cfg.power.show_reboot || cfg.power.show_poweroff) {
        let dock_wrapper = build_power_dock(&cfg, opts.mock, &card, Rc::clone(&confirm_state));
        root_overlay.add_overlay(&dock_wrapper);
    }

    let key_ctrl = gtk4::EventControllerKey::new();
    let confirm_esc = Rc::clone(&confirm_state);
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape
            && let Some(bar) = confirm_esc.borrow().as_ref()
            && bar.is_visible()
        {
            bar.set_visible(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);

    window.set_child(Some(&root_overlay));

    if let Some(warn) = opts.config_warning {
        eprintln!("kgreet config warning: {warn}");
    }

    for sec in &secondary_windows {
        sec.present();
    }
    window.present();
}

fn build_login_card(
    app: &gtk4::Application,
    cfg: &Config,
    users: &Rc<Vec<UserEntry>>,
    sessions: &Rc<Vec<SessionEntry>>,
    opts: &RuntimeOptions,
    wallpaper_guard: Rc<RefCell<Option<Rc<RefCell<crate::media::wallpaper::WallpaperController>>>>>,
) -> gtk4::Box {
    let s = &STRINGS;

    let card = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(14)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .width_request(cfg.card.width)
        .build();
    card.add_css_class("login-card");

    let selected_user_idx = Rc::new(Cell::new(0usize));
    let selected_session_idx = Rc::new(Cell::new(0usize));

    let avatar_holder = gtk4::Box::builder()
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();
    if cfg.card.show_avatar {
        let initial_user = users.first();
        avatar_holder.append(&build_avatar_widget(initial_user, cfg.card.avatar_size));
        card.append(&avatar_holder);
    }

    let header_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(4)
        .halign(gtk4::Align::Center)
        .build();

    let display_name_label = gtk4::Label::builder()
        .halign(gtk4::Align::Center)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    display_name_label.add_css_class("display-name");

    let username_subtitle = gtk4::Label::builder()
        .halign(gtk4::Align::Center)
        .visible(false)
        .build();
    username_subtitle.add_css_class("username-subtitle");

    if let Some(first) = users.first() {
        display_name_label.set_text(&first.display_name);
        if cfg.user.show_username_subtitle && first.display_name != first.username {
            username_subtitle.set_text(&format!("@{}", first.username));
            username_subtitle.set_visible(true);
        }
    } else {
        display_name_label.set_text("kgreet");
    }
    header_box.append(&display_name_label);
    header_box.append(&username_subtitle);
    card.append(&header_box);

    if cfg.user.allow_user_switch && users.len() > 1 {
        let user_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let user_lbl = gtk4::Label::builder()
            .label(s.user_label)
            .halign(gtk4::Align::Start)
            .build();
        user_lbl.add_css_class("field-label");

        let names: Vec<&str> = users
            .iter()
            .map(|u| u.display_name.as_str())
            .collect();
        let dropdown = gtk4::DropDown::from_strings(&names);
        dropdown.add_css_class("kgreet-dropdown");
        dropdown.set_cursor_from_name(Some("pointer"));
        dropdown.set_selected(0);
        user_lbl.set_mnemonic_widget(Some(&dropdown));

        let users_c = Rc::clone(users);
        let idx_c = Rc::clone(&selected_user_idx);
        let title_c = display_name_label.clone();
        let sub_c = username_subtitle.clone();
        let av_holder = avatar_holder.clone();
        let show_av = cfg.card.show_avatar;
        let av_size = cfg.card.avatar_size;
        let show_sub = cfg.user.show_username_subtitle;

        dropdown.connect_selected_notify(move |dd| {
            let idx = dd.selected() as usize;
            if let Some(u) = users_c.get(idx) {
                idx_c.set(idx);
                title_c.set_text(&u.display_name);
                if show_sub && u.display_name != u.username {
                    sub_c.set_text(&format!("@{}", u.username));
                    sub_c.set_visible(true);
                } else {
                    sub_c.set_visible(false);
                }
                if show_av {
                    while let Some(child) = av_holder.first_child() {
                        av_holder.remove(&child);
                    }
                    av_holder.append(&build_avatar_widget(Some(u), av_size));
                }
            }
        });

        user_box.append(&user_lbl);
        user_box.append(&dropdown);
        card.append(&user_box);
    }

    let pass_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let placeholder_text = if cfg.input.placeholder.trim().is_empty() {
        s.password_placeholder
    } else {
        cfg.input.placeholder.as_str()
    };
    let password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(placeholder_text)
        .show_peek_icon(cfg.input.show_peek_icon)
        .hexpand(true)
        .build();
    password_entry.add_css_class("password-input");
    password_entry.add_css_class("masked");
    password_entry.update_property(&[gtk4::accessible::Property::Label(s.password_label)]);

    let mut input_widget: Option<gtk4::Overlay> = None;
    if let Some(delegate) = password_entry.delegate()
        && let Ok(text_widget) = delegate.downcast::<gtk4::Text>()
    {
        text_widget.set_invisible_char(cfg.resolved_dot_char());
        let entry_c = password_entry.clone();
        text_widget.connect_visibility_notify(move |t| {
            if t.is_visible() {
                entry_c.remove_css_class("masked");
            } else {
                entry_c.add_css_class("masked");
            }
        });
        input_widget = Some(attach_cursor_glide(
            &password_entry,
            &text_widget,
            &cfg.input.cursor_glide,
            &cfg.appearance.accent_color,
        ));
    }

    let submit_icon = gtk4::Image::from_icon_name("go-next-symbolic");
    submit_icon.set_pixel_size(18);
    let unlock_button = gtk4::Button::builder()
        .child(&submit_icon)
        .tooltip_text(s.unlock_button)
        .width_request(cfg.input.height)
        .height_request(cfg.input.height)
        .valign(gtk4::Align::Center)
        .build();
    unlock_button.add_css_class("submit-inline");
    unlock_button.set_cursor_from_name(Some("pointer"));
    unlock_button.update_property(&[gtk4::accessible::Property::Label(s.unlock_button)]);

    if let Some(overlay) = input_widget {
        pass_row.append(&overlay);
    } else {
        pass_row.append(&password_entry);
    }
    pass_row.append(&unlock_button);
    card.append(&pass_row);

    if cfg.session.show_selector && !sessions.is_empty() {
        let sess_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        let sess_lbl = gtk4::Label::builder()
            .label(s.session_label)
            .halign(gtk4::Align::Start)
            .build();
        sess_lbl.add_css_class("field-label");

        let names: Vec<&str> = sessions.iter().map(|se| se.name.as_str()).collect();
        let dropdown = gtk4::DropDown::from_strings(&names);
        dropdown.add_css_class("kgreet-dropdown");
        dropdown.set_cursor_from_name(Some("pointer"));
        dropdown.set_selected(0);
        sess_lbl.set_mnemonic_widget(Some(&dropdown));

        let sess_idx_c = Rc::clone(&selected_session_idx);
        dropdown.connect_selected_notify(move |dd| {
            sess_idx_c.set(dd.selected() as usize);
        });

        sess_box.append(&sess_lbl);
        sess_box.append(&dropdown);
        card.append(&sess_box);
    }

    let capslock_banner = build_status_banner("dialog-warning-symbolic", s.capslock_active, false);
    capslock_banner.set_visible(false);
    card.append(&capslock_banner);

    if cfg.keyboard.show_capslock_warning {
        wire_capslock_indicator(&capslock_banner);
    }

    let error_banner = build_status_banner("dialog-error-symbolic", "", true);
    error_banner.set_visible(false);
    card.append(&error_banner);

    if let Some(warn) = &opts.config_warning {
        set_banner_text(&error_banner, warn);
        error_banner.set_visible(true);
    }

    if users.is_empty() {
        set_banner_text(&error_banner, s.err_no_users);
        error_banner.set_visible(true);
        password_entry.set_sensitive(false);
        unlock_button.set_sensitive(false);
        unlock_button.set_cursor_from_name(Some("not-allowed"));
        return card;
    }

    // Clear stale auth error as soon as the user edits the password.
    let err_on_type = error_banner.clone();
    password_entry.connect_changed(move |_| {
        if err_on_type.is_visible() {
            err_on_type.set_visible(false);
        }
    });

    let auth_busy = Rc::new(Cell::new(false));
    let submit_auth = {
        let app = app.clone();
        let cfg = cfg.clone();
        let users = Rc::clone(users);
        let sessions = Rc::clone(sessions);
        let selected_user_idx = Rc::clone(&selected_user_idx);
        let selected_session_idx = Rc::clone(&selected_session_idx);
        let password_entry = password_entry.clone();
        let unlock_button = unlock_button.clone();
        let error_banner = error_banner.clone();
        let auth_busy = Rc::clone(&auth_busy);
        let mock = opts.mock;

        Rc::new(move || {
            if auth_busy.get() {
                return;
            }
            let Some(user) = users.get(selected_user_idx.get()) else {
                set_banner_text(&error_banner, s.err_no_users);
                error_banner.set_visible(true);
                return;
            };
            let password = password_entry.text().to_string();
            if password.is_empty() {
                set_banner_text(&error_banner, s.err_empty_password);
                error_banner.set_visible(true);
                password_entry.grab_focus();
                return;
            }

            let (session_cmd, session_kind) = sessions
                .get(selected_session_idx.get())
                .map(|se| (se.exec.clone(), se.kind))
                .unwrap_or_else(|| {
                    (
                        crate::system::sessions::split_command(&cfg.session.default),
                        SessionKind::Wayland,
                    )
                });

            auth_busy.set(true);
            error_banner.set_visible(false);
            password_entry.set_sensitive(false);
            unlock_button.set_sensitive(false);
            unlock_button.set_cursor_from_name(Some("not-allowed"));
            unlock_button.set_tooltip_text(Some(s.unlocking_button));
            let spinner = gtk4::Spinner::new();
            spinner.start();
            unlock_button.set_child(Some(&spinner));

            let params = AuthParams {
                username: user.username.clone(),
                password,
                session_cmd,
                session_kind,
                extra_env: cfg.session.extra_env.clone(),
                mock,
            };

            let (tx, rx) = mpsc::channel::<Result<(), String>>();
            thread::spawn(move || {
                let res = authenticate_and_start(&params);
                let _ = tx.send(res);
            });

            let app_c = app.clone();
            let pass_c = password_entry.clone();
            let btn_c = unlock_button.clone();
            let err_c = error_banner.clone();
            let busy_c = Rc::clone(&auth_busy);
            let wp_guard = Rc::clone(&wallpaper_guard);

            glib::timeout_add_local(Duration::from_millis(20), move || match rx.try_recv() {
                Ok(Ok(())) => {
                    // Stop video decoder cleanly before greetd spawns the user session.
                    wp_guard.borrow_mut().take();
                    app_c.quit();
                    glib::ControlFlow::Break
                }
                Ok(Err(msg)) => {
                    busy_c.set(false);
                    pass_c.set_text("");
                    pass_c.set_sensitive(true);
                    btn_c.set_sensitive(true);
                    btn_c.set_cursor_from_name(Some("pointer"));
                    btn_c.set_tooltip_text(Some(s.unlock_button));
                    let icon = gtk4::Image::from_icon_name("go-next-symbolic");
                    icon.set_pixel_size(18);
                    btn_c.set_child(Some(&icon));
                    set_banner_text(&err_c, &msg);
                    err_c.set_visible(true);
                    pass_c.grab_focus();
                    glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    busy_c.set(false);
                    pass_c.set_sensitive(true);
                    btn_c.set_sensitive(true);
                    btn_c.set_cursor_from_name(Some("pointer"));
                    btn_c.set_tooltip_text(Some(s.unlock_button));
                    let icon = gtk4::Image::from_icon_name("go-next-symbolic");
                    icon.set_pixel_size(18);
                    btn_c.set_child(Some(&icon));
                    set_banner_text(&err_c, "Authentication worker exited unexpectedly.");
                    err_c.set_visible(true);
                    glib::ControlFlow::Break
                }
            });
        })
    };

    let submit_btn = Rc::clone(&submit_auth);
    unlock_button.connect_clicked(move |_| submit_btn());

    let submit_enter = Rc::clone(&submit_auth);
    password_entry.connect_activate(move |_| submit_enter());

    let pass_focus = password_entry.clone();
    glib::idle_add_local_once(move || {
        pass_focus.grab_focus();
    });

    // Attach shared error banner handle onto card data for power dock feedback.
    unsafe {
        card.set_data("kgreet-error-banner", error_banner);
    }

    card
}

fn build_power_dock(
    cfg: &Config,
    mock: bool,
    card: &gtk4::Box,
    confirm_state: Rc<RefCell<Option<gtk4::Box>>>,
) -> gtk4::Box {
    let s = &STRINGS;

    let (halign, valign) = match cfg.power.position {
        PowerPosition::BottomCenter => (gtk4::Align::Center, gtk4::Align::End),
        PowerPosition::BottomLeft => (gtk4::Align::Start, gtk4::Align::End),
        PowerPosition::BottomRight => (gtk4::Align::End, gtk4::Align::End),
        PowerPosition::TopCenter => (gtk4::Align::Center, gtk4::Align::Start),
        PowerPosition::TopLeft => (gtk4::Align::Start, gtk4::Align::Start),
        PowerPosition::TopRight => (gtk4::Align::End, gtk4::Align::Start),
    };

    let wrapper = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(8)
        .halign(halign)
        .valign(valign)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();

    let confirm_bar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk4::Align::Center)
        .visible(false)
        .build();
    confirm_bar.add_css_class("power-dock-bar");

    let confirm_label = gtk4::Label::new(None);
    let confirm_yes = gtk4::Button::builder().label(s.confirm_yes).build();
    confirm_yes.add_css_class("confirm-danger");
    confirm_yes.set_cursor_from_name(Some("pointer"));
    let confirm_cancel = gtk4::Button::builder().label(s.confirm_cancel).build();
    confirm_cancel.add_css_class("confirm-cancel");
    confirm_cancel.set_cursor_from_name(Some("pointer"));

    confirm_bar.append(&confirm_label);
    confirm_bar.append(&confirm_yes);
    confirm_bar.append(&confirm_cancel);
    *confirm_state.borrow_mut() = Some(confirm_bar.clone());

    let pending_action: Rc<Cell<Option<PowerAction>>> = Rc::new(Cell::new(None));

    let bar_cancel = confirm_bar.clone();
    confirm_cancel.connect_clicked(move |_| {
        bar_cancel.set_visible(false);
    });

    let card_for_confirm = card.clone();
    let bar_exec = confirm_bar.clone();
    let pending_exec = Rc::clone(&pending_action);
    confirm_yes.connect_clicked(move |_| {
        bar_exec.set_visible(false);
        if let Some(action) = pending_exec.get()
            && let Err(err) = execute_power_action(action, mock)
        {
            show_card_error(&card_for_confirm, &err);
        }
    });

    let dock = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(if cfg.power.compact_icons { 10 } else { 4 })
        .halign(gtk4::Align::Center)
        .build();
    if !cfg.power.compact_icons {
        dock.add_css_class("power-dock-bar");
    }

    let items = [
        (cfg.power.show_suspend, PowerAction::Suspend, "media-playback-pause-symbolic"),
        (cfg.power.show_reboot, PowerAction::Reboot, "system-reboot-symbolic"),
        (cfg.power.show_poweroff, PowerAction::PowerOff, "system-shutdown-symbolic"),
    ];

    for (enabled, action, icon_name) in items {
        if !enabled {
            continue;
        }
        let label_text = action.label();
        let btn = make_power_button(
            icon_name,
            label_text,
            cfg.power.compact_icons,
            cfg.power.button_size,
        );
        let card_c = card.clone();
        let bar_c = confirm_bar.clone();
        let lbl_c = confirm_label.clone();
        let pending_c = Rc::clone(&pending_action);
        let yes_c = confirm_yes.clone();
        let require_confirm = cfg.power.require_confirm;

        btn.connect_clicked(move |_| {
            if require_confirm {
                pending_c.set(Some(action));
                lbl_c.set_text(&format!("{} {}", s.confirm_prompt_prefix, label_text));
                bar_c.set_visible(true);
                yes_c.grab_focus();
            } else if let Err(err) = execute_power_action(action, mock) {
                show_card_error(&card_c, &err);
            }
        });

        dock.append(&btn);
    }

    if matches!(
        cfg.power.position,
        PowerPosition::TopCenter | PowerPosition::TopLeft | PowerPosition::TopRight
    ) {
        wrapper.append(&dock);
        wrapper.append(&confirm_bar);
    } else {
        wrapper.append(&confirm_bar);
        wrapper.append(&dock);
    }

    wrapper
}

fn make_power_button(
    icon_name: &str,
    label: &str,
    compact_icons: bool,
    button_size: i32,
) -> gtk4::Button {
    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);

    let btn = if compact_icons {
        let btn = gtk4::Button::builder()
            .child(&icon)
            .tooltip_text(label)
            .width_request(button_size)
            .height_request(button_size)
            .build();
        btn.add_css_class("power-circle");
        btn
    } else {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        let lbl = gtk4::Label::new(Some(label));
        row.append(&icon);
        row.append(&lbl);

        let btn = gtk4::Button::builder()
            .child(&row)
            .tooltip_text(label)
            .build();
        btn.add_css_class("power-button");
        btn
    };

    btn.set_cursor_from_name(Some("pointer"));
    btn.update_property(&[gtk4::accessible::Property::Label(label)]);
    btn
}

fn build_avatar_widget(user: Option<&UserEntry>, size: i32) -> gtk4::Box {
    let frame = gtk4::Box::builder()
        .width_request(size)
        .height_request(size)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .hexpand(false)
        .vexpand(false)
        .overflow(gtk4::Overflow::Hidden)
        .build();
    frame.add_css_class("avatar-frame");

    if let Some(u) = user
        && let Some(path) = &u.avatar_path
        && let Some(texture) = load_square_avatar(path, size)
    {
        let img = gtk4::Image::from_paintable(Some(&texture));
        img.set_pixel_size(size);
        img.set_halign(gtk4::Align::Center);
        img.set_valign(gtk4::Align::Center);
        frame.append(&img);
        return frame;
    }

    let initial = user
        .and_then(|u| u.display_name.chars().next())
        .unwrap_or('K')
        .to_uppercase()
        .to_string();
    let lbl = gtk4::Label::builder()
        .label(&initial)
        .hexpand(true)
        .vexpand(true)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .build();
    lbl.add_css_class("avatar-initial");
    frame.append(&lbl);
    frame
}

fn load_square_avatar(path: &std::path::Path, target_px: i32) -> Option<gdk::Texture> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    let w = pixbuf.width();
    let h = pixbuf.height();
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    let cropped = pixbuf.new_subpixbuf(x, y, side, side);
    let scaled = cropped.scale_simple(target_px, target_px, gdk_pixbuf::InterpType::Bilinear)?;
    Some(gdk::Texture::for_pixbuf(&scaled))
}

fn build_status_banner(icon_name: &str, text: &str, is_error: bool) -> gtk4::Box {
    let banner = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .build();
    banner.add_css_class("status-banner");
    banner.add_css_class(if is_error { "error" } else { "warning" });

    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    icon.add_css_class(if is_error {
        "status-icon-error"
    } else {
        "status-icon-warning"
    });

    let label = gtk4::Label::builder()
        .label(text)
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .build();

    banner.append(&icon);
    banner.append(&label);
    unsafe {
        banner.set_data("kgreet-banner-label", label);
    }
    banner
}

fn set_banner_text(banner: &gtk4::Box, text: &str) {
    if let Some(ptr) = unsafe { banner.data::<gtk4::Label>("kgreet-banner-label") } {
        let lbl = unsafe { ptr.as_ref() };
        lbl.set_text(text);
    }
}

fn show_card_error(card: &gtk4::Box, message: &str) {
    if let Some(ptr) = unsafe { card.data::<gtk4::Box>("kgreet-error-banner") } {
        let banner = unsafe { ptr.as_ref() };
        set_banner_text(banner, message);
        banner.set_visible(true);
    }
}

fn wire_capslock_indicator(banner: &gtk4::Box) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let Some(seat) = display.default_seat() else {
        return;
    };
    let Some(keyboard) = seat.keyboard() else {
        return;
    };

    banner.set_visible(keyboard.is_caps_locked());
    let banner_c = banner.clone();
    keyboard.connect_caps_lock_state_notify(move |dev| {
        banner_c.set_visible(dev.is_caps_locked());
    });
}

fn apply_css(display: &gdk::Display, cfg: &Config) {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(&build_css(cfg));
    gtk4::style_context_add_provider_for_display(
        display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

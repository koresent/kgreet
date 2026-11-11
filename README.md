# kgreet

Wayland `greetd` login greeter written in Rust using GTK4 and `gtk4-layer-shell`.

![kgreet preview](assets/preview.webp)

## Features

- Detailed YAML configuration (`config.yaml` / `config.yml`, see [config.example.yaml](config.example.yaml)).
- Configurable translucent or solid surfaces for the login card, password input, and power dock.
- Configurable power dock position (`bottom-center` by default) wired to `systemd-logind` D-Bus.
- Session selector hidden by default (`session.show_selector: false`).
- Display name header resolved from `user.display_name`, `AccountsService`, or `/etc/passwd` GECOS.
- Human-readable PAM and `greetd` error banners with icon and text.
- Glide-animated vertical bar cursor on the password input (`input.cursor_glide`).
- Static images, directories with rotation, Gaussian-style blur, and looping video wallpapers via `ffmpeg`.
- Primary-monitor-only presentation (`monitor.target` and `monitor.secondary_mode: none | black | wallpaper`).

## Build and local test

```bash
cargo build --release
cargo run -- --windowed --mock --config config.example.yaml
```

In `--mock` mode, any non-empty password signs in and exits, while `"wrong"`, `"locked"`, and `"expired"` trigger the corresponding error states.

## Configuration

`kgreet` searches for its YAML config in this order (unless `-c` / `--config` is passed):

1. `/etc/kgreet/config.yaml` (or `.yml`)
2. `/var/lib/kgreet/config.yaml` (or `.yml`)
3. `~/.config/kgreet/config.yaml` (or `.yml`)

## greetd integration

`kgreet` expects a Wayland compositor that implements `wlr-layer-shell` (such as `labwc` or `sway`), launched from `/etc/greetd/config.toml` under the `greeter` user.

Example wrapper script (`/usr/local/bin/kgreet-session`):

```sh
#!/usr/bin/env sh
exec labwc -s "/usr/local/bin/kgreet -c /etc/kgreet/config.yaml"
```

use std::cell::RefCell;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk4::gdk;
use gtk4::gdk_pixbuf;
use gtk4::glib;

use crate::config::{FillMode, WallpaperConfig, WallpaperMode};
use crate::media::blur::blur_rgba;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaKind {
    Image,
    Video,
}

struct ActiveVideo {
    stop: Arc<AtomicBool>,
    child: Option<Child>,
    timer_id: Option<glib::SourceId>,
}

impl Drop for ActiveVideo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(id) = self.timer_id.take() {
            id.remove();
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct WallpaperController {
    pictures: Vec<gtk4::Picture>,
    cfg: WallpaperConfig,
    playlist: Vec<PathBuf>,
    current_index: usize,
    active_video: Option<ActiveVideo>,
    rotation_timer: Option<glib::SourceId>,
}

impl Drop for WallpaperController {
    fn drop(&mut self) {
        self.active_video.take();
        if let Some(id) = self.rotation_timer.take() {
            id.remove();
        }
    }
}

pub fn attach_wallpaper(
    pictures: Vec<gtk4::Picture>,
    cfg: WallpaperConfig,
    target_size: (i32, i32),
) -> Option<Rc<RefCell<WallpaperController>>> {
    if pictures.is_empty() || cfg.mode == WallpaperMode::Solid {
        return None;
    }

    for pic in &pictures {
        pic.set_content_fit(match cfg.fill_mode {
            FillMode::Cover => gtk4::ContentFit::Cover,
            FillMode::Contain => gtk4::ContentFit::Contain,
            FillMode::Fill => gtk4::ContentFit::Fill,
        });
    }

    let playlist = resolve_playlist(&cfg);
    if playlist.is_empty() {
        return None;
    }

    let start_idx = if playlist.len() > 1 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0);
        nanos % playlist.len()
    } else {
        0
    };

    let controller = Rc::new(RefCell::new(WallpaperController {
        pictures,
        cfg: cfg.clone(),
        playlist,
        current_index: start_idx,
        active_video: None,
        rotation_timer: None,
    }));

    load_current(&controller, target_size);

    if cfg.rotate_interval_secs > 0 && controller.borrow().playlist.len() > 1 {
        let weak = Rc::downgrade(&controller);
        let id = glib::timeout_add_seconds_local(cfg.rotate_interval_secs as u32, move || {
            let Some(ctrl) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            {
                let mut inner = ctrl.borrow_mut();
                inner.current_index = (inner.current_index + 1) % inner.playlist.len();
            }
            load_current(&ctrl, target_size);
            glib::ControlFlow::Continue
        });
        controller.borrow_mut().rotation_timer = Some(id);
    }

    Some(controller)
}

fn load_current(ctrl: &Rc<RefCell<WallpaperController>>, target_size: (i32, i32)) {
    let (path, cfg) = {
        let mut inner = ctrl.borrow_mut();
        inner.active_video.take();
        let path = inner.playlist[inner.current_index].clone();
        (path, inner.cfg.clone())
    };

    let kind = classify_media(&path, cfg.mode);
    match kind {
        Some(MediaKind::Image) => {
            if let Some(texture) = load_image_texture(&path, cfg.blur_sigma) {
                let inner = ctrl.borrow();
                for pic in &inner.pictures {
                    pic.set_paintable(Some(&texture));
                }
            }
        }
        Some(MediaKind::Video) => {
            if !start_video_playback(ctrl, &path, &cfg, target_size) {
                eprintln!(
                    "kgreet: failed to start video wallpaper '{}', falling back",
                    path.display()
                );
            }
        }
        None => {}
    }
}

fn start_video_playback(
    ctrl: &Rc<RefCell<WallpaperController>>,
    path: &Path,
    cfg: &WallpaperConfig,
    target_size: (i32, i32),
) -> bool {
    // Cap blurred or raw decode dimensions to keep pipe bandwidth and CPU low.
    let (mut width, mut height) = (
        target_size.0.clamp(640, 1920),
        target_size.1.clamp(360, 1080),
    );
    if cfg.blur_sigma >= 8.0 {
        width = width.min(1280);
        height = height.min(720);
    }
    // Ensure even dimensions for video scalers.
    width &= !1;
    height &= !1;

    let mut vf = format!(
        "fps={},scale={width}:{height}:force_original_aspect_ratio=increase,crop={width}:{height}",
        cfg.video_fps
    );
    if cfg.blur_sigma > 0.2 {
        let sigma = cfg.blur_sigma.min(40.0);
        vf.push_str(&format!(",gblur=sigma={sigma:.1}:steps=2"));
    }

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    if cfg.video_loop {
        cmd.args(["-stream_loop", "-1"]);
    }
    cmd.arg("-i").arg(path);
    cmd.args([
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &vf,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let Ok(mut child) = cmd.spawn() else {
        return false;
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        return false;
    };

    let frame_bytes = (width as usize) * (height as usize) * 4;
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(2);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_reader = Arc::clone(&stop);

    thread::spawn(move || {
        while !stop_reader.load(Ordering::Relaxed) {
            let mut buf = vec![0u8; frame_bytes];
            if stdout.read_exact(&mut buf).is_err() {
                break;
            }
            let _ = tx.try_send(buf);
        }
    });

    let interval_ms = (1000 / cfg.video_fps.max(1)).max(16);
    let weak = Rc::downgrade(ctrl);
    let stop_timer = Arc::clone(&stop);

    let timer_id = glib::timeout_add_local(Duration::from_millis(interval_ms as u64), move || {
        if stop_timer.load(Ordering::Relaxed) {
            return glib::ControlFlow::Break;
        }
        let Some(ctrl_rc) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let mut latest = None;
        while let Ok(frame) = rx.try_recv() {
            latest = Some(frame);
        }

        if let Some(rgba) = latest {
            let bytes = glib::Bytes::from_owned(rgba);
            let texture = gdk::MemoryTexture::new(
                width,
                height,
                gdk::MemoryFormat::R8g8b8a8,
                &bytes,
                (width as usize) * 4,
            );
            let inner = ctrl_rc.borrow();
            for pic in &inner.pictures {
                pic.set_paintable(Some(&texture));
            }
        }
        glib::ControlFlow::Continue
    });

    ctrl.borrow_mut().active_video = Some(ActiveVideo {
        stop,
        child: Some(child),
        timer_id: Some(timer_id),
    });
    true
}

fn load_image_texture(path: &Path, blur_sigma: f32) -> Option<gdk::MemoryTexture> {
    let pixbuf = if blur_sigma >= 8.0 {
        gdk_pixbuf::Pixbuf::from_file_at_scale(path, 1280, 1280, true).ok()?
    } else {
        gdk_pixbuf::Pixbuf::from_file(path).ok()?
    };

    let width = pixbuf.width() as usize;
    let height = pixbuf.height() as usize;
    let channels = pixbuf.n_channels() as usize;
    let rowstride = pixbuf.rowstride() as usize;
    let src = unsafe { pixbuf.pixels() };

    let mut rgba = vec![255u8; width * height * 4];
    for y in 0..height {
        let row_src = &src[y * rowstride..];
        let row_dst = &mut rgba[y * width * 4..];
        for x in 0..width {
            let s = x * channels;
            let d = x * 4;
            row_dst[d] = row_src[s];
            row_dst[d + 1] = row_src[s + 1];
            row_dst[d + 2] = row_src[s + 2];
            row_dst[d + 3] = if channels == 4 { row_src[s + 3] } else { 255 };
        }
    }

    if blur_sigma > 0.1 {
        blur_rgba(&mut rgba, width, height, blur_sigma);
    }

    let bytes = glib::Bytes::from_owned(rgba);
    Some(gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        width * 4,
    ))
}

fn resolve_playlist(cfg: &WallpaperConfig) -> Vec<PathBuf> {
    let Some(path) = &cfg.path else {
        return Vec::new();
    };

    if path.is_dir() || cfg.mode == WallpaperMode::Directory {
        let Ok(entries) = fs::read_dir(path) else {
            return Vec::new();
        };
        let mut items: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && classify_media(p, WallpaperMode::Auto).is_some())
            .collect();
        items.sort();
        return items;
    }

    if path.is_file() {
        return vec![path.clone()];
    }

    Vec::new()
}

fn classify_media(path: &Path, mode: WallpaperMode) -> Option<MediaKind> {
    match mode {
        WallpaperMode::Image => Some(MediaKind::Image),
        WallpaperMode::Video => Some(MediaKind::Video),
        WallpaperMode::Solid => None,
        WallpaperMode::Auto | WallpaperMode::Directory => {
            let ext = path.extension()?.to_str()?.to_ascii_lowercase();
            match ext.as_str() {
                "jpg" | "jpeg" | "png" | "webp" | "bmp" | "avif" | "svg" => Some(MediaKind::Image),
                "mp4" | "webm" | "mkv" | "mov" | "avi" | "m4v" | "gif" => Some(MediaKind::Video),
                _ => None,
            }
        }
    }
}

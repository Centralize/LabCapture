//! Grabbing pixels from screens and windows, plus the pixel helpers the
//! screenshot and video code share (scaling, the mouse pointer overlay).

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use xcap::{Monitor, Window};

use crate::platform;

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub id: u32,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub primary: bool,
}

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub id: u32,
    pub title: String,
    pub app: String,
    pub width: u32,
    pub height: u32,
}

impl WindowInfo {
    pub fn label(&self) -> String {
        let title = shorten(&self.title, 46);
        if self.app.is_empty() || self.title.contains(&self.app) {
            title
        } else {
            format!("{title} — {}", shorten(&self.app, 20))
        }
    }
}

pub fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

pub fn list_monitors() -> Vec<MonitorInfo> {
    let Ok(monitors) = Monitor::all() else {
        return Vec::new();
    };
    let mut out: Vec<MonitorInfo> = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let width = m.width().unwrap_or(0);
            let height = m.height().unwrap_or(0);
            let primary = m.is_primary().unwrap_or(false);
            let mut label = format!("Screen {}  ·  {width} × {height}", i + 1);
            if primary {
                label.push_str("  ·  main");
            }
            MonitorInfo {
                id: m.id().unwrap_or(i as u32),
                label,
                x: m.x().unwrap_or(0),
                y: m.y().unwrap_or(0),
                width,
                height,
                scale: m.scale_factor().unwrap_or(1.0).max(0.5),
                primary,
            }
        })
        .collect();
    // Main screen first.
    out.sort_by_key(|m| !m.primary);
    out
}

pub fn list_windows() -> Vec<WindowInfo> {
    let Ok(windows) = Window::all() else {
        return Vec::new();
    };
    let me = std::process::id();
    windows
        .iter()
        .filter_map(|w| {
            if w.pid().ok()? == me || w.is_minimized().unwrap_or(false) {
                return None;
            }
            let title = w.title().unwrap_or_default();
            let width = w.width().unwrap_or(0);
            let height = w.height().unwrap_or(0);
            if title.trim().is_empty() || width < 40 || height < 40 {
                return None;
            }
            Some(WindowInfo {
                id: w.id().ok()?,
                title,
                app: w.app_name().unwrap_or_default(),
                width,
                height,
            })
        })
        .collect()
}

/// What to capture, in a form that can be stored and sent between threads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    /// One screen, by id.
    Monitor(u32),
    /// Every screen stitched into one picture.
    AllMonitors,
    /// One window, by id.
    Window(u32),
    /// Whichever window is in front when the capture happens.
    FocusedWindow,
    /// Part of one screen, in that screen's pixels.
    Region { monitor: u32, x: u32, y: u32, w: u32, h: u32 },
}

/// A capture source resolved on the thread that uses it.
pub enum Source {
    Monitor(Monitor),
    All(Vec<Monitor>),
    Window(Window),
    Region { monitor: Monitor, x: u32, y: u32, w: u32, h: u32 },
}

fn find_monitor(id: u32) -> Result<Monitor, String> {
    let monitors = Monitor::all().map_err(|e| format!("Couldn't list screens: {e}"))?;
    monitors
        .iter()
        .find(|m| m.id().ok() == Some(id))
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitors.first())
        .cloned()
        .ok_or_else(|| "No screen found".to_string())
}

impl Source {
    pub fn resolve(target: &Target) -> Result<Source, String> {
        match target {
            Target::Monitor(id) => Ok(Source::Monitor(find_monitor(*id)?)),
            Target::AllMonitors => {
                let monitors = Monitor::all().map_err(|e| format!("Couldn't list screens: {e}"))?;
                if monitors.is_empty() {
                    return Err("No screen found".into());
                }
                Ok(Source::All(monitors))
            }
            Target::Window(id) => {
                let windows = Window::all().map_err(|e| format!("Couldn't list windows: {e}"))?;
                let window = windows
                    .into_iter()
                    .find(|w| w.id().ok() == Some(*id))
                    .ok_or_else(|| "That window was closed. Pick another one.".to_string())?;
                if window.is_minimized().unwrap_or(false) {
                    return Err("That window is minimized. Restore it first.".into());
                }
                Ok(Source::Window(window))
            }
            Target::FocusedWindow => {
                let me = std::process::id();
                let windows = Window::all().map_err(|e| format!("Couldn't list windows: {e}"))?;
                let window = windows
                    .into_iter()
                    .find(|w| w.is_focused().unwrap_or(false) && w.pid().ok() != Some(me))
                    .ok_or_else(|| "No window is in front right now.".to_string())?;
                Ok(Source::Window(window))
            }
            Target::Region { monitor, x, y, w, h } => Ok(Source::Region {
                monitor: find_monitor(*monitor)?,
                x: *x,
                y: *y,
                w: (*w).max(1),
                h: (*h).max(1),
            }),
        }
    }

    pub fn capture(&self) -> Result<RgbaImage, String> {
        match self {
            Source::Monitor(m) => m.capture_image().map_err(|e| format!("Screen capture failed: {e}")),
            Source::Window(w) => w.capture_image().map_err(|e| format!("Window capture failed: {e}")),
            Source::Region { monitor, x, y, w, h } => {
                let mw = monitor.width().unwrap_or(*w);
                let mh = monitor.height().unwrap_or(*h);
                let x = (*x).min(mw.saturating_sub(1));
                let y = (*y).min(mh.saturating_sub(1));
                let w = (*w).min(mw - x).max(1);
                let h = (*h).min(mh - y).max(1);
                monitor
                    .capture_region(x, y, w, h)
                    .map_err(|e| format!("Area capture failed: {e}"))
            }
            Source::All(monitors) => stitch(monitors),
        }
    }

    /// Screen position (physical pixels) of the captured picture's top-left corner.
    pub fn origin(&self) -> (i32, i32) {
        match self {
            Source::Monitor(m) => (m.x().unwrap_or(0), m.y().unwrap_or(0)),
            Source::Window(w) => (w.x().unwrap_or(0), w.y().unwrap_or(0)),
            Source::Region { monitor, x, y, .. } => {
                (monitor.x().unwrap_or(0) + *x as i32, monitor.y().unwrap_or(0) + *y as i32)
            }
            Source::All(monitors) => (
                monitors.iter().map(|m| m.x().unwrap_or(0)).min().unwrap_or(0),
                monitors.iter().map(|m| m.y().unwrap_or(0)).min().unwrap_or(0),
            ),
        }
    }

    /// Display scale where the capture happens (for sizing the pointer).
    pub fn scale(&self) -> f32 {
        let s = match self {
            Source::Monitor(m) | Source::Region { monitor: m, .. } => m.scale_factor().unwrap_or(1.0),
            Source::Window(w) => w
                .current_monitor()
                .ok()
                .and_then(|m| m.scale_factor().ok())
                .unwrap_or(1.0),
            Source::All(monitors) => monitors
                .first()
                .and_then(|m| m.scale_factor().ok())
                .unwrap_or(1.0),
        };
        s.clamp(0.5, 4.0)
    }
}

fn stitch(monitors: &[Monitor]) -> Result<RgbaImage, String> {
    let rects: Vec<(i32, i32, u32, u32)> = monitors
        .iter()
        .map(|m| {
            (
                m.x().unwrap_or(0),
                m.y().unwrap_or(0),
                m.width().unwrap_or(0),
                m.height().unwrap_or(0),
            )
        })
        .collect();
    let left = rects.iter().map(|r| r.0).min().unwrap_or(0);
    let top = rects.iter().map(|r| r.1).min().unwrap_or(0);
    let right = rects.iter().map(|r| r.0 + r.2 as i32).max().unwrap_or(1);
    let bottom = rects.iter().map(|r| r.1 + r.3 as i32).max().unwrap_or(1);
    let mut canvas = RgbaImage::from_pixel(
        (right - left).max(1) as u32,
        (bottom - top).max(1) as u32,
        Rgba([0, 0, 0, 255]),
    );
    for (m, r) in monitors.iter().zip(&rects) {
        let shot = m.capture_image().map_err(|e| format!("Screen capture failed: {e}"))?;
        image::imageops::replace(&mut canvas, &shot, (r.0 - left) as i64, (r.1 - top) as i64);
    }
    Ok(canvas)
}

/// One-off capture of a target (used by screenshots and previews).
pub fn capture_target(target: &Target) -> Result<RgbaImage, String> {
    Source::resolve(target)?.capture()
}

// ---------------------------------------------------------------------------
// Pixel helpers
// ---------------------------------------------------------------------------

/// Shrink a picture by averaging every source pixel that falls into each
/// destination pixel. Fast enough for every video frame.
pub fn downscale(src: &RgbaImage, new_w: u32, new_h: u32) -> RgbaImage {
    let (sw, sh) = src.dimensions();
    let new_w = new_w.clamp(1, sw.max(1));
    let new_h = new_h.clamp(1, sh.max(1));
    if new_w == sw && new_h == sh {
        return src.clone();
    }
    if new_w * 2 == sw && new_h * 2 == sh {
        return half(src);
    }
    let raw = src.as_raw();
    let mut out = vec![0u8; (new_w * new_h * 4) as usize];
    // Column ranges are the same for every row, so work them out once.
    let cols: Vec<(usize, usize)> = (0..new_w)
        .map(|dx| {
            let x0 = (dx as u64 * sw as u64 / new_w as u64) as usize;
            let x1 = (((dx + 1) as u64 * sw as u64 / new_w as u64) as usize).max(x0 + 1);
            (x0, x1.min(sw as usize))
        })
        .collect();
    let mut acc = vec![[0u32; 4]; new_w as usize];
    for dy in 0..new_h {
        let y0 = (dy as u64 * sh as u64 / new_h as u64) as usize;
        let y1 = ((((dy + 1) as u64 * sh as u64 / new_h as u64) as usize).max(y0 + 1)).min(sh as usize);
        for a in acc.iter_mut() {
            *a = [0; 4];
        }
        for y in y0..y1 {
            let row = &raw[y * sw as usize * 4..(y + 1) * sw as usize * 4];
            for (dx, &(x0, x1)) in cols.iter().enumerate() {
                let a = &mut acc[dx];
                for x in x0..x1 {
                    let p = &row[x * 4..x * 4 + 4];
                    a[0] += p[0] as u32;
                    a[1] += p[1] as u32;
                    a[2] += p[2] as u32;
                    a[3] += p[3] as u32;
                }
            }
        }
        let rows = (y1 - y0) as u32;
        let base = dy as usize * new_w as usize * 4;
        for (dx, &(x0, x1)) in cols.iter().enumerate() {
            let n = rows * (x1 - x0) as u32;
            let a = acc[dx];
            let o = base + dx * 4;
            out[o] = (a[0] / n) as u8;
            out[o + 1] = (a[1] / n) as u8;
            out[o + 2] = (a[2] / n) as u8;
            out[o + 3] = (a[3] / n) as u8;
        }
    }
    RgbaImage::from_raw(new_w, new_h, out).expect("buffer size matches")
}

fn half(src: &RgbaImage) -> RgbaImage {
    let (sw, sh) = src.dimensions();
    let (w, h) = ((sw / 2).max(1), (sh / 2).max(1));
    let raw = src.as_raw();
    let stride = sw as usize * 4;
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h as usize {
        let r0 = &raw[(y * 2) * stride..(y * 2) * stride + stride];
        let r1 = &raw[(y * 2 + 1).min(sh as usize - 1) * stride..][..stride];
        let o = &mut out[y * w as usize * 4..(y + 1) * w as usize * 4];
        for x in 0..w as usize {
            for c in 0..4 {
                let s = r0[x * 8 + c] as u32
                    + r0[x * 8 + 4 + c] as u32
                    + r1[x * 8 + c] as u32
                    + r1[x * 8 + 4 + c] as u32;
                o[x * 4 + c] = (s / 4) as u8;
            }
        }
    }
    RgbaImage::from_raw(w, h, out).expect("buffer size matches")
}

/// Fit (w, h) inside max_w × max_h, keeping the shape.
pub fn fit_within(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w <= max_w && h <= max_h {
        return (w, h);
    }
    let s = (max_w as f32 / w as f32).min(max_h as f32 / h as f32);
    (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1))
}

// The classic arrow pointer: X = outline, o = fill.
const ARROW: [&str; 19] = [
    "X           ",
    "XX          ",
    "XoX         ",
    "XooX        ",
    "XoooX       ",
    "XooooX      ",
    "XoooooX     ",
    "XooooooX    ",
    "XoooooooX   ",
    "XooooooooX  ",
    "XoooooooooX ",
    "XooooooXXXXX",
    "XoooXooX    ",
    "XooXXooX    ",
    "XoX  XooX   ",
    "XX   XooX   ",
    "X     XooX  ",
    "      XooX  ",
    "       XX   ",
];

fn blend(img: &mut RgbaImage, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let a = color[3] as u32;
    for c in 0..3 {
        p.0[c] = ((color[c] as u32 * a + p.0[c] as u32 * (255 - a)) / 255) as u8;
    }
    p.0[3] = 255;
}

/// Draw the mouse pointer (and a ring when a button is held) onto a frame.
/// `x`, `y` are the pointer's position inside the picture.
pub fn draw_pointer(img: &mut RgbaImage, x: i32, y: i32, scale: f32, show_clicks: bool, p: &platform::Pointer) {
    let s = scale.max(1.0);
    if show_clicks && (p.left_down || p.right_down) {
        let r = 20.0 * s;
        let color = if p.left_down { [255, 200, 40, 110] } else { [90, 160, 255, 110] };
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= r {
                    blend(img, x + dx, y + dy, color);
                }
            }
        }
    }
    if !p.visible {
        return;
    }
    let h = ARROW.len() as f32 * s;
    let w = ARROW[0].len() as f32 * s;
    for py in 0..h.ceil() as i32 {
        let row = ARROW[((py as f32 / s) as usize).min(ARROW.len() - 1)].as_bytes();
        for px in 0..w.ceil() as i32 {
            let ch = row[((px as f32 / s) as usize).min(row.len() - 1)];
            match ch {
                b'X' => blend(img, x + px, y + py, [0, 0, 0, 255]),
                b'o' => blend(img, x + px, y + py, [255, 255, 255, 255]),
                _ => {}
            }
        }
    }
}

/// An egui texture-ready copy of a picture, no larger than `max_side`.
pub fn to_color_image(img: &RgbaImage, max_side: u32) -> eframe::egui::ColorImage {
    let (w, h) = fit_within(img.width(), img.height(), max_side, max_side);
    if (w, h) == img.dimensions() {
        eframe::egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw())
    } else {
        let small = downscale(img, w, h);
        eframe::egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], small.as_raw())
    }
}

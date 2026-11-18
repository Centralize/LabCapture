//! The gallery of past screenshots and recordings.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Video,
    Gif,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub path: PathBuf,
    pub name: String,
    pub kind: Kind,
    pub modified: SystemTime,
    pub size: u64,
}

impl Item {
    pub fn when(&self) -> String {
        let dt: chrono::DateTime<chrono::Local> = self.modified.into();
        let today = chrono::Local::now().date_naive();
        if dt.date_naive() == today {
            format!("Today {}", dt.format("%H:%M"))
        } else if Some(dt.date_naive()) == today.pred_opt() {
            format!("Yesterday {}", dt.format("%H:%M"))
        } else {
            dt.format("%d %b %Y").to_string()
        }
    }
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" => Some(Kind::Image),
        "mp4" => Some(Kind::Video),
        "gif" => Some(Kind::Gif),
        _ => None,
    }
}

/// Everything in the given folders, newest first.
pub fn scan(dirs: &[PathBuf]) -> Vec<Item> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !seen.insert(path.clone()) {
                continue;
            }
            let Some(kind) = kind_of(&path) else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            items.push(Item {
                name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                path,
                kind,
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                size: meta.len(),
            });
        }
    }
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    items.truncate(300);
    items
}

pub fn thumb_path_for(video: &Path) -> PathBuf {
    let name = video.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::settings::thumbs_dir().join(format!("{name}.png"))
}

/// Load a small preview picture for an item (runs on a worker thread).
pub fn load_thumb(item_path: &Path, kind: Kind) -> Option<eframe::egui::ColorImage> {
    let img = match kind {
        Kind::Image => image::open(item_path).ok()?.to_rgba8(),
        Kind::Video | Kind::Gif => image::open(thumb_path_for(item_path)).ok()?.to_rgba8(),
    };
    let (w, h) = crate::capture::fit_within(img.width(), img.height(), 480, 300);
    let small = image::imageops::thumbnail(&img, w, h);
    Some(eframe::egui::ColorImage::from_rgba_unmultiplied(
        [small.width() as usize, small.height() as usize],
        small.as_raw(),
    ))
}

pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB", b / 1024.0 / 1024.0 / 1024.0)
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

pub fn human_duration(d: std::time::Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

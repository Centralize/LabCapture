//! Choices the person makes, remembered between runs.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::capture::Target;
use crate::recorder::{Quality, VideoFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Screen,
    Window,
    Area,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenPick {
    Main,
    All,
    Id(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpg,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub shots_dir: PathBuf,
    pub videos_dir: PathBuf,
    pub image_format: ImageFormat,
    pub delay: u32,
    pub copy_after: bool,
    pub save_after: bool,
    pub show_after: bool,
    pub keep_out_of_captures: bool,
    pub hotkeys: bool,
    pub shot_mode: Mode,
    pub shot_screen: ScreenPick,
    pub video_mode: Mode,
    pub video_screen: ScreenPick,
    pub video_region: Option<Target>,
    pub fps: u32,
    pub format: VideoFormat,
    pub quality: Quality,
    pub half_size: bool,
    pub pointer: bool,
    pub clicks: bool,
    pub countdown: bool,
    pub live_preview: bool,
    pub system_audio: bool,
    pub mic_audio: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let pictures = dirs::picture_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let videos = dirs::video_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Settings {
            shots_dir: pictures.join("LabCapture"),
            videos_dir: videos.join("LabCapture"),
            image_format: ImageFormat::Png,
            delay: 0,
            copy_after: true,
            save_after: true,
            show_after: true,
            keep_out_of_captures: true,
            hotkeys: true,
            shot_mode: Mode::Screen,
            shot_screen: ScreenPick::Main,
            video_mode: Mode::Screen,
            video_screen: ScreenPick::Main,
            video_region: None,
            fps: 30,
            format: VideoFormat::Mp4,
            quality: Quality::Medium,
            half_size: false,
            pointer: true,
            clicks: true,
            countdown: true,
            live_preview: true,
            system_audio: true,
            mic_audio: false,
        }
    }
}

/// Where thumbnails of recordings are kept (outside the person's folders).
pub fn thumbs_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("LabCapture")
        .join("thumbs")
}

/// A file name like "Screenshot 2026-11-17 at 22.15.30.png" that isn't taken yet.
pub fn new_file_path(dir: &std::path::Path, prefix: &str, ext: &str) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y-%m-%d at %H.%M.%S");
    let base = format!("{prefix} {stamp}");
    let mut path = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} ({n}).{ext}"));
        n += 1;
    }
    path
}

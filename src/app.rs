//! App state and everything that happens off-screen: capture jobs, saving,
//! the clipboard, keyboard shortcuts, recordings and the library.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, ViewportCommand};
use image::RgbaImage;

use crate::annotate::{self, Mark, Tool};
use crate::capture::{self, MonitorInfo, Target, WindowInfo};
use crate::library::{self, Item};
use crate::platform;
use crate::recorder::{RecordResult, RecordSettings, Recording};
use crate::settings::{self, Mode, ScreenPick, Settings};

pub const SETTINGS_KEY: &str = "lapcapture-settings";
/// Largest texture side used to show a picture on screen.
pub const MAX_TEXTURE: u32 = 4096;
/// Frames drawn before the `--screenshot` picture is taken.
const SELF_SHOT_FRAMES: u64 = 120;

pub enum Msg {
    Shot { result: Result<RgbaImage, String>, purpose: Purpose },
    Saved { editor: u64, rev: u64, path: PathBuf, result: Result<(), String>, announce: bool },
    Copied(Result<(), String>),
    Preview { generation: u64, result: Result<ColorImage, String> },
    Thumb(PathBuf, Option<ColorImage>),
    Windows(Vec<WindowInfo>),
    Opened { path: PathBuf, result: Result<RgbaImage, String> },
    Hotkey(u32),
}

#[derive(Clone)]
pub enum Purpose {
    Screenshot { label: String },
    Freeze { monitor: MonitorInfo, for_video: bool, start_after: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Screenshot,
    Record,
    Library,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibFilter {
    All,
    Pictures,
    Videos,
}

#[derive(Clone, Debug)]
pub enum ShotKind {
    Screen,
    Window(u32),
    Focused,
    Area,
}

pub enum Pending {
    Idle,
    Countdown { until: Instant, kind: ShotKind },
    Hiding { since: Instant, kind: ShotKind },
    Working { hidden: bool },
}

pub struct Picker {
    pub monitor: MonitorInfo,
    pub image: Arc<RgbaImage>,
    pub texture: TextureHandle,
    pub for_video: bool,
    pub start_after: bool,
    pub drag_from: Option<egui::Pos2>,
    pub frames: u32,
}

pub enum PickOutcome {
    Open,
    Cancel,
    Done { x: u32, y: u32, w: u32, h: u32 },
}

pub struct Editor {
    pub id: u64,
    pub base: Arc<RgbaImage>,
    pub marks: Vec<Mark>,
    pub redo: Vec<Mark>,
    pub composite: Arc<RgbaImage>,
    pub texture: TextureHandle,
    pub path: Option<PathBuf>,
    pub rev: u64,
    pub saved_rev: Option<u64>,
    pub saving: bool,
    pub label: String,
    pub taken: chrono::DateTime<chrono::Local>,
}

impl Editor {
    fn new(ctx: &egui::Context, id: u64, image: RgbaImage, label: String) -> Editor {
        let base = Arc::new(image);
        let texture = ctx.load_texture(
            format!("editor-{id}"),
            capture::to_color_image(&base, MAX_TEXTURE),
            TextureOptions::LINEAR,
        );
        Editor {
            id,
            composite: base.clone(),
            base,
            marks: Vec::new(),
            redo: Vec::new(),
            texture,
            path: None,
            rev: 0,
            saved_rev: None,
            saving: false,
            label,
            taken: chrono::Local::now(),
        }
    }

    fn refresh(&mut self) {
        self.composite = Arc::new(annotate::render(&self.base, &self.marks));
        self.texture
            .set(capture::to_color_image(&self.composite, MAX_TEXTURE), TextureOptions::LINEAR);
        self.rev += 1;
    }

    pub fn add(&mut self, mark: Mark) {
        self.marks.push(mark);
        self.redo.clear();
        self.refresh();
    }

    pub fn undo(&mut self) {
        if let Some(m) = self.marks.pop() {
            self.redo.push(m);
            self.refresh();
        }
    }

    pub fn redo(&mut self) {
        if let Some(m) = self.redo.pop() {
            self.marks.push(m);
            self.refresh();
        }
    }

    pub fn clear(&mut self) {
        if !self.marks.is_empty() {
            self.redo.extend(self.marks.drain(..).rev());
            self.refresh();
        }
    }

    pub fn is_saved(&self) -> bool {
        self.saved_rev == Some(self.rev)
    }

    pub fn size(&self) -> (u32, u32) {
        self.base.dimensions()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub at: Instant,
}

pub struct Hotkeys {
    manager: global_hotkey::GlobalHotKeyManager,
    keys: Vec<global_hotkey::hotkey::HotKey>,
    pub screen: u32,
    pub area: u32,
    pub window: u32,
    pub record: u32,
}

struct SelfShot {
    path: PathBuf,
    requested: bool,
    done: Arc<Mutex<Option<Result<(), String>>>>,
    closing: bool,
}

pub struct LapApp {
    pub s: Settings,
    self_shot: Option<SelfShot>,
    pub tab: Tab,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub monitors: Vec<MonitorInfo>,
    monitors_at: Instant,
    pub windows: Vec<WindowInfo>,
    windows_at: Option<Instant>,
    windows_busy: bool,
    pub shot_window: Option<u32>,
    pub video_window: Option<u32>,
    hwnd: isize,
    excluded: bool,
    capture_in_flight: bool,

    pub pending: Pending,
    pub picker: Option<Picker>,
    pub editor: Option<Editor>,
    next_editor_id: u64,
    pub tool: Tool,
    pub color_idx: usize,
    pub thickness: f32,
    pub drawing: Option<Mark>,

    pub recording: Option<Recording>,
    pub rec_countdown: Option<(Instant, Target)>,
    pub last_video: Option<RecordResult>,
    pub preview: Option<TextureHandle>,
    preview_at: Option<Instant>,
    preview_busy: bool,
    preview_generation: u64,
    pub preview_error: Option<String>,

    pub lib_items: Vec<Item>,
    lib_scanned: Option<Instant>,
    pub lib_thumbs: HashMap<PathBuf, TextureHandle>,
    lib_requested: HashSet<PathBuf>,
    pub lib_filter: LibFilter,
    pub confirm_delete: Option<PathBuf>,

    pub toasts: Vec<Toast>,
    pub hotkeys: Option<Hotkeys>,
    pub hotkey_error: Option<String>,
    pub frames: u64,
}

pub const PALETTE: [[u8; 3]; 8] = [
    [255, 59, 48],
    [255, 149, 0],
    [255, 214, 10],
    [52, 199, 89],
    [10, 132, 255],
    [191, 90, 242],
    [255, 255, 255],
    [20, 20, 24],
];

impl LapApp {
    pub fn new(cc: &eframe::CreationContext<'_>, self_shot: Option<PathBuf>) -> LapApp {
        crate::theme::apply(&cc.egui_ctx);
        let s: Settings = if self_shot.is_some() {
            Settings::default()
        } else {
            cc.storage
                .and_then(|st| eframe::get_value(st, SETTINGS_KEY))
                .unwrap_or_default()
        };

        let hwnd = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
                _ => 0,
            }
        };

        let (tx, rx) = mpsc::channel();
        let mut app = LapApp {
            s,
            self_shot: self_shot.map(|path| SelfShot {
                path,
                requested: false,
                done: Arc::new(Mutex::new(None)),
                closing: false,
            }),
            tab: Tab::Screenshot,
            tx,
            rx,
            monitors: capture::list_monitors(),
            monitors_at: Instant::now(),
            windows: Vec::new(),
            windows_at: None,
            windows_busy: false,
            shot_window: None,
            video_window: None,
            hwnd,
            excluded: false,
            capture_in_flight: false,
            pending: Pending::Idle,
            picker: None,
            editor: None,
            next_editor_id: 1,
            tool: Tool::Arrow,
            color_idx: 0,
            thickness: 5.0,
            drawing: None,
            recording: None,
            rec_countdown: None,
            last_video: None,
            preview: None,
            preview_at: None,
            preview_busy: false,
            preview_generation: 0,
            preview_error: None,
            lib_items: Vec::new(),
            lib_scanned: None,
            lib_thumbs: HashMap::new(),
            lib_requested: HashSet::new(),
            lib_filter: LibFilter::All,
            confirm_delete: None,
            toasts: Vec::new(),
            hotkeys: None,
            hotkey_error: None,
            frames: 0,
        };
        if app.self_shot.is_none() && app.s.hotkeys {
            app.enable_hotkeys(&cc.egui_ctx);
        }
        app.rescan_library();
        if app.self_shot.is_some() {
            // For checking other pages with --screenshot.
            match std::env::var("LAPCAPTURE_TAB").unwrap_or_default().as_str() {
                "record" => app.tab = Tab::Record,
                "library" => app.tab = Tab::Library,
                "settings" => app.tab = Tab::Settings,
                _ => {}
            }
            if let Ok(path) = std::env::var("LAPCAPTURE_OPEN") {
                app.open_in_editor(PathBuf::from(path));
            }
        }
        app
    }

    pub fn is_self_shot(&self) -> bool {
        self.self_shot.is_some()
    }

    // -----------------------------------------------------------------------
    // Messages and timers (runs every frame, also while the window is hidden)
    // -----------------------------------------------------------------------

    pub fn tick(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle(ctx, msg);
        }

        let now = Instant::now();
        match &self.pending {
            Pending::Countdown { until, kind } => {
                if now >= *until {
                    let kind = kind.clone();
                    self.pending = Pending::Idle;
                    self.begin_shot(ctx, kind);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
            Pending::Hiding { since, kind } => {
                if now.duration_since(*since) >= Duration::from_millis(300) {
                    let kind = kind.clone();
                    self.spawn_capture(ctx, kind, true);
                } else {
                    ctx.request_repaint_after(Duration::from_millis(30));
                }
            }
            _ => {}
        }

        if let Some((at, target)) = &self.rec_countdown {
            if now >= *at {
                let target = target.clone();
                self.rec_countdown = None;
                self.begin_recording(ctx, target);
            } else {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
        }

        if let Some(rec) = &mut self.recording {
            if let Some(result) = rec.poll() {
                self.recording = None;
                match result {
                    Ok(r) => {
                        let mut text = format!(
                            "Recording saved · {} · {}",
                            library::human_duration(r.duration),
                            library::human_size(r.bytes)
                        );
                        if let Some(w) = &r.warning {
                            text.push_str(&format!(" ({w})"));
                        }
                        self.toast(text, ToastKind::Success);
                        self.last_video = Some(r);
                    }
                    Err(e) => self.toast(format!("Recording failed: {e}"), ToastKind::Error),
                }
                self.rescan_library();
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }

        if now.duration_since(self.monitors_at) > Duration::from_secs(5) {
            self.monitors = capture::list_monitors();
            self.monitors_at = now;
        }

        let want_windows = (self.tab == Tab::Screenshot && self.s.shot_mode == Mode::Window)
            || (self.tab == Tab::Record && self.s.video_mode == Mode::Window);
        if want_windows && !self.windows_busy && self.windows_at.is_none_or(|t| now.duration_since(t) > Duration::from_secs(3)) {
            self.refresh_windows(ctx);
        }

        if self.tab == Tab::Library
            && self.lib_scanned.is_none_or(|t| now.duration_since(t) > Duration::from_secs(4))
        {
            self.rescan_library();
        }

        self.update_preview(ctx);
        self.apply_exclusion();
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_millis(4200));
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn handle(&mut self, ctx: &egui::Context, msg: Msg) {
        match msg {
            Msg::Shot { result, purpose } => {
                let hidden = matches!(self.pending, Pending::Working { hidden: true });
                self.pending = Pending::Idle;
                self.capture_in_flight = false;
                if hidden {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                }
                match (result, purpose) {
                    (Err(e), _) => {
                        self.toast(e, ToastKind::Error);
                    }
                    (Ok(img), Purpose::Screenshot { label }) => self.accept_screenshot(ctx, img, label, None),
                    (Ok(img), Purpose::Freeze { monitor, for_video, start_after }) => {
                        let texture = ctx.load_texture(
                            "area-picker",
                            capture::to_color_image(&img, MAX_TEXTURE),
                            TextureOptions::LINEAR,
                        );
                        self.picker = Some(Picker {
                            monitor,
                            image: Arc::new(img),
                            texture,
                            for_video,
                            start_after,
                            drag_from: None,
                            frames: 0,
                        });
                    }
                }
            }
            Msg::Saved { editor, rev, path, result, announce } => {
                if let Some(ed) = self.editor.as_mut().filter(|e| e.id == editor) {
                    ed.saving = false;
                    if result.is_ok() {
                        ed.path = Some(path.clone());
                        ed.saved_rev = Some(rev);
                    }
                }
                match result {
                    Ok(()) => {
                        if announce {
                            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            self.toast(format!("Saved {name}"), ToastKind::Success);
                        }
                        let thumb = path.clone();
                        self.lib_thumbs.remove(&thumb);
                        self.lib_requested.remove(&thumb);
                        self.rescan_library();
                    }
                    Err(e) => self.toast(format!("Couldn't save: {e}"), ToastKind::Error),
                }
            }
            Msg::Copied(result) => match result {
                Ok(()) => {}
                Err(e) => self.toast(format!("Couldn't copy to the clipboard: {e}"), ToastKind::Error),
            },
            Msg::Preview { generation, result } => {
                self.preview_busy = false;
                if generation != self.preview_generation {
                    return;
                }
                match result {
                    Ok(img) => {
                        self.preview_error = None;
                        match &mut self.preview {
                            Some(t) if t.size() == img.size => t.set(img, TextureOptions::LINEAR),
                            _ => self.preview = Some(ctx.load_texture("preview", img, TextureOptions::LINEAR)),
                        }
                    }
                    Err(e) => {
                        self.preview_error = Some(e);
                        self.preview = None;
                    }
                }
            }
            Msg::Thumb(path, img) => {
                if let Some(img) = img {
                    let tex = ctx.load_texture(format!("thumb-{}", path.display()), img, TextureOptions::LINEAR);
                    self.lib_thumbs.insert(path, tex);
                }
            }
            Msg::Windows(list) => {
                self.windows_busy = false;
                self.windows_at = Some(Instant::now());
                self.windows = list;
            }
            Msg::Opened { path, result } => match result {
                Ok(img) => {
                    let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.accept_screenshot(ctx, img, label, Some(path));
                }
                Err(e) => self.toast(e, ToastKind::Error),
            },
            Msg::Hotkey(id) => {
                let Some(hk) = &self.hotkeys else { return };
                let (screen, area, window, record) = (hk.screen, hk.area, hk.window, hk.record);
                if id == screen {
                    self.request_shot(ctx, ShotKind::Screen);
                } else if id == area {
                    self.request_shot(ctx, ShotKind::Area);
                } else if id == window {
                    self.request_shot(ctx, ShotKind::Focused);
                } else if id == record {
                    self.toggle_recording(ctx);
                }
            }
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast { text: text.into(), kind, at: Instant::now() });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    // -----------------------------------------------------------------------
    // Keeping our own window out of captures
    // -----------------------------------------------------------------------

    fn apply_exclusion(&mut self) {
        if self.self_shot.is_some() || !platform::can_exclude_from_capture() {
            return;
        }
        let want = self.s.keep_out_of_captures
            && (self.capture_in_flight
                || self.picker.is_some()
                || self.recording.is_some()
                || self.rec_countdown.is_some()
                || (self.tab == Tab::Record && self.s.live_preview));
        if want != self.excluded {
            platform::exclude_from_capture(self.hwnd, want);
            self.excluded = want;
        }
    }

    // -----------------------------------------------------------------------
    // Screenshots
    // -----------------------------------------------------------------------

    pub fn busy(&self) -> bool {
        !matches!(self.pending, Pending::Idle) || self.picker.is_some()
    }

    pub fn request_shot(&mut self, ctx: &egui::Context, kind: ShotKind) {
        if self.busy() {
            return;
        }
        if let ShotKind::Window(_) = kind {
            // fine
        }
        if self.s.delay > 0 {
            self.pending = Pending::Countdown {
                until: Instant::now() + Duration::from_secs(self.s.delay as u64),
                kind,
            };
            ctx.request_repaint();
        } else {
            self.begin_shot(ctx, kind);
        }
    }

    pub fn cancel_countdown(&mut self) {
        if matches!(self.pending, Pending::Countdown { .. }) {
            self.pending = Pending::Idle;
        }
        self.rec_countdown = None;
    }

    fn begin_shot(&mut self, ctx: &egui::Context, kind: ShotKind) {
        let must_hide = self.s.keep_out_of_captures && !platform::can_exclude_from_capture();
        if must_hide {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.pending = Pending::Hiding { since: Instant::now(), kind };
            ctx.request_repaint_after(Duration::from_millis(30));
        } else {
            self.spawn_capture(ctx, kind, false);
        }
    }

    pub fn monitor_for(&self, pick: ScreenPick) -> Option<&MonitorInfo> {
        match pick {
            ScreenPick::Id(id) => self.monitors.iter().find(|m| m.id == id).or_else(|| self.monitors.first()),
            _ => self.monitors.iter().find(|m| m.primary).or_else(|| self.monitors.first()),
        }
    }

    fn monitor_under_pointer(&self) -> Option<MonitorInfo> {
        let p = platform::pointer()?;
        self.monitors
            .iter()
            .find(|m| p.x >= m.x && p.y >= m.y && p.x < m.x + m.width as i32 && p.y < m.y + m.height as i32)
            .cloned()
    }

    fn spawn_capture(&mut self, ctx: &egui::Context, kind: ShotKind, hidden: bool) {
        self.monitors = capture::list_monitors();
        self.monitors_at = Instant::now();
        let (target, purpose) = match kind {
            ShotKind::Screen => {
                let target = match self.s.shot_screen {
                    ScreenPick::All if self.monitors.len() > 1 => Target::AllMonitors,
                    pick => match self.monitor_for(pick) {
                        Some(m) => Target::Monitor(m.id),
                        None => Target::AllMonitors,
                    },
                };
                let label = if target == Target::AllMonitors { "All screens" } else { "Screen" };
                (target, Purpose::Screenshot { label: label.into() })
            }
            ShotKind::Window(id) => {
                let label = self
                    .windows
                    .iter()
                    .find(|w| w.id == id)
                    .map(|w| capture::shorten(&w.title, 40))
                    .unwrap_or_else(|| "Window".into());
                (Target::Window(id), Purpose::Screenshot { label })
            }
            ShotKind::Focused => (Target::FocusedWindow, Purpose::Screenshot { label: "Front window".into() }),
            ShotKind::Area => {
                let monitor = self
                    .monitor_under_pointer()
                    .or_else(|| self.monitor_for(self.s.shot_screen).cloned());
                let Some(monitor) = monitor else {
                    self.pending = Pending::Idle;
                    self.toast("No screen found", ToastKind::Error);
                    return;
                };
                (
                    Target::Monitor(monitor.id),
                    Purpose::Freeze { monitor, for_video: false, start_after: false },
                )
            }
        };
        self.launch_capture(ctx, target, purpose, hidden);
    }

    fn launch_capture(&mut self, ctx: &egui::Context, target: Target, purpose: Purpose, hidden: bool) {
        self.pending = Pending::Working { hidden };
        self.capture_in_flight = true;
        self.apply_exclusion();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Give the compositor a moment to drop our window from captures.
            std::thread::sleep(Duration::from_millis(90));
            let result = capture::capture_target(&target);
            let _ = tx.send(Msg::Shot { result, purpose });
            ctx.request_repaint();
        });
    }

    pub fn start_area_pick(&mut self, ctx: &egui::Context, for_video: bool, start_after: bool) {
        if self.busy() {
            return;
        }
        let monitor = if for_video {
            self.monitor_for(self.s.video_screen).cloned()
        } else {
            self.monitor_under_pointer().or_else(|| self.monitor_for(self.s.shot_screen).cloned())
        };
        let Some(monitor) = monitor else {
            self.toast("No screen found", ToastKind::Error);
            return;
        };
        let target = Target::Monitor(monitor.id);
        self.launch_capture(ctx, target, Purpose::Freeze { monitor, for_video, start_after }, false);
    }

    pub fn finish_pick(&mut self, ctx: &egui::Context, outcome: PickOutcome) {
        let Some(picker) = self.picker.take() else { return };
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Focus);
        match outcome {
            PickOutcome::Open => self.picker = Some(picker),
            PickOutcome::Cancel => {}
            PickOutcome::Done { x, y, w, h } => {
                if picker.for_video {
                    self.s.video_region = Some(Target::Region { monitor: picker.monitor.id, x, y, w, h });
                    self.s.video_mode = Mode::Area;
                    self.preview_at = None;
                    if picker.start_after {
                        self.start_recording(ctx);
                    }
                } else {
                    let cropped = image::imageops::crop_imm(&*picker.image, x, y, w, h).to_image();
                    self.accept_screenshot(ctx, cropped, "Area".into(), None);
                }
            }
        }
    }

    fn accept_screenshot(&mut self, ctx: &egui::Context, img: RgbaImage, label: String, from_file: Option<PathBuf>) {
        let id = self.next_editor_id;
        self.next_editor_id += 1;
        let mut editor = Editor::new(ctx, id, img, label);
        self.drawing = None;
        self.tab = Tab::Screenshot;
        let opened = from_file.is_some();
        if let Some(path) = from_file {
            editor.path = Some(path);
            editor.saved_rev = Some(editor.rev);
        }
        self.editor = Some(editor);
        if opened {
            return;
        }
        let mut done = Vec::new();
        if self.s.save_after {
            self.save_editor(None, false);
            done.push("saved");
        }
        if self.s.copy_after {
            self.copy_editor();
            done.push("copied to the clipboard");
        }
        let text = if done.is_empty() {
            "Screenshot taken".to_string()
        } else {
            format!("Screenshot {}", done.join(" and "))
        };
        self.toast(text, ToastKind::Success);
        if self.s.show_after {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Focus);
        }
    }

    /// Save the current screenshot (with its markup). `to` picks a new file;
    /// otherwise it goes to the file it came from or a new one in the folder.
    pub fn save_editor(&mut self, to: Option<PathBuf>, announce: bool) {
        let format = self.s.image_format;
        let dir = self.s.shots_dir.clone();
        let Some(ed) = self.editor.as_mut() else { return };
        let path = to
            .or_else(|| ed.path.clone())
            .unwrap_or_else(|| settings::new_file_path(&dir, "Screenshot", format.extension()));
        ed.saving = true;
        let img = ed.composite.clone();
        let (editor, rev) = (ed.id, ed.rev);
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = save_image(&img, &path);
            let _ = tx.send(Msg::Saved { editor, rev, path, result, announce });
        });
    }

    pub fn copy_editor(&mut self) {
        let Some(ed) = self.editor.as_ref() else { return };
        let img = ed.composite.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = copy_image(&img);
            let _ = tx.send(Msg::Copied(result));
        });
    }

    pub fn open_in_editor(&mut self, path: PathBuf) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = image::open(&path)
                .map(|i| i.to_rgba8())
                .map_err(|e| format!("Couldn't open the picture: {e}"));
            let _ = tx.send(Msg::Opened { path, result });
        });
    }

    pub fn current_color(&self) -> [u8; 4] {
        let c = PALETTE[self.color_idx.min(PALETTE.len() - 1)];
        [c[0], c[1], c[2], 255]
    }

    // -----------------------------------------------------------------------
    // Windows list
    // -----------------------------------------------------------------------

    pub fn refresh_windows(&mut self, ctx: &egui::Context) {
        if self.windows_busy {
            return;
        }
        self.windows_busy = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let list = capture::list_windows();
            let _ = tx.send(Msg::Windows(list));
            ctx.request_repaint();
        });
    }

    // -----------------------------------------------------------------------
    // Recording
    // -----------------------------------------------------------------------

    pub fn toggle_recording(&mut self, ctx: &egui::Context) {
        if let Some(rec) = &mut self.recording {
            if !rec.is_stopping() {
                rec.stop();
            }
        } else if self.rec_countdown.is_some() {
            self.rec_countdown = None;
        } else {
            self.start_recording(ctx);
        }
    }

    pub fn video_target(&self) -> Result<Target, String> {
        match self.s.video_mode {
            Mode::Screen => self
                .monitor_for(self.s.video_screen)
                .map(|m| Target::Monitor(m.id))
                .ok_or_else(|| "No screen found".to_string()),
            Mode::Window => self
                .video_window
                .map(Target::Window)
                .ok_or_else(|| "Pick a window to record first.".to_string()),
            Mode::Area => self
                .s
                .video_region
                .clone()
                .ok_or_else(|| "Choose an area to record first.".to_string()),
        }
    }

    pub fn start_recording(&mut self, ctx: &egui::Context) {
        if self.recording.is_some() || self.rec_countdown.is_some() {
            return;
        }
        let target = match self.video_target() {
            Ok(t) => t,
            Err(e) => {
                if self.s.video_mode == Mode::Area {
                    self.start_area_pick(ctx, true, true);
                } else {
                    self.tab = Tab::Record;
                    self.toast(e, ToastKind::Error);
                }
                return;
            }
        };
        if self.s.countdown {
            self.rec_countdown = Some((Instant::now() + Duration::from_secs(3), target));
            ctx.request_repaint();
        } else {
            self.begin_recording(ctx, target);
        }
    }

    fn begin_recording(&mut self, ctx: &egui::Context, target: Target) {
        let ext = self.s.format.extension();
        if let Err(e) = std::fs::create_dir_all(&self.s.videos_dir) {
            self.toast(format!("Couldn't create the videos folder: {e}"), ToastKind::Error);
            return;
        }
        let path = settings::new_file_path(&self.s.videos_dir, "Recording", ext);
        let settings = RecordSettings {
            target,
            fps: self.s.fps,
            format: self.s.format,
            half_size: self.s.half_size,
            quality: self.s.quality,
            pointer: self.s.pointer,
            clicks: self.s.clicks,
            thumb_path: Some(library::thumb_path_for(&path)),
            path,
            system_audio: self.s.system_audio,
            mic: self.s.mic_audio,
        };
        self.preview_generation += 1;
        self.preview_busy = false;
        self.recording = Some(Recording::start(settings, ctx.clone()));
        self.last_video = None;
        self.apply_exclusion();
        ctx.request_repaint();
    }

    fn update_preview(&mut self, ctx: &egui::Context) {
        if self.tab != Tab::Record
            || !self.s.live_preview
            || self.recording.is_some()
            || self.preview_busy
            || self.self_shot.is_some()
        {
            return;
        }
        if self.preview_at.is_some_and(|t| t.elapsed() < Duration::from_millis(900)) {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        }
        let target = match self.video_target() {
            Ok(t) => t,
            Err(e) => {
                self.preview = None;
                self.preview_error = Some(e);
                self.preview_at = Some(Instant::now());
                return;
            }
        };
        self.preview_at = Some(Instant::now());
        self.preview_busy = true;
        self.preview_generation += 1;
        let generation = self.preview_generation;
        let pointer = self.s.pointer;
        let tx = self.tx.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let result = capture::Source::resolve(&target).and_then(|src| {
                let mut img = src.capture()?;
                if pointer {
                    if let Some(p) = platform::pointer() {
                        let (ox, oy) = src.origin();
                        capture::draw_pointer(&mut img, p.x - ox, p.y - oy, src.scale(), false, &p);
                    }
                }
                Ok(capture::to_color_image(&img, 1280))
            });
            let _ = tx.send(Msg::Preview { generation, result });
            ctx2.request_repaint();
        });
        ctx.request_repaint_after(Duration::from_millis(300));
    }

    // -----------------------------------------------------------------------
    // Library
    // -----------------------------------------------------------------------

    pub fn rescan_library(&mut self) {
        self.lib_items = library::scan(&[self.s.shots_dir.clone(), self.s.videos_dir.clone()]);
        self.lib_scanned = Some(Instant::now());
    }

    /// Ask for a thumbnail to be loaded if we don't have one yet.
    pub fn want_thumb(&mut self, ctx: &egui::Context, item: &Item) {
        if self.lib_thumbs.contains_key(&item.path) || !self.lib_requested.insert(item.path.clone()) {
            return;
        }
        let tx = self.tx.clone();
        let path = item.path.clone();
        let kind = item.kind;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let img = library::load_thumb(&path, kind);
            let _ = tx.send(Msg::Thumb(path, img));
            ctx.request_repaint();
        });
    }

    pub fn delete_file(&mut self, path: &PathBuf) {
        match std::fs::remove_file(path) {
            Ok(()) => {
                let _ = std::fs::remove_file(library::thumb_path_for(path));
                self.lib_thumbs.remove(path);
                if self.editor.as_ref().and_then(|e| e.path.as_ref()) == Some(path) {
                    if let Some(ed) = &mut self.editor {
                        ed.path = None;
                        ed.saved_rev = None;
                    }
                }
                self.toast("Deleted", ToastKind::Info);
            }
            Err(e) => self.toast(format!("Couldn't delete: {e}"), ToastKind::Error),
        }
        self.rescan_library();
    }

    // -----------------------------------------------------------------------
    // Keyboard shortcuts that work anywhere
    // -----------------------------------------------------------------------

    pub fn enable_hotkeys(&mut self, ctx: &egui::Context) {
        use global_hotkey::hotkey::{Code, HotKey, Modifiers};
        use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
        if self.hotkeys.is_some() {
            return;
        }
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                self.hotkey_error = Some(format!("Shortcuts are unavailable: {e}"));
                return;
            }
        };
        let mods = Some(Modifiers::ALT | Modifiers::SHIFT);
        let keys = vec![
            HotKey::new(mods, Code::KeyS),
            HotKey::new(mods, Code::KeyA),
            HotKey::new(mods, Code::KeyW),
            HotKey::new(mods, Code::KeyR),
        ];
        let mut failed = Vec::new();
        let mut registered = Vec::new();
        for k in &keys {
            match manager.register(*k) {
                Ok(()) => registered.push(*k),
                Err(_) => failed.push(k.into_string()),
            }
        }
        self.hotkey_error = if failed.is_empty() {
            None
        } else {
            Some(format!("Another app already uses: {}", failed.join(", ")))
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                let _ = tx.send(Msg::Hotkey(e.id));
                ctx.request_repaint();
            }
        }));
        self.hotkeys = Some(Hotkeys {
            manager,
            screen: keys[0].id(),
            area: keys[1].id(),
            window: keys[2].id(),
            record: keys[3].id(),
            keys: registered,
        });
    }

    pub fn disable_hotkeys(&mut self) {
        if let Some(hk) = self.hotkeys.take() {
            let _ = hk.manager.unregister_all(&hk.keys);
        }
        global_hotkey::GlobalHotKeyEvent::set_event_handler(None::<fn(global_hotkey::GlobalHotKeyEvent)>);
        self.hotkey_error = None;
    }

    // -----------------------------------------------------------------------
    // The `--screenshot` option
    // -----------------------------------------------------------------------

    pub fn self_shot_step(&mut self, ctx: &egui::Context) {
        let frames = self.frames;
        let Some(shot) = &mut self.self_shot else { return };
        // Keep drawing even without focus, or the picture never comes.
        ctx.request_repaint();
        if shot.closing {
            return;
        }
        if frames >= SELF_SHOT_FRAMES && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |image: Arc<ColorImage>| {
                let result = save_color_image(&image, &path);
                if let Err(e) = &result {
                    eprintln!("lapcapture: couldn't save the screenshot: {e}");
                }
                *done.lock().unwrap() = Some(result);
                ctx2.request_repaint();
            });
        }
        let finished = shot.done.lock().unwrap().is_some();
        if finished {
            shot.closing = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    pub fn save_settings(&self, storage: &mut dyn eframe::Storage) {
        if self.self_shot.is_none() {
            eframe::set_value(storage, SETTINGS_KEY, &self.s);
        }
    }

    /// Closing the app while recording: finish the file properly first.
    pub fn finish_recording_before_exit(&mut self) {
        if let Some(rec) = &mut self.recording {
            rec.stop();
            let until = Instant::now() + Duration::from_secs(15);
            while Instant::now() < until {
                if rec.poll().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
        }
        self.recording = None;
        self.disable_hotkeys();
    }

    /// Ask for a fresh live preview on the next frame.
    pub fn reset_preview_timer(&mut self) {
        self.preview_at = None;
        self.preview_generation += 1;
        self.preview_busy = false;
    }
}

fn save_image(img: &RgbaImage, path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    if ext == "jpg" || ext == "jpeg" {
        let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::new(file);
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, 92);
        rgb.write_with_encoder(enc).map_err(|e| e.to_string())
    } else {
        img.save_with_format(path, image::ImageFormat::Png).map_err(|e| e.to_string())
    }
}

fn copy_image(img: &RgbaImage) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: std::borrow::Cow::Borrowed(img.as_raw()),
        })
        .map_err(|e| e.to_string())
}

fn save_color_image(image: &ColorImage, path: &std::path::Path) -> Result<(), String> {
    let [w, h] = image.size;
    let mut raw = Vec::with_capacity(w * h * 4);
    for p in &image.pixels {
        let [r, g, b, a] = p.to_srgba_unmultiplied();
        raw.extend_from_slice(&[r, g, b, a]);
    }
    let img = RgbaImage::from_raw(w as u32, h as u32, raw).ok_or("bad screenshot size")?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    img.save_with_format(path, image::ImageFormat::Png).map_err(|e| e.to_string())
}

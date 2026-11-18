//! Screen recording: one thread grabs frames at a steady rate, another
//! encodes them to MP4 (H.264) or an animated GIF.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::capture::{self, Source, Target};
use crate::platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    Gif,
}

impl VideoFormat {
    pub fn extension(self) -> &'static str {
        match self {
            VideoFormat::Mp4 => "mp4",
            VideoFormat::Gif => "gif",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    /// Bits per pixel per frame for the H.264 encoder.
    fn bits_per_pixel(self) -> f32 {
        match self {
            Quality::Low => 0.05,
            Quality::Medium => 0.1,
            Quality::High => 0.2,
        }
    }
    /// NeuQuant sampling speed for GIF (1 = best, 30 = fastest).
    fn gif_speed(self) -> i32 {
        match self {
            Quality::Low => 30,
            Quality::Medium => 20,
            Quality::High => 10,
        }
    }
}

/// GIFs bigger than this are scaled down; they get huge and slow otherwise.
pub const GIF_MAX_SIDE: u32 = 1280;

#[derive(Clone, Debug)]
pub struct RecordSettings {
    pub target: Target,
    pub fps: u32,
    pub format: VideoFormat,
    pub half_size: bool,
    pub quality: Quality,
    pub pointer: bool,
    pub clicks: bool,
    pub path: PathBuf,
    pub thumb_path: Option<PathBuf>,
    /// Record what the computer plays (MP4 only).
    pub system_audio: bool,
    /// Record the microphone (MP4 only).
    pub mic: bool,
}

/// The recording's clock: time since the first frame, not counting pauses.
/// Video and sound both read it, so they stay together.
#[derive(Default)]
pub struct Clock {
    state: Mutex<ClockState>,
}

#[derive(Default)]
struct ClockState {
    start: Option<Instant>,
    paused_total: Duration,
    pause_began: Option<Instant>,
}

impl Clock {
    pub fn start(&self) {
        if let Ok(mut s) = self.state.lock() {
            if s.start.is_none() {
                s.start = Some(Instant::now());
            }
        }
    }

    /// Recording time now, or None before the first frame.
    pub fn now(&self) -> Option<Duration> {
        let s = self.state.lock().ok()?;
        let start = s.start?;
        let paused_now = s.pause_began.map(|p| p.elapsed()).unwrap_or_default();
        Some(start.elapsed().saturating_sub(s.paused_total + paused_now))
    }

    pub fn set_paused(&self, paused: bool) {
        if let Ok(mut s) = self.state.lock() {
            match (paused, s.pause_began) {
                (true, None) => s.pause_began = Some(Instant::now()),
                (false, Some(began)) => {
                    s.paused_total += began.elapsed();
                    s.pause_began = None;
                }
                _ => {}
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.state.lock().map(|s| s.pause_began.is_some()).unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecStats {
    pub captured: u64,
    pub dropped: u64,
    pub encoded: u64,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    /// Recorded time, not counting pauses.
    pub elapsed: Duration,
    /// Whether the fast graphics-card capture is in use.
    pub fast_capture: bool,
    /// Which encoder writes the file.
    pub encoder: &'static str,
    /// Whether sound is being recorded.
    pub audio: bool,
}

#[derive(Clone, Debug)]
pub struct RecordResult {
    pub path: PathBuf,
    pub duration: Duration,
    pub frames: u64,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub warning: Option<String>,
}

pub struct Recording {
    pub settings: RecordSettings,
    stop: Arc<AtomicBool>,
    clock: Arc<Clock>,
    stats: Arc<Mutex<RecStats>>,
    done: Receiver<Result<RecordResult, String>>,
    finished: Option<Result<RecordResult, String>>,
    stopping: bool,
}

struct Frame {
    image: RgbaImage,
    at: Duration,
}

/// Sound coming from the audio thread, and whether it managed to start.
struct AudioFeed {
    chunks: Receiver<crate::audio::AudioChunk>,
    ready: Receiver<Result<Option<String>, String>>,
    capture: crate::audio::AudioCapture,
}

impl Recording {
    pub fn start(settings: RecordSettings, ctx: eframe::egui::Context) -> Recording {
        let stop = Arc::new(AtomicBool::new(false));
        let clock = Arc::new(Clock::default());
        let stats = Arc::new(Mutex::new(RecStats::default()));
        let capture_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let (done_tx, done_rx) = mpsc::channel();
        // A short queue: if encoding falls behind, frames are dropped rather
        // than piling up in memory. Timestamps keep the video in real time.
        let (frame_tx, frame_rx) = mpsc::sync_channel::<Frame>(4);

        // Sound goes into MP4 files through Windows' encoder.
        let wants_audio =
            cfg!(windows) && settings.format == VideoFormat::Mp4 && (settings.system_audio || settings.mic);
        let audio = wants_audio.then(|| {
            let (tx, chunks) = mpsc::channel();
            let (ready_tx, ready) = mpsc::sync_channel(1);
            let capture = crate::audio::start(settings.system_audio, settings.mic, clock.clone(), tx, ready_tx);
            AudioFeed { chunks, ready, capture }
        });

        {
            let settings = settings.clone();
            let stop = stop.clone();
            let clock = clock.clone();
            let stats = stats.clone();
            let capture_error = capture_error.clone();
            let ctx = ctx.clone();
            thread::Builder::new()
                .name("lapcapture-grab".into())
                .spawn(move || {
                    if let Err(e) = grab_loop(&settings, &stop, &clock, &stats, frame_tx) {
                        *capture_error.lock().unwrap() = Some(e);
                        stop.store(true, Ordering::SeqCst);
                    }
                    ctx.request_repaint();
                })
                .expect("spawn capture thread");
        }
        {
            let settings = settings.clone();
            let stats = stats.clone();
            thread::Builder::new()
                .name("lapcapture-encode".into())
                .spawn(move || {
                    let result = encode_loop(&settings, frame_rx, audio, &stats);
                    let capture_err = capture_error.lock().unwrap().clone();
                    let result = match (result, capture_err) {
                        (Ok(mut r), err) if r.frames > 0 => {
                            r.warning = match (err, r.warning.take()) {
                                (Some(a), Some(b)) => Some(format!("{a}; {b}")),
                                (a, b) => a.or(b),
                            };
                            Ok(r)
                        }
                        (Ok(_), Some(err)) | (Err(err), _) => {
                            let _ = std::fs::remove_file(&settings.path);
                            Err(err)
                        }
                        (Ok(_), None) => {
                            let _ = std::fs::remove_file(&settings.path);
                            Err("Nothing was recorded.".into())
                        }
                    };
                    let _ = done_tx.send(result);
                    ctx.request_repaint();
                })
                .expect("spawn encoder thread");
        }

        Recording {
            settings,
            stop,
            clock,
            stats,
            done: done_rx,
            finished: None,
            stopping: false,
        }
    }

    pub fn stop(&mut self) {
        self.stopping = true;
        self.clock.set_paused(false);
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn is_stopping(&self) -> bool {
        self.stopping || self.stop.load(Ordering::SeqCst)
    }

    pub fn toggle_pause(&self) {
        self.clock.set_paused(!self.clock.is_paused());
    }

    pub fn is_paused(&self) -> bool {
        self.clock.is_paused()
    }

    pub fn stats(&self) -> RecStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The final result once both threads have finished.
    pub fn poll(&mut self) -> Option<Result<RecordResult, String>> {
        if self.finished.is_none() {
            if let Ok(r) = self.done.try_recv() {
                self.finished = Some(r);
            }
        }
        self.finished.take()
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn grab_loop(
    settings: &RecordSettings,
    stop: &AtomicBool,
    clock: &Clock,
    stats: &Mutex<RecStats>,
    tx: SyncSender<Frame>,
) -> Result<(), String> {
    let source = Source::resolve(&settings.target)?;
    // Let the compositor drop our own window from captures first.
    thread::sleep(Duration::from_millis(120));

    // Whole screens and areas use the fast graphics-card path when it works.
    #[cfg(windows)]
    let mut fast = match &source {
        Source::Monitor(m) => crate::dxgi::Duplicator::new(m.x().unwrap_or(0), m.y().unwrap_or(0), None).ok(),
        Source::Region { monitor, x, y, w, h } => {
            crate::dxgi::Duplicator::new(monitor.x().unwrap_or(0), monitor.y().unwrap_or(0), Some((*x, *y, *w, *h))).ok()
        }
        _ => None,
    };
    #[cfg(windows)]
    if let Some(f) = fast.as_mut() {
        match source.capture() {
            Ok(img) => f.seed(img),
            Err(_) => fast = None,
        }
    }
    #[cfg(windows)]
    if let Ok(mut s) = stats.lock() {
        s.fast_capture = fast.is_some();
    }
    let (origin, scale) = (source.origin(), source.scale());
    let fixed_origin = !matches!(source, Source::Window(_));

    let interval = Duration::from_secs_f64(1.0 / settings.fps.clamp(1, 120) as f64);
    clock.start();
    let mut was_paused = false;
    let mut next = Instant::now();
    let mut canvas_size: Option<(u32, u32)> = None;
    let mut failures = 0;

    while !stop.load(Ordering::SeqCst) {
        if clock.is_paused() {
            was_paused = true;
            thread::sleep(Duration::from_millis(15));
            continue;
        }
        if was_paused {
            was_paused = false;
            next = Instant::now();
        }
        let at = clock.now().unwrap_or_default();

        #[cfg(windows)]
        let grabbed = match fast.as_mut().map(|f| f.frame(0)) {
            Some(Ok(img)) => Ok(img),
            Some(Err(_)) => {
                // Fall back to the standard capture for the rest of the video.
                fast = None;
                if let Ok(mut s) = stats.lock() {
                    s.fast_capture = false;
                }
                source.capture()
            }
            None => source.capture(),
        };
        #[cfg(not(windows))]
        let grabbed = source.capture();

        let mut image = match grabbed {
            Ok(img) => {
                failures = 0;
                img
            }
            Err(e) => {
                failures += 1;
                if failures > 30 || canvas_size.is_none() && failures > 3 {
                    return Err(e);
                }
                thread::sleep(interval);
                continue;
            }
        };

        if settings.pointer || settings.clicks {
            if let Some(p) = platform::pointer() {
                let (ox, oy) = if fixed_origin { origin } else { source.origin() };
                let mut p = p;
                p.visible = p.visible && settings.pointer;
                capture::draw_pointer(&mut image, p.x - ox, p.y - oy, scale, settings.clicks, &p);
            }
        }

        // Work out the video size from the first frame.
        let (w, h) = match canvas_size {
            Some(size) => size,
            None => {
                let (mut w, mut h) = image.dimensions();
                if settings.half_size {
                    w = (w / 2).max(2);
                    h = (h / 2).max(2);
                }
                if settings.format == VideoFormat::Gif {
                    (w, h) = capture::fit_within(w, h, GIF_MAX_SIDE, GIF_MAX_SIDE);
                }
                // H.264 needs even sizes.
                w = (w & !1).max(2);
                h = (h & !1).max(2);
                canvas_size = Some((w, h));
                if let Ok(mut s) = stats.lock() {
                    s.width = w;
                    s.height = h;
                }
                (w, h)
            }
        };
        let image = fit_frame(image, w, h, settings.half_size || settings.format == VideoFormat::Gif);

        if let Ok(mut s) = stats.lock() {
            s.captured += 1;
            s.elapsed = at;
        }
        match tx.try_send(Frame { image, at }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                if let Ok(mut s) = stats.lock() {
                    s.dropped += 1;
                }
            }
            Err(TrySendError::Disconnected(_)) => return Err("The video encoder stopped.".into()),
        }

        next += interval;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else if now - next > interval * 2 {
            // Too slow to keep up: don't try to catch up with a burst.
            next = now;
        }
    }
    Ok(())
}

/// Make every frame exactly w × h: scale it if it is a different size
/// because of the size option, otherwise crop or pad (a window resized
/// while recording).
fn fit_frame(image: RgbaImage, w: u32, h: u32, scaled: bool) -> RgbaImage {
    if image.dimensions() == (w, h) {
        return image;
    }
    let (iw, ih) = image.dimensions();
    if scaled {
        let (fw, fh) = capture::fit_within(iw, ih, w, h);
        let small = if (fw, fh) == (iw, ih) && iw <= w && ih <= h {
            image
        } else {
            capture::downscale(&image, fw.min(w), fh.min(h))
        };
        if small.dimensions() == (w, h) {
            return small;
        }
        let mut canvas = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
        image::imageops::replace(&mut canvas, &small, 0, 0);
        return canvas;
    }
    let mut canvas = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
    image::imageops::replace(&mut canvas, &image, 0, 0);
    canvas
}

fn encode_loop(
    settings: &RecordSettings,
    rx: Receiver<Frame>,
    audio: Option<AudioFeed>,
    stats: &Mutex<RecStats>,
) -> Result<RecordResult, String> {
    if let Some(parent) = settings.path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Couldn't create the video folder: {e}"))?;
    }

    // Find out whether sound started; the sound thread answers within moments.
    let mut audio_note: Option<String> = None;
    let mut audio = match audio {
        Some(feed) => match feed.ready.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(note)) => {
                audio_note = note;
                Some(feed)
            }
            Ok(Err(e)) => {
                audio_note = Some(e);
                None
            }
            Err(_) => {
                audio_note = Some("no sound (the sound device didn't answer)".into());
                None
            }
        },
        None => None,
    };
    if let Ok(mut s) = stats.lock() {
        s.audio = audio.is_some();
    }

    let mut sink: Box<dyn Sink> = match settings.format {
        VideoFormat::Mp4 => Box::new(Mp4Auto::new(settings, audio.is_some())),
        VideoFormat::Gif => Box::new(GifSink::new(settings)?),
    };
    let mut frames = 0u64;
    let mut size = (0, 0);
    let mut first_at: Option<Duration> = None;
    let mut last_at = Duration::ZERO;
    let mut thumb_done = settings.thumb_path.is_none();
    let mut held_audio: Vec<crate::audio::AudioChunk> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(mut frame) => {
                if !thumb_done {
                    thumb_done = true;
                    if let Some(thumb) = &settings.thumb_path {
                        save_thumbnail(&frame.image, thumb);
                    }
                }
                // Videos start at zero.
                let first = *first_at.get_or_insert(frame.at);
                frame.at = frame.at.saturating_sub(first);
                size = frame.image.dimensions();
                last_at = frame.at;
                sink.push(frame)?;
                frames += 1;
                if let Ok(mut s) = stats.lock() {
                    s.encoded = frames;
                    s.bytes = sink.bytes_written();
                    s.encoder = sink.label();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if let Some(feed) = &audio {
            held_audio.extend(feed.chunks.try_iter());
            if let Some(first) = first_at {
                for chunk in held_audio.drain(..) {
                    push_audio_chunk(&mut *sink, chunk, first)?;
                }
            }
        }
    }
    // The picture is done: stop the sound and add what is left of it.
    if let Some(feed) = audio.take() {
        let AudioFeed { chunks, ready: _, mut capture } = feed;
        capture.stop();
        drop(capture);
        held_audio.extend(chunks.try_iter());
        if let Some(first) = first_at {
            for chunk in held_audio.drain(..) {
                push_audio_chunk(&mut *sink, chunk, first)?;
            }
        }
    }

    let label = sink.label();
    if settings.format == VideoFormat::Mp4 && (settings.system_audio || settings.mic) && label == "openh264" {
        audio_note = Some("no sound (Windows' video encoder wasn't available)".into());
    }
    let bytes = sink.finish()?;
    if let Ok(mut s) = stats.lock() {
        s.encoder = label;
    }
    let frame_time = Duration::from_secs_f64(1.0 / settings.fps.max(1) as f64);
    let size = if label == "openh264" { capture::fit_within(size.0, size.1, OPENH264_MAX.0, OPENH264_MAX.1) } else { size };
    Ok(RecordResult {
        path: settings.path.clone(),
        duration: if frames > 0 { last_at + frame_time } else { Duration::ZERO },
        frames,
        bytes,
        width: size.0 & !1,
        height: size.1 & !1,
        warning: audio_note,
    })
}

/// Hand sound to the encoder on the video's timeline (the video starts at
/// its first frame; sound from before that is cut).
fn push_audio_chunk(sink: &mut dyn Sink, chunk: crate::audio::AudioChunk, first: Duration) -> Result<(), String> {
    let channels = crate::audio::CHANNELS as usize;
    let frames = chunk.samples.len() / channels;
    let end = chunk.at + Duration::from_secs_f64(frames as f64 / crate::audio::RATE as f64);
    if end <= first {
        return Ok(());
    }
    if chunk.at >= first {
        return sink.push_audio(chunk.at - first, &chunk.samples);
    }
    let skip = (((first - chunk.at).as_secs_f64() * crate::audio::RATE as f64).round() as usize).min(frames);
    sink.push_audio(Duration::ZERO, &chunk.samples[skip * channels..])
}

pub fn save_thumbnail(image: &RgbaImage, path: &std::path::Path) {
    let (w, h) = capture::fit_within(image.width(), image.height(), 480, 300);
    let small = capture::downscale(image, w, h);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = small.save_with_format(path, image::ImageFormat::Png);
}

trait Sink {
    fn push(&mut self, frame: Frame) -> Result<(), String>;
    /// Sound (interleaved stereo) starting at `at` on the video's timeline.
    fn push_audio(&mut self, _at: Duration, _samples: &[i16]) -> Result<(), String> {
        Ok(())
    }
    fn bytes_written(&self) -> u64;
    fn finish(self: Box<Self>) -> Result<u64, String>;
    /// Which encoder is doing the work.
    fn label(&self) -> &'static str;
}

fn bitrate_for(w: u32, h: u32, fps: u32, quality: Quality) -> u32 {
    (w as f32 * h as f32 * fps as f32 * quality.bits_per_pixel()).clamp(500_000.0, 60_000_000.0) as u32
}

// ---------------------------------------------------------------------------
// MP4: Windows' own encoder when possible, the built-in one otherwise
// ---------------------------------------------------------------------------

struct Mp4Auto {
    settings: RecordSettings,
    inner: Option<Box<dyn Sink>>,
    pushed: u64,
    #[cfg_attr(not(windows), allow(dead_code))]
    audio: bool,
}

impl Mp4Auto {
    fn new(settings: &RecordSettings, audio: bool) -> Mp4Auto {
        Mp4Auto { settings: settings.clone(), inner: None, pushed: 0, audio }
    }

    fn fallback(&mut self) -> Result<(), String> {
        self.inner = Some(Box::new(Mp4Sink::new(&self.settings)?));
        Ok(())
    }
}

impl Sink for Mp4Auto {
    fn push(&mut self, frame: Frame) -> Result<(), String> {
        if self.inner.is_none() {
            #[cfg(windows)]
            {
                let (w, h) = frame.image.dimensions();
                match MfSink::create(&self.settings, w, h, self.audio) {
                    Ok(sink) => self.inner = Some(Box::new(sink)),
                    Err(e) => eprintln!("lapcapture: Windows video encoder unavailable ({e}); using the built-in one"),
                }
            }
            if self.inner.is_none() {
                self.fallback()?;
            }
        }
        #[cfg(windows)]
        if self.pushed == 0 && self.inner.as_ref().is_some_and(|s| s.label() == "media-foundation") {
            // If Windows' encoder refuses the very first frame, switch now.
            let copy = Frame { image: frame.image.clone(), at: frame.at };
            match self.inner.as_mut().unwrap().push(copy) {
                Ok(()) => {
                    self.pushed += 1;
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("lapcapture: Windows video encoder failed ({e}); using the built-in one");
                    if let Some(old) = self.inner.take() {
                        let _ = old.finish();
                    }
                    self.fallback()?;
                }
            }
        }
        self.pushed += 1;
        self.inner.as_mut().unwrap().push(frame)
    }

    fn push_audio(&mut self, at: Duration, samples: &[i16]) -> Result<(), String> {
        match self.inner.as_mut() {
            Some(sink) => sink.push_audio(at, samples),
            None => Ok(()),
        }
    }

    fn bytes_written(&self) -> u64 {
        self.inner.as_ref().map(|s| s.bytes_written()).unwrap_or(0)
    }

    fn finish(self: Box<Self>) -> Result<u64, String> {
        match self.inner {
            Some(sink) => sink.finish(),
            None => Ok(0),
        }
    }

    fn label(&self) -> &'static str {
        self.inner.as_ref().map(|s| s.label()).unwrap_or("")
    }
}

#[cfg(windows)]
struct MfSink {
    writer: Option<crate::mf::MfWriter>,
    path: PathBuf,
    interval: i64,
    frames: u64,
    bytes: u64,
}

#[cfg(windows)]
impl MfSink {
    fn create(settings: &RecordSettings, w: u32, h: u32, audio: bool) -> Result<MfSink, String> {
        let fps = settings.fps.max(1);
        let bitrate = bitrate_for(w, h, fps, settings.quality);
        let writer = crate::mf::MfWriter::create(&settings.path, w, h, fps, bitrate, audio)?;
        Ok(MfSink {
            writer: Some(writer),
            path: settings.path.clone(),
            interval: 10_000_000 / fps as i64,
            frames: 0,
            bytes: 0,
        })
    }
}

#[cfg(windows)]
impl Sink for MfSink {
    fn push(&mut self, frame: Frame) -> Result<(), String> {
        let start = (frame.at.as_nanos() / 100) as i64;
        self.writer
            .as_mut()
            .ok_or("video writer closed")?
            .write(frame.image.as_raw(), start, self.interval)?;
        self.frames += 1;
        if self.frames % 15 == 0 {
            if let Ok(meta) = std::fs::metadata(&self.path) {
                self.bytes = meta.len();
            }
        }
        Ok(())
    }

    fn push_audio(&mut self, at: Duration, samples: &[i16]) -> Result<(), String> {
        let start = (at.as_nanos() / 100) as i64;
        self.writer.as_mut().ok_or("video writer closed")?.write_audio(samples, start)
    }

    fn bytes_written(&self) -> u64 {
        self.bytes
    }

    fn finish(mut self: Box<Self>) -> Result<u64, String> {
        if let Some(mut writer) = self.writer.take() {
            writer.finish()?;
        }
        Ok(std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(self.bytes))
    }

    fn label(&self) -> &'static str {
        "media-foundation"
    }
}

const MP4_TIMESCALE: u32 = 90_000;
/// The built-in encoder is slow on very large frames, so it gets smaller ones.
const OPENH264_MAX: (u32, u32) = (1920, 1200);

struct Mp4Sink {
    file: Option<BufWriter<File>>,
    writer: Option<mp4::Mp4Writer<BufWriter<File>>>,
    encoder: Option<openh264::encoder::Encoder>,
    yuv: Option<openh264::formats::YUVBuffer>,
    fps: u32,
    quality: Quality,
    pending: Option<(Vec<u8>, bool, u64)>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    bytes: u64,
    size: (u32, u32),
}

impl Mp4Sink {
    fn new(settings: &RecordSettings) -> Result<Mp4Sink, String> {
        let file = File::create(&settings.path).map_err(|e| format!("Couldn't create the video file: {e}"))?;
        Ok(Mp4Sink {
            file: Some(BufWriter::new(file)),
            writer: None,
            encoder: None,
            yuv: None,
            fps: settings.fps.max(1),
            quality: settings.quality,
            pending: None,
            sps: None,
            pps: None,
            bytes: 0,
            size: (0, 0),
        })
    }

    fn make_encoder(&self, w: u32, h: u32) -> Result<openh264::encoder::Encoder, String> {
        use openh264::encoder::{
            BitRate, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, RateControlMode, UsageType,
        };
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 4) as u16;
        let config = EncoderConfig::new()
            .usage_type(UsageType::ScreenContentRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .bitrate(BitRate::from_bps(bitrate_for(w, h, self.fps, self.quality)))
            .max_frame_rate(FrameRate::from_hz(self.fps as f32))
            .skip_frames(false)
            .num_threads(threads)
            .intra_frame_period(IntraFramePeriod::from_num_frames(self.fps * 2));
        Encoder::with_api_config(openh264::OpenH264API::from_source(), config)
            .map_err(|e| format!("Couldn't start the video encoder: {e}"))
    }

    fn write_sample(&mut self, data: Vec<u8>, is_sync: bool, start: u64, duration: u32) -> Result<(), String> {
        if self.writer.is_none() {
            let (Some(sps), Some(pps)) = (self.sps.clone(), self.pps.clone()) else {
                return Ok(()); // can't start the file without stream headers
            };
            let file = self.file.take().ok_or("video file missing")?;
            let config = mp4::Mp4Config {
                major_brand: str::parse("isom").unwrap(),
                minor_version: 512,
                compatible_brands: vec![
                    str::parse("isom").unwrap(),
                    str::parse("iso2").unwrap(),
                    str::parse("avc1").unwrap(),
                    str::parse("mp41").unwrap(),
                ],
                timescale: 1000,
            };
            let mut writer = mp4::Mp4Writer::write_start(file, &config).map_err(|e| format!("MP4 error: {e}"))?;
            writer
                .add_track(&mp4::TrackConfig {
                    track_type: mp4::TrackType::Video,
                    timescale: MP4_TIMESCALE,
                    language: "und".into(),
                    media_conf: mp4::MediaConfig::AvcConfig(mp4::AvcConfig {
                        width: self.size.0 as u16,
                        height: self.size.1 as u16,
                        seq_param_set: sps,
                        pic_param_set: pps,
                    }),
                })
                .map_err(|e| format!("MP4 error: {e}"))?;
            self.writer = Some(writer);
        }
        let len = data.len() as u64;
        let sample = mp4::Mp4Sample {
            start_time: start,
            duration: duration.max(1),
            rendering_offset: 0,
            is_sync,
            bytes: bytes::Bytes::from(data),
        };
        self.writer
            .as_mut()
            .unwrap()
            .write_sample(1, &sample)
            .map_err(|e| format!("Couldn't write the video: {e}"))?;
        self.bytes += len;
        Ok(())
    }
}

/// Split an Annex B stream into NAL units (without start codes).
fn split_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut nals = Vec::new();
    let mut i = 0;
    let mut start: Option<usize> = None;
    while i + 3 <= data.len() {
        let code3 = data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1;
        if code3 {
            if let Some(s) = start {
                let mut end = i;
                if end > s && data[end - 1] == 0 {
                    end -= 1; // 4-byte start code
                }
                nals.push(&data[s..end]);
            }
            i += 3;
            start = Some(i);
        } else {
            i += 1;
        }
    }
    if let Some(s) = start {
        if s < data.len() {
            nals.push(&data[s..]);
        }
    }
    nals.into_iter().filter(|n| !n.is_empty()).collect()
}

impl Sink for Mp4Sink {
    fn push(&mut self, frame: Frame) -> Result<(), String> {
        use openh264::formats::{RgbaSliceU8, YUVBuffer};
        let (fw, fh) = frame.image.dimensions();
        if self.encoder.is_none() {
            let (w, h) = capture::fit_within(fw, fh, OPENH264_MAX.0, OPENH264_MAX.1);
            let (w, h) = ((w & !1).max(2), (h & !1).max(2));
            self.size = (w, h);
            self.encoder = Some(self.make_encoder(w, h)?);
            self.yuv = Some(YUVBuffer::new(w as usize, h as usize));
        }
        let (w, h) = self.size;
        let image = if (fw, fh) == (w, h) { frame.image } else { capture::downscale(&frame.image, w, h) };
        let image = if image.dimensions() == (w, h) {
            image
        } else {
            let mut canvas = RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, 255]));
            image::imageops::replace(&mut canvas, &image, 0, 0);
            canvas
        };
        let yuv = self.yuv.as_mut().unwrap();
        yuv.read_rgba8(RgbaSliceU8::new(image.as_raw(), (w as usize, h as usize)));
        let encoded = self
            .encoder
            .as_mut()
            .unwrap()
            .encode(&*yuv)
            .map_err(|e| format!("Video encoding failed: {e}"))?
            .to_vec();

        let mut sample = Vec::with_capacity(encoded.len() + 16);
        let mut is_sync = false;
        for nal in split_nals(&encoded) {
            match nal[0] & 0x1f {
                7 => self.sps = Some(nal.to_vec()),
                8 => self.pps = Some(nal.to_vec()),
                t => {
                    if t == 5 {
                        is_sync = true;
                    }
                    sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                    sample.extend_from_slice(nal);
                }
            }
        }
        if sample.is_empty() {
            return Ok(()); // the encoder skipped this frame
        }
        let ts = (frame.at.as_secs_f64() * MP4_TIMESCALE as f64) as u64;
        if let Some((data, sync, start)) = self.pending.take() {
            let dur = ts.saturating_sub(start).max(1) as u32;
            self.write_sample(data, sync, start, dur)?;
        }
        self.pending = Some((sample, is_sync, ts));
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes
    }

    fn finish(mut self: Box<Self>) -> Result<u64, String> {
        if let Some((data, sync, start)) = self.pending.take() {
            let dur = MP4_TIMESCALE / self.fps;
            self.write_sample(data, sync, start, dur)?;
        }
        if let Some(mut writer) = self.writer.take() {
            writer.write_end().map_err(|e| format!("Couldn't finish the video file: {e}"))?;
            let mut inner = writer.into_writer();
            use std::io::Write;
            inner.flush().map_err(|e| format!("Couldn't finish the video file: {e}"))?;
        }
        Ok(self.bytes)
    }

    fn label(&self) -> &'static str {
        "openh264"
    }
}

// ---------------------------------------------------------------------------
// GIF
// ---------------------------------------------------------------------------

struct GifSink {
    file: Option<BufWriter<File>>,
    encoder: Option<gif::Encoder<BufWriter<File>>>,
    speed: i32,
    pending: Option<(gif::Frame<'static>, Duration)>,
    /// The previous picture, so only what changed is stored.
    previous: Option<Vec<u8>>,
    bytes: u64,
}

impl GifSink {
    fn new(settings: &RecordSettings) -> Result<GifSink, String> {
        let file = File::create(&settings.path).map_err(|e| format!("Couldn't create the GIF file: {e}"))?;
        Ok(GifSink {
            file: Some(BufWriter::new(file)),
            encoder: None,
            speed: settings.quality.gif_speed(),
            pending: None,
            previous: None,
            bytes: 0,
        })
    }

    fn write(&mut self, mut frame: gif::Frame<'static>, delay: Duration) -> Result<(), String> {
        frame.delay = ((delay.as_millis() as f64 / 10.0).round() as u16).max(2);
        // Rough size so far (palette indexes compress well).
        self.bytes += frame.buffer.len() as u64 / 4;
        self.encoder
            .as_mut()
            .unwrap()
            .write_frame(&frame)
            .map_err(|e| format!("Couldn't write the GIF: {e}"))
    }
}

/// The smallest box (x, y, w, h) holding every pixel that differs.
fn changed_box(a: &[u8], b: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let stride = w * 4;
    let (mut minx, mut miny, mut maxx, mut maxy) = (w, h, 0usize, 0usize);
    let mut any = false;
    for y in 0..h {
        let ra = &a[y * stride..(y + 1) * stride];
        let rb = &b[y * stride..(y + 1) * stride];
        if ra == rb {
            continue;
        }
        let first = (0..w).find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4]).unwrap_or(0);
        let last = (0..w).rev().find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4]).unwrap_or(w - 1);
        minx = minx.min(first);
        maxx = maxx.max(last);
        miny = miny.min(y);
        maxy = y;
        any = true;
    }
    any.then(|| (minx, miny, maxx - minx + 1, maxy - miny + 1))
}

impl Sink for GifSink {
    fn push(&mut self, frame: Frame) -> Result<(), String> {
        let (w, h) = frame.image.dimensions();
        if self.encoder.is_none() {
            let file = self.file.take().ok_or("GIF file missing")?;
            let mut enc = gif::Encoder::new(file, w as u16, h as u16, &[])
                .map_err(|e| format!("Couldn't start the GIF: {e}"))?;
            enc.set_repeat(gif::Repeat::Infinite)
                .map_err(|e| format!("Couldn't start the GIF: {e}"))?;
            self.encoder = Some(enc);
        }
        let rgba = frame.image.into_raw();
        let area = match &self.previous {
            None => Some((0, 0, w as usize, h as usize)),
            Some(prev) => changed_box(prev, &rgba, w as usize, h as usize),
        };
        // Nothing changed: the previous frame simply stays up longer.
        let Some((x, y, bw, bh)) = area else { return Ok(()) };

        let mut part = Vec::with_capacity(bw * bh * 4);
        for row in y..y + bh {
            let start = (row * w as usize + x) * 4;
            part.extend_from_slice(&rgba[start..start + bw * 4]);
            // Pixels that didn't change become see-through, which compresses
            // far better than repeating them.
            if let Some(prev) = &self.previous {
                let n = part.len();
                let line = &mut part[n - bw * 4..];
                for (i, px) in line.chunks_exact_mut(4).enumerate() {
                    let p = start + i * 4;
                    if px == &prev[p..p + 4] {
                        px.copy_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
        }
        let mut gif_frame = gif::Frame::from_rgba_speed(bw as u16, bh as u16, &mut part, self.speed);
        gif_frame.left = x as u16;
        gif_frame.top = y as u16;
        gif_frame.dispose = gif::DisposalMethod::Keep;
        if let Some((prev, prev_at)) = self.pending.take() {
            self.write(prev, frame.at.saturating_sub(prev_at))?;
        }
        self.pending = Some((gif_frame, frame.at));
        self.previous = Some(rgba);
        Ok(())
    }

    fn bytes_written(&self) -> u64 {
        self.bytes
    }

    fn finish(mut self: Box<Self>) -> Result<u64, String> {
        if let Some((prev, _)) = self.pending.take() {
            self.write(prev, Duration::from_millis(100))?;
        }
        if let Some(enc) = self.encoder.take() {
            let mut inner = enc.into_inner().map_err(|e| format!("Couldn't finish the GIF: {e}"))?;
            use std::io::Write;
            inner.flush().map_err(|e| format!("Couldn't finish the GIF: {e}"))?;
            if let Ok(meta) = inner.get_ref().metadata() {
                self.bytes = meta.len();
            }
        }
        Ok(self.bytes)
    }

    fn label(&self) -> &'static str {
        "gif"
    }
}

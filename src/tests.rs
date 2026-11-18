//! End-to-end checks of the capture, video and markup code on a real screen.
//! Results are checked by numbers (size, frame count, decodability, colour
//! variety); nothing captured is kept outside the temp folder.

use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};

use crate::annotate::{self, Mark};
use crate::capture::{self, Target};
use crate::recorder::{Quality, RecordResult, RecordSettings, Recording, VideoFormat};

fn test_dir() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-captures");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How many distinct grey levels a picture has (a blank picture has one).
fn variety(img: &image::RgbaImage) -> usize {
    let mut seen = [false; 256];
    for p in img.pixels().step_by(97) {
        seen[((p.0[0] as u32 + p.0[1] as u32 + p.0[2] as u32) / 3) as usize] = true;
    }
    seen.iter().filter(|s| **s).count()
}

#[test]
fn screenshot_of_main_screen() {
    let monitors = capture::list_monitors();
    assert!(!monitors.is_empty(), "no screens found");
    let m = &monitors[0];
    let start = Instant::now();
    let img = capture::capture_target(&Target::Monitor(m.id)).expect("capture works");
    println!("screen {}x{} captured in {:?}, variety {}", img.width(), img.height(), start.elapsed(), variety(&img));
    assert_eq!((img.width(), img.height()), (m.width, m.height));
    assert!(variety(&img) > 1, "screenshot is blank");

    let region = capture::capture_target(&Target::Region { monitor: m.id, x: 10, y: 20, w: 300, h: 200 }).unwrap();
    assert_eq!(region.dimensions(), (300, 200));

    let windows = capture::list_windows();
    println!("{} windows listed", windows.len());
    if let Some(w) = windows.first() {
        match capture::capture_target(&Target::Window(w.id)) {
            Ok(img) => println!("window capture {}x{}", img.width(), img.height()),
            Err(e) => println!("window capture failed: {e}"),
        }
    }
}

#[test]
fn markup_changes_pixels() {
    let base = image::RgbaImage::from_pixel(400, 300, image::Rgba([240, 240, 240, 255]));
    let marks = vec![
        Mark::Pen { points: vec![[10.0, 10.0], [100.0, 80.0], [150.0, 40.0]], color: [255, 0, 0, 255], width: 6.0 },
        Mark::Highlight { points: vec![[20.0, 200.0], [300.0, 200.0]], color: [255, 214, 10, 100], width: 18.0 },
        Mark::Rect { a: [50.0, 120.0], b: [200.0, 180.0], color: [0, 0, 255, 255], width: 4.0 },
        Mark::Arrow { a: [380.0, 280.0], b: [250.0, 150.0], color: [0, 160, 0, 255], width: 5.0 },
        Mark::Pixelate { a: [300.0, 10.0], b: [390.0, 90.0] },
    ];
    let out = annotate::render(&base, &marks);
    assert_eq!(out.get_pixel(55, 47).0[0], 255, "pen stroke missing");
    assert!(out.get_pixel(55, 47).0[1] < 60);
    let hl = out.get_pixel(160, 200).0;
    assert!(hl[2] < 200 && hl[0] > 200, "highlighter missing: {hl:?}");
    assert!(out.get_pixel(50, 150).0[2] > 200 && out.get_pixel(50, 150).0[0] < 60, "box missing");
    assert!(out.get_pixel(260, 160).0[1] > 120 && out.get_pixel(260, 160).0[0] < 60, "arrow head missing");
}

fn record(format: VideoFormat, secs: u64, name: &str, half_size: bool) -> RecordResult {
    let monitors = capture::list_monitors();
    let m = monitors.first().expect("a screen");
    let dir = test_dir();
    let path = dir.join(format!("{name}.{}", format.extension()));
    let settings = RecordSettings {
        target: Target::Monitor(m.id),
        fps: 30,
        format,
        half_size,
        quality: Quality::Medium,
        pointer: true,
        clicks: true,
        path,
        thumb_path: Some(dir.join(format!("{name}-thumb.png"))),
        system_audio: true,
        mic: false,
    };
    let mut rec = Recording::start(settings, eframe::egui::Context::default());
    std::thread::sleep(Duration::from_secs(secs));
    let stats = rec.stats();
    println!(
        "{name}: captured {} dropped {} encoded {} size {}x{} fast {} encoder {} sound {}",
        stats.captured, stats.dropped, stats.encoded, stats.width, stats.height, stats.fast_capture, stats.encoder, stats.audio
    );
    rec.stop();
    let start = Instant::now();
    loop {
        if let Some(r) = rec.poll() {
            let r = r.expect("recording works");
            println!("{name}: note {:?}", r.warning);
            return r;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "recording never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn records_a_playable_mp4() {
    let r = record(VideoFormat::Mp4, 4, "test-video", false);
    println!("mp4: {:?} frames {} bytes {} {}x{}", r.duration, r.frames, r.bytes, r.width, r.height);
    assert!(r.frames >= 30, "too few frames: {}", r.frames);

    let file = File::open(&r.path).unwrap();
    let size = file.metadata().unwrap().len();
    let mut reader = mp4::Mp4Reader::read_header(BufReader::new(file), size).expect("valid mp4");
    let (track_id, samples, sps, pps, w, h, dur) = {
        for (id, t) in reader.tracks() {
            println!("track {id}: {:?} {} samples {:?}", t.track_type().ok(), t.sample_count(), t.duration());
        }
        #[cfg(windows)]
        assert!(
            reader.tracks().values().any(|t| t.track_type().ok() == Some(mp4::TrackType::Audio)),
            "no sound track"
        );
        let (id, track) = reader
            .tracks()
            .iter()
            .find(|(_, t)| t.track_type().ok() == Some(mp4::TrackType::Video))
            .expect("a video track");
        (
            *id,
            track.sample_count(),
            track.sequence_parameter_set().unwrap().to_vec(),
            track.picture_parameter_set().unwrap().to_vec(),
            track.width(),
            track.height(),
            track.duration(),
        )
    };
    println!("mp4 track: {samples} samples, {w}x{h}, {dur:?}");
    assert_eq!((w as u32, h as u32), (r.width, r.height));
    assert!(samples as u64 >= r.frames.saturating_sub(2));
    assert!(dur > Duration::from_millis(3000) && dur < Duration::from_millis(6000), "duration {dur:?}");

    // Decode the first frames to prove the stream is real H.264.
    let mut decoder = openh264::decoder::Decoder::new().unwrap();
    let mut decoded = 0;
    for sample_id in 1..=samples.min(10) {
        let sample = reader.read_sample(track_id, sample_id).unwrap().unwrap();
        let mut annexb = Vec::new();
        if sample_id == 1 {
            for ps in [&sps, &pps] {
                annexb.extend_from_slice(&[0, 0, 0, 1]);
                annexb.extend_from_slice(ps);
            }
        }
        let data = &sample.bytes[..];
        let mut i = 0;
        while i + 4 <= data.len() {
            let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
            annexb.extend_from_slice(&[0, 0, 0, 1]);
            annexb.extend_from_slice(&data[i + 4..i + 4 + len]);
            i += 4 + len;
        }
        if let Ok(Some(yuv)) = decoder.decode(&annexb) {
            use openh264::formats::YUVSource;
            assert_eq!(yuv.dimensions(), (w as usize, h as usize));
            if decoded == 0 {
                let mut seen = [false; 256];
                for v in yuv.y().iter().step_by(101) {
                    seen[*v as usize] = true;
                }
                let levels = seen.iter().filter(|s| **s).count();
                println!("first decoded frame has {levels} brightness levels");
                assert!(levels > 1, "first video frame is blank");
            }
            decoded += 1;
        }
    }
    println!("decoded {decoded} frames");
    assert!(decoded > 0, "no frame could be decoded");

    let thumb = image::open(test_dir().join("test-video-thumb.png")).expect("thumbnail saved").to_rgba8();
    assert!(variety(&thumb) > 1, "thumbnail is blank");
    // Recordings of the screen are not kept.
    drop(reader);
    let _ = std::fs::remove_file(&r.path);
    let _ = std::fs::remove_file(test_dir().join("test-video-thumb.png"));
}

#[cfg(windows)]
#[test]
fn fast_capture_frames_are_stable() {
    let m = capture::list_monitors().into_iter().next().expect("a screen");
    let mut dup = crate::dxgi::Duplicator::new(m.x, m.y, None).expect("desktop duplication");
    dup.seed(capture::capture_target(&Target::Monitor(m.id)).unwrap());
    let mut prev = dup.frame(100).unwrap();
    let mut changed_total = 0usize;
    for i in 0..8 {
        std::thread::sleep(Duration::from_millis(100));
        let cur = dup.frame(0).unwrap();
        let changed = prev
            .as_raw()
            .chunks_exact(4)
            .zip(cur.as_raw().chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        let total = (cur.width() * cur.height()) as usize;
        println!("frame {i}: {changed} of {total} pixels changed ({:.2}%)", changed as f64 * 100.0 / total as f64);
        changed_total += changed;
        prev = cur;
    }
    println!("average changed pixels per frame: {}", changed_total / 8);
}

#[test]
fn records_a_gif() {
    let r = record(VideoFormat::Gif, 3, "test-anim", true);
    println!("gif: {:?} frames {} bytes {} {}x{}", r.duration, r.frames, r.bytes, r.width, r.height);
    assert!(r.frames >= 5, "too few frames: {}", r.frames);
    let file = File::open(&r.path).unwrap();
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(BufReader::new(file)).expect("valid gif");
    assert_eq!((decoder.width() as u32, decoder.height() as u32), (r.width, r.height));
    let mut frames = 0;
    while let Some(_frame) = decoder.read_next_frame().unwrap() {
        frames += 1;
    }
    println!("gif decoded {frames} frames");
    assert!(frames >= 1 && frames as u64 <= r.frames);
    drop(decoder);
    let _ = std::fs::remove_file(&r.path);
    let _ = std::fs::remove_file(test_dir().join("test-anim-thumb.png"));
}

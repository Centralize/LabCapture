//! LapCapture: take screenshots and record your screen.
//!
//! Run with `--screenshot <file.png>` to open the window without taking
//! focus, draw about 120 frames, save the window's own picture to that file
//! and exit.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod annotate;
mod app;
mod audio;
mod capture;
#[cfg(windows)]
mod dxgi;
mod icons;
mod library;
#[cfg(windows)]
mod mf;
mod platform;
mod recorder;
mod settings;
mod theme;
mod views;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use eframe::egui;

fn parse_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--screenshot" {
            return args.get(i + 1).map(PathBuf::from);
        }
        if let Some(v) = args[i].strip_prefix("--screenshot=") {
            return Some(PathBuf::from(v));
        }
        i += 1;
    }
    None
}

/// The window icon: a rounded gradient square with a camera lens.
fn app_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let r = 16.0f32;
    let n = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance outside the rounded square.
            let qx = (fx - n / 2.0).abs() - (n / 2.0 - r);
            let qy = (fy - n / 2.0).abs() - (n / 2.0 - r);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
            let alpha = (0.5 - outside).clamp(0.0, 1.0);
            let t = (fx + fy) / (2.0 * n);
            let (r0, g0, b0) = (99.0, 132.0, 255.0);
            let (r1, g1, b1) = (214.0, 92.0, 220.0);
            let mut c = [r0 + (r1 - r0) * t, g0 + (g1 - g0) * t, b0 + (b1 - b0) * t];
            let d = ((fx - n / 2.0).powi(2) + (fy - n / 2.0).powi(2)).sqrt();
            let ring = (1.0 - ((d - 16.0).abs() - 2.5).clamp(0.0, 1.0)).max(0.0);
            let dot = (6.5 - d).clamp(0.0, 1.0);
            let white = ring.max(dot);
            for ch in &mut c {
                *ch = *ch + (255.0 - *ch) * white;
            }
            let i = ((y * size + x) * 4) as usize;
            rgba[i] = c[0] as u8;
            rgba[i + 1] = c[1] as u8;
            rgba[i + 2] = c[2] as u8;
            rgba[i + 3] = (alpha * 255.0) as u8;
        }
    }
    egui::IconData { rgba, width: size, height: size }
}

fn main() -> eframe::Result<()> {
    let self_shot = parse_args();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LabCapture")
        .with_app_id("labcapture")
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([920.0, 620.0])
        .with_icon(std::sync::Arc::new(app_icon()));

    if self_shot.is_some() {
        // A check run: never steal focus from what the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if self_shot.is_some() {
        options.persist_window = false;
        options.persistence_path = Some(std::env::temp_dir().join("lapcapture-check").join("state.ron"));
        #[cfg(target_os = "macos")]
        {
            options.event_loop_builder = Some(Box::new(|builder| {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                builder.with_activate_ignoring_other_apps(false);
            }));
        }
    }

    eframe::run_native(
        "LabCapture",
        options,
        Box::new(move |cc| Ok(Box::new(app::LapApp::new(cc, self_shot)))),
    )
}

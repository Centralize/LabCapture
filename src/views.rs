//! Everything you see: the top bar, the Screenshot, Record, Library and
//! Settings pages, the area picker, countdowns and notifications.

use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Frame, Key, Layout, Margin, Pos2, Rect, RichText, Sense,
    Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2, ViewportCommand, pos2, vec2,
};

use crate::annotate::{self, Mark, Tool};
use crate::app::{LapApp, LibFilter, PALETTE, Pending, PickOutcome, Picker, ShotKind, Tab, ToastKind};
use crate::capture::{self, Target};
use crate::icons::{self, Icon};
use crate::library::{self, Item, Kind};
use crate::platform;
use crate::recorder::{GIF_MAX_SIDE, Quality, VideoFormat};
use crate::settings::{ImageFormat, Mode, ScreenPick};
use crate::theme::{self, ACCENT, ACCENT_HI, AMBER, BG, BORDER, CARD, CARD_HI, FAINT, GREEN, MUTED, PANEL, RED, RED_HI, TEXT};

impl eframe::App for LapApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        !self.is_self_shot()
    }

    fn on_exit(&mut self) {
        self.finish_recording_before_exit();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }
}

fn side_frame() -> Frame {
    Frame::new().fill(PANEL).inner_margin(Margin { left: 18, right: 18, top: 18, bottom: 14 })
}

fn main_frame() -> Frame {
    Frame::new().fill(BG).inner_margin(Margin::same(20))
}

impl LapApp {
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.frames += 1;
        self.local_shortcuts(&ctx);

        egui::Panel::top("top-bar")
            .exact_size(62.0)
            .resizable(false)
            .frame(Frame::new().fill(PANEL).inner_margin(Margin::symmetric(18, 0)))
            .show(ui, |ui| self.top_bar(ui));

        match self.tab {
            Tab::Screenshot => {
                egui::Panel::left("shot-side")
                    .exact_size(318.0)
                    .resizable(false)
                    .frame(side_frame())
                    .show(ui, |ui| {
                        // The main button stays at the bottom; the options scroll above it.
                        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                            self.shot_actions(ui);
                            ui.add_space(10.0);
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.shot_sidebar(ui));
                            });
                        });
                    });
                egui::CentralPanel::no_frame().frame(main_frame()).show(ui, |ui| self.shot_main(ui));
            }
            Tab::Record => {
                egui::Panel::left("rec-side")
                    .exact_size(318.0)
                    .resizable(false)
                    .frame(side_frame())
                    .show(ui, |ui| {
                        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                            self.record_actions(ui);
                            ui.add_space(10.0);
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| self.record_sidebar(ui));
                            });
                        });
                    });
                egui::CentralPanel::no_frame().frame(main_frame()).show(ui, |ui| self.record_main(ui));
            }
            Tab::Library => {
                egui::CentralPanel::no_frame().frame(main_frame()).show(ui, |ui| self.library_view(ui));
            }
            Tab::Settings => {
                egui::CentralPanel::no_frame().frame(main_frame()).show(ui, |ui| self.settings_view(ui));
            }
        }

        self.picker_viewport(&ctx);
        self.countdown_overlay(&ctx);
        self.toasts_overlay(&ctx);
        self.delete_modal(&ctx);
        self.self_shot_step(&ctx);
    }

    fn local_shortcuts(&mut self, ctx: &egui::Context) {
        let (esc, undo, redo, save, copy) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                i.key_pressed(Key::Escape),
                cmd && !i.modifiers.shift && i.key_pressed(Key::Z),
                cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z))),
                cmd && i.key_pressed(Key::S),
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
            )
        });
        if esc {
            self.cancel_countdown();
            self.drawing = None;
        }
        if self.tab == Tab::Screenshot {
            if let Some(ed) = &mut self.editor {
                if undo {
                    ed.undo();
                }
                if redo {
                    ed.redo();
                }
            }
            if self.editor.is_some() {
                if save {
                    self.save_editor(None, true);
                }
                if copy {
                    self.copy_editor();
                    self.toast("Copied to the clipboard", ToastKind::Success);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Top bar
    // -----------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            theme::logo(ui.painter(), r);
            ui.add_space(2.0);
            ui.label(RichText::new("LabCapture").size(17.0).strong().color(TEXT));
            ui.add_space(28.0);
            for (tab, icon, label) in [
                (Tab::Screenshot, Icon::Camera, "Screenshot"),
                (Tab::Record, Icon::Video, "Record"),
                (Tab::Library, Icon::Library, "Library"),
            ] {
                if nav_pill(ui, self.tab == tab, icon, label).clicked() {
                    self.tab = tab;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(ui, Icon::Settings, self.tab == Tab::Settings, true, "Settings").clicked() {
                    self.tab = Tab::Settings;
                }
                ui.add_space(6.0);
                if let Some(rec) = &self.recording {
                    let stats = rec.stats();
                    let paused = rec.is_paused();
                    let ctx = ui.ctx().clone();
                    if rec_pill(ui, &library::human_duration(stats.elapsed), paused).clicked() {
                        self.toggle_recording(&ctx);
                    }
                }
            });
        });
    }

    // -----------------------------------------------------------------------
    // Shared pieces of the side panels
    // -----------------------------------------------------------------------

    fn mode_tiles(&mut self, ui: &mut Ui, for_video: bool) {
        let w = ((ui.available_width() - 16.0) / 3.0).floor();
        let mut changed = false;
        ui.horizontal(|ui| {
            let mode = if for_video { &mut self.s.video_mode } else { &mut self.s.shot_mode };
            for (m, icon, label) in [
                (Mode::Screen, Icon::Screen, "Screen"),
                (Mode::Window, Icon::Window, "Window"),
                (Mode::Area, Icon::Area, "Area"),
            ] {
                if theme::tile(ui, *mode == m, icon, label, vec2(w, 80.0)).clicked() && *mode != m {
                    *mode = m;
                    changed = true;
                }
            }
        });
        if changed && for_video {
            self.preview_at_reset();
        }
    }

    fn preview_at_reset(&mut self) {
        self.preview = None;
        self.preview_error = None;
        self.reset_preview_timer();
    }

    fn screen_combo(&mut self, ui: &mut Ui, id: &str, for_video: bool) {
        let monitors = self.monitors.clone();
        let main_label = monitors
            .iter()
            .find(|m| m.primary)
            .or(monitors.first())
            .map(|m| format!("Main screen  ·  {} × {}", m.width, m.height))
            .unwrap_or_else(|| "Main screen".into());
        let pick = if for_video { &mut self.s.video_screen } else { &mut self.s.shot_screen };
        let text = match *pick {
            ScreenPick::Main => main_label.clone(),
            ScreenPick::All => "All screens together".into(),
            ScreenPick::Id(id) => monitors
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.label.clone())
                .unwrap_or_else(|| main_label.clone()),
        };
        let before = *pick;
        egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
            .selected_text(text)
            .show_ui(ui, |ui| {
                ui.selectable_value(pick, ScreenPick::Main, main_label.as_str());
                if monitors.len() > 1 {
                    for m in &monitors {
                        ui.selectable_value(pick, ScreenPick::Id(m.id), m.label.as_str());
                    }
                    if !for_video {
                        ui.selectable_value(pick, ScreenPick::All, "All screens together");
                    }
                }
            });
        if for_video && *pick != before {
            self.preview_at_reset();
        }
    }

    fn window_combo(&mut self, ui: &mut Ui, id: &str, for_video: bool) {
        let windows = self.windows.clone();
        let sel = if for_video { &mut self.video_window } else { &mut self.shot_window };
        if let Some(cur) = *sel {
            if !windows.is_empty() && !windows.iter().any(|w| w.id == cur) {
                *sel = None;
            }
        }
        let text = sel
            .and_then(|id| windows.iter().find(|w| w.id == id))
            .map(|w| capture::shorten(&w.label(), 34))
            .unwrap_or_else(|| "Choose a window…".into());
        let before = *sel;
        let resp = egui::ComboBox::from_id_salt(id)
            .width(ui.available_width())
            .selected_text(text)
            .show_ui(ui, |ui| {
                if windows.is_empty() {
                    ui.label(RichText::new("Looking for open windows…").color(MUTED));
                }
                for w in &windows {
                    ui.selectable_value(sel, Some(w.id), capture::shorten(&w.label(), 52))
                        .on_hover_text(format!("{} · {} × {}", w.title, w.width, w.height));
                }
            });
        let after = *sel;
        if resp.response.clicked() {
            let ctx = ui.ctx().clone();
            self.refresh_windows(&ctx);
        }
        if for_video && after != before {
            self.preview_at_reset();
        }
    }

    // -----------------------------------------------------------------------
    // Screenshot page
    // -----------------------------------------------------------------------

    fn shot_sidebar(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("New screenshot").size(19.0).strong());
        ui.label(RichText::new("Choose what to capture.").color(MUTED));
        ui.add_space(4.0);
        self.mode_tiles(ui, false);
        ui.add_space(2.0);
        match self.s.shot_mode {
            Mode::Screen => self.screen_combo(ui, "shot-screen", false),
            Mode::Window => self.window_combo(ui, "shot-window", false),
            Mode::Area => {
                hint_box(ui, Icon::Area, "Press the button, then drag over your screen to choose the area.");
            }
        }

        theme::section(ui, "Delay");
        theme::segmented(ui, &mut self.s.delay, &[(0, "None"), (3, "3 s"), (5, "5 s"), (10, "10 s")]);

        theme::section(ui, "After capture");
        theme::toggle(ui, &mut self.s.copy_after, "Copy to clipboard", None);
        theme::toggle(ui, &mut self.s.save_after, "Save to your screenshots folder", None);
        theme::toggle(ui, &mut self.s.show_after, "Bring LabCapture to the front", None);
        ui.add_space(12.0);
        self.shortcuts_card(ui);
    }

    /// The pinned button at the bottom of the screenshot panel.
    fn shot_actions(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let width = ui.available_width();
        match &self.pending {
            Pending::Countdown { until, .. } => {
                let left = until.saturating_duration_since(std::time::Instant::now()).as_secs() + 1;
                if theme::big_button(ui, Some(Icon::Close), &format!("Cancel  ·  {left}"), CARD_HI, BORDER, vec2(width, 48.0))
                    .clicked()
                {
                    self.cancel_countdown();
                }
            }
            Pending::Idle if self.picker.is_none() => {
                if theme::big_button(ui, Some(Icon::Camera), "Take screenshot", ACCENT, ACCENT_HI, vec2(width, 48.0))
                    .clicked()
                {
                    self.take_from_ui(&ctx);
                }
            }
            _ => {
                theme::big_button(ui, Some(Icon::Timer), "Capturing…", CARD_HI, CARD_HI, vec2(width, 48.0));
            }
        }
    }

    fn take_from_ui(&mut self, ctx: &egui::Context) {
        match self.s.shot_mode {
            Mode::Screen => self.request_shot(ctx, ShotKind::Screen),
            Mode::Window => match self.shot_window {
                Some(id) => self.request_shot(ctx, ShotKind::Window(id)),
                None => self.toast("Choose a window first", ToastKind::Error),
            },
            Mode::Area => self.request_shot(ctx, ShotKind::Area),
        }
    }

    fn shortcuts_card(&mut self, ui: &mut Ui) {
        theme::card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 5.0;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Keyboard, MUTED);
                ui.label(RichText::new("Shortcuts that work anywhere").size(13.0).strong());
            });
            if self.hotkeys.is_none() && !self.is_self_shot() {
                ui.label(
                    RichText::new(if self.s.hotkeys { "Not available right now." } else { "Turned off in Settings." })
                        .size(12.0)
                        .color(FAINT),
                );
            }
            for (label, key) in [
                ("Whole screen", "S"),
                ("Drag an area", "A"),
                ("Window in front", "W"),
                ("Start / stop recording", "R"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).size(12.5).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Right to left: the last key comes first.
                        theme::keys(ui, &[key, "Shift", "Alt"]);
                    });
                });
            }
            if let Some(err) = &self.hotkey_error {
                ui.label(RichText::new(err).size(12.0).color(AMBER));
            }
        });
    }

    fn shot_main(&mut self, ui: &mut Ui) {
        if self.editor.is_some() {
            self.editor_view(ui);
        } else {
            self.empty_state(ui);
        }
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect(rect, 18, Color32::from_rgb(17, 20, 28), Stroke::new(1.0, BORDER), StrokeKind::Inside);
        // Soft glow behind the icon.
        let recent: Vec<Item> = self.lib_items.iter().filter(|i| i.kind == Kind::Image).take(5).cloned().collect();
        let has_recent = !recent.is_empty();
        let content_h = 290.0 + if has_recent { 150.0 } else { 0.0 };
        let top = (rect.top() + (rect.height() - content_h) / 2.0).max(rect.top() + 30.0);
        theme::glow(&painter, pos2(rect.center().x, top + 46.0), 230.0, theme::VIOLET);
        // Corner marks, like a viewfinder.
        let inset = rect.shrink(26.0);
        let l = 34.0;
        let st = Stroke::new(2.0, Color32::from_rgb(52, 60, 80));
        for (c, dx, dy) in [
            (inset.left_top(), 1.0, 1.0),
            (inset.right_top(), -1.0, 1.0),
            (inset.left_bottom(), 1.0, -1.0),
            (inset.right_bottom(), -1.0, -1.0),
        ] {
            painter.line(vec![c + vec2(dx * l, 0.0), c, c + vec2(0.0, dy * l)], st);
        }

        let inner = Rect::from_min_max(pos2(rect.left() + 40.0, top), pos2(rect.right() - 40.0, rect.bottom() - 20.0));
        ui.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            ui.vertical_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(92.0, 92.0), Sense::hover());
                let p = ui.painter();
                theme::gradient_rounded_rect(p, r, 28.0, ACCENT.gamma_multiply(0.35), theme::VIOLET.gamma_multiply(0.35));
                p.rect_stroke(r, 28, Stroke::new(1.0, ACCENT.gamma_multiply(0.6)), StrokeKind::Inside);
                icons::paint(p, r.shrink(24.0), Icon::Camera, Color32::WHITE);
                ui.add_space(14.0);
                ui.label(RichText::new("Capture anything on your screen").size(25.0).strong().color(TEXT));
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Grab a whole screen, one window, or an area you drag out.")
                        .size(14.5)
                        .color(MUTED),
                );
                ui.label(
                    RichText::new("It's copied to the clipboard and saved in your Pictures folder, ready to mark up.")
                        .size(14.5)
                        .color(MUTED),
                );
                ui.add_space(18.0);

                let labels = ["Whole screen", "Drag an area", "Record a video"];
                let widths: f32 = labels.iter().map(|t| soft_width(ui, t)).sum::<f32>() + 2.0 * 10.0;
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - widths) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if theme::soft_button(ui, Icon::Screen, labels[0]).clicked() {
                        self.request_shot(&ctx, ShotKind::Screen);
                    }
                    if theme::soft_button(ui, Icon::Area, labels[1]).clicked() {
                        self.request_shot(&ctx, ShotKind::Area);
                    }
                    if theme::soft_button(ui, Icon::Video, labels[2]).clicked() {
                        self.tab = Tab::Record;
                    }
                });

                if has_recent {
                    ui.add_space(34.0);
                    ui.label(RichText::new("RECENT SCREENSHOTS").size(11.0).strong().color(FAINT));
                    ui.add_space(6.0);
                    let tw = 150.0;
                    let total = recent.len() as f32 * tw + (recent.len() as f32 - 1.0) * 10.0;
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                        ui.spacing_mut().item_spacing.x = 10.0;
                        for item in &recent {
                            self.want_thumb(&ctx, item);
                            let (r, resp) = ui.allocate_exact_size(vec2(tw, 92.0), Sense::click());
                            let p = ui.painter();
                            p.rect_filled(r, 10, CARD);
                            if let Some(tex) = self.lib_thumbs.get(&item.path) {
                                paint_cover(p, r.shrink(3.0), tex);
                            } else {
                                icons::paint(p, Rect::from_center_size(r.center(), vec2(26.0, 26.0)), Icon::Image, FAINT);
                            }
                            let hovered = resp.hovered();
                            p.rect_stroke(
                                r,
                                10,
                                Stroke::new(if hovered { 1.5 } else { 1.0 }, if hovered { ACCENT } else { BORDER }),
                                StrokeKind::Inside,
                            );
                            let resp = resp.on_hover_text(&item.name).on_hover_cursor(CursorIcon::PointingHand);
                            if resp.clicked() {
                                self.open_in_editor(item.path.clone());
                            }
                        }
                    });
                }
            });
        });
    }

    fn editor_view(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let toolbar = Rect::from_min_size(full.min, vec2(full.width(), 52.0));
        let actions = Rect::from_min_max(pos2(full.left(), full.bottom() - 52.0), full.max);
        let canvas = Rect::from_min_max(pos2(full.left(), toolbar.bottom() + 12.0), pos2(full.right(), actions.top() - 12.0));

        ui.scope_builder(UiBuilder::new().max_rect(toolbar), |ui| self.editor_toolbar(ui));
        self.editor_canvas(ui, canvas);
        ui.scope_builder(UiBuilder::new().max_rect(actions), |ui| self.editor_actions(ui));
    }

    fn editor_toolbar(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        ui.painter().rect(rect, 12, PANEL, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let inner = rect.shrink2(vec2(10.0, 9.0));
        ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for tool in Tool::ALL {
                let icon = match tool {
                    Tool::Pen => Icon::Pen,
                    Tool::Highlighter => Icon::Highlighter,
                    Tool::Rectangle => Icon::Rect,
                    Tool::Arrow => Icon::Arrow,
                    Tool::Pixelate => Icon::Pixelate,
                };
                let tip = format!("{} — {}", tool.name(), tool.hint());
                if theme::icon_button(ui, icon, self.tool == tool, true, &tip).clicked() {
                    self.tool = tool;
                    if tool == Tool::Highlighter && self.color_idx != 2 && self.color_idx != 3 {
                        self.color_idx = 2;
                    }
                }
            }
            divider(ui);
            for (i, c) in PALETTE.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(24.0, 34.0), Sense::click());
                let col = Color32::from_rgb(c[0], c[1], c[2]);
                let center = r.center();
                if self.color_idx == i {
                    ui.painter().circle_stroke(center, 10.5, Stroke::new(2.0, TEXT));
                }
                ui.painter().circle_filled(center, 7.5, col);
                if i == 7 {
                    ui.painter().circle_stroke(center, 7.5, Stroke::new(1.0, Color32::from_gray(90)));
                }
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    self.color_idx = i;
                }
            }
            divider(ui);
            ui.label(RichText::new("Size").size(12.5).color(MUTED));
            ui.spacing_mut().slider_width = 90.0;
            ui.add(egui::Slider::new(&mut self.thickness, 2.0..=24.0).show_value(false));
            let (r, _) = ui.allocate_exact_size(vec2(28.0, 34.0), Sense::hover());
            let c = self.current_color();
            ui.painter()
                .circle_filled(r.center(), (self.thickness / 2.0).clamp(1.5, 12.0), Color32::from_rgb(c[0], c[1], c[2]));
            divider(ui);
            let (can_undo, can_redo) = self
                .editor
                .as_ref()
                .map(|e| (!e.marks.is_empty(), !e.redo.is_empty()))
                .unwrap_or((false, false));
            if theme::icon_button(ui, Icon::Undo, false, can_undo, "Undo (Ctrl+Z)").clicked() {
                if let Some(e) = &mut self.editor {
                    e.undo();
                }
            }
            if theme::icon_button(ui, Icon::Redo, false, can_redo, "Redo (Ctrl+Y)").clicked() {
                if let Some(e) = &mut self.editor {
                    e.redo();
                }
            }
            if theme::icon_button(ui, Icon::Trash, false, can_undo, "Remove all markup").clicked() {
                if let Some(e) = &mut self.editor {
                    e.clear();
                }
            }
        });
    }

    fn editor_canvas(&mut self, ui: &mut Ui, area: Rect) {
        let Some(ed) = &self.editor else { return };
        let (iw, ih) = ed.size();
        let tex_id = ed.texture.id();
        let ppp = ui.ctx().pixels_per_point();

        let painter = ui.painter_at(area);
        painter.rect_filled(area, 14, Color32::from_rgb(10, 12, 17));
        // Dotted backdrop.
        let step = 22.0;
        let mut y = area.top() + step / 2.0;
        while y < area.bottom() {
            let mut x = area.left() + step / 2.0;
            while x < area.right() {
                painter.circle_filled(pos2(x, y), 0.9, Color32::from_rgb(32, 37, 50));
                x += step;
            }
            y += step;
        }

        let avail = area.shrink(18.0);
        let fit = (avail.width() / iw as f32).min(avail.height() / ih as f32);
        let scale = fit.min(1.0 / ppp).max(0.01);
        let img_rect = Rect::from_center_size(avail.center(), vec2(iw as f32 * scale, ih as f32 * scale));
        painter.rect_filled(img_rect.translate(vec2(0.0, 8.0)).expand(4.0), 8, Color32::from_black_alpha(70));
        painter.image(tex_id, img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        painter.rect_stroke(img_rect, 0, Stroke::new(1.0, Color32::from_rgb(56, 63, 82)), StrokeKind::Outside);

        let resp = ui
            .interact(img_rect, ui.scope_id().with("markup-canvas"), Sense::drag())
            .on_hover_cursor(CursorIcon::Crosshair);
        let to_img = |p: Pos2| -> [f32; 2] {
            [
                ((p.x - img_rect.left()) / scale).clamp(0.0, iw as f32),
                ((p.y - img_rect.top()) / scale).clamp(0.0, ih as f32),
            ]
        };
        let to_screen = |p: [f32; 2]| -> Pos2 { pos2(img_rect.left() + p[0] * scale, img_rect.top() + p[1] * scale) };
        let width_img = self.thickness / scale;
        let color = self.current_color();

        if resp.drag_started() {
            let start = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
            if let Some(start) = start {
                let a = to_img(start);
                self.drawing = Some(match self.tool {
                    Tool::Pen => Mark::Pen { points: vec![a], color, width: width_img },
                    Tool::Highlighter => Mark::Highlight {
                        points: vec![a],
                        color: [color[0], color[1], color[2], 100],
                        width: width_img * 3.0,
                    },
                    Tool::Rectangle => Mark::Rect { a, b: a, color, width: width_img },
                    Tool::Arrow => Mark::Arrow { a, b: a, color, width: width_img },
                    Tool::Pixelate => Mark::Pixelate { a, b: a },
                });
            }
        }
        if resp.dragged() {
            if let (Some(pos), Some(mark)) = (resp.interact_pointer_pos(), self.drawing.as_mut()) {
                let p = to_img(pos);
                match mark {
                    Mark::Pen { points, .. } | Mark::Highlight { points, .. } => {
                        let last = *points.last().unwrap();
                        if (last[0] - p[0]).abs() + (last[1] - p[1]).abs() > 1.5 / scale {
                            points.push(p);
                        }
                    }
                    Mark::Rect { b, .. } | Mark::Arrow { b, .. } | Mark::Pixelate { b, .. } => *b = p,
                }
            }
        }

        // The mark being drawn, on top.
        if let Some(mark) = &self.drawing {
            let pr = ui.painter_at(img_rect);
            let c32 = |c: [u8; 4]| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
            match mark {
                Mark::Pen { points, color, width } | Mark::Highlight { points, color, width } => {
                    let pts: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
                    if pts.len() == 1 {
                        pr.circle_filled(pts[0], width * scale / 2.0, c32(*color));
                    } else {
                        pr.add(Shape::line(pts, Stroke::new(width * scale, c32(*color))));
                    }
                }
                Mark::Rect { a, b, color, width } => {
                    pr.rect_stroke(
                        Rect::from_two_pos(to_screen(*a), to_screen(*b)),
                        0,
                        Stroke::new(width * scale, c32(*color)),
                        StrokeKind::Middle,
                    );
                }
                Mark::Arrow { a, b, color, width } => {
                    let (sa, sb) = (to_screen(*a), to_screen(*b));
                    let v = sb - sa;
                    let len = v.length().max(0.001);
                    let dir = v / len;
                    let (hl, hw) = annotate::arrow_head(*width);
                    let (hl, hw) = ((hl * scale).min(len), hw * scale);
                    let base = sb - dir * hl;
                    let normal = vec2(-dir.y, dir.x);
                    pr.line_segment([sa, sb - dir * hl * 0.7], Stroke::new(width * scale, c32(*color)));
                    pr.add(Shape::convex_polygon(
                        vec![sb, base + normal * hw, base - normal * hw],
                        c32(*color),
                        Stroke::NONE,
                    ));
                }
                Mark::Pixelate { a, b } => {
                    let r = Rect::from_two_pos(to_screen(*a), to_screen(*b));
                    pr.rect_filled(r, 0, Color32::from_white_alpha(40));
                    pr.add(Shape::dashed_line(
                        &[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()],
                        Stroke::new(1.5, Color32::WHITE),
                        6.0,
                        4.0,
                    ));
                }
            }
        }

        if resp.drag_stopped() {
            if let Some(mark) = self.drawing.take() {
                if mark.is_meaningful() {
                    if let Some(ed) = &mut self.editor {
                        ed.add(mark);
                    }
                }
            }
        }
    }

    fn editor_actions(&mut self, ui: &mut Ui) {
        let Some(ed) = &self.editor else { return };
        let (w, h) = ed.size();
        let name = ed
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Not saved yet".into());
        let detail = format!("{w} × {h}  ·  {}  ·  {}", ed.label, ed.taken.format("%H:%M"));
        let status = if ed.saving {
            ("Saving…", MUTED)
        } else if ed.is_saved() {
            ("Saved", GREEN)
        } else if ed.path.is_some() {
            ("Unsaved markup", AMBER)
        } else {
            ("Not saved", AMBER)
        };
        let path = ed.path.clone();

        ui.horizontal_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(r, 9, CARD);
            icons::paint(ui.painter(), r.shrink(9.0), Icon::Image, ACCENT_HI);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(capture::shorten(&name, 44)).strong().size(14.0));
                    chip(ui, status.0, status.1);
                });
                ui.label(RichText::new(detail).size(12.0).color(FAINT));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if theme::icon_button(ui, Icon::Close, false, true, "Close this screenshot").clicked() {
                    self.editor = None;
                    self.drawing = None;
                    return;
                }
                if let Some(path) = &path {
                    if theme::soft_button(ui, Icon::Folder, "Show in folder").clicked() {
                        platform::show_in_folder(path);
                    }
                }
                if theme::soft_button(ui, Icon::Save, "Save as…").clicked() {
                    let ext = self.s.image_format.extension();
                    let stem = name.rsplit_once('.').map(|(s, _)| s.to_string()).unwrap_or_else(|| "Screenshot".into());
                    let _ = std::fs::create_dir_all(&self.s.shots_dir);
                    let dialog = rfd::FileDialog::new()
                        .set_title("Save screenshot")
                        .set_directory(&self.s.shots_dir)
                        .set_file_name(format!("{stem}.{ext}"))
                        .add_filter("PNG picture", &["png"])
                        .add_filter("JPEG picture", &["jpg", "jpeg"]);
                    if let Some(to) = dialog.save_file() {
                        self.save_editor(Some(to), true);
                    }
                }
                if theme::big_button(ui, Some(Icon::Save), "Save", ACCENT, ACCENT_HI, vec2(96.0, 34.0)).clicked() {
                    self.save_editor(None, true);
                }
                if theme::soft_button(ui, Icon::Copy, "Copy").clicked() {
                    self.copy_editor();
                    self.toast("Copied to the clipboard", ToastKind::Success);
                }
            });
        });
    }

    // -----------------------------------------------------------------------
    // Record page
    // -----------------------------------------------------------------------

    fn record_sidebar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let recording = self.recording.is_some();
        ui.label(RichText::new("New recording").size(19.0).strong());
        ui.label(RichText::new("Record your screen to a video file.").color(MUTED));
        ui.add_space(4.0);

        ui.add_enabled_ui(!recording && self.rec_countdown.is_none(), |ui| {
            self.mode_tiles(ui, true);
            ui.add_space(2.0);
            match self.s.video_mode {
                Mode::Screen => self.screen_combo(ui, "rec-screen", true),
                Mode::Window => self.window_combo(ui, "rec-window", true),
                Mode::Area => {
                    let text = match &self.s.video_region {
                        Some(Target::Region { w, h, .. }) => format!("{w} × {h} area chosen"),
                        _ => "No area chosen yet".into(),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(text).color(MUTED));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if theme::soft_button(ui, Icon::Area, "Choose area").clicked() {
                                self.start_area_pick(&ctx, true, false);
                            }
                        });
                    });
                }
            }

            theme::section(ui, "Format");
            theme::segmented(ui, &mut self.s.format, &[(VideoFormat::Mp4, "MP4 video"), (VideoFormat::Gif, "Animated GIF")]);
            if self.s.format == VideoFormat::Gif {
                ui.label(
                    RichText::new(format!("GIFs are kept under {GIF_MAX_SIDE} px wide so they stay small."))
                        .size(12.0)
                        .color(FAINT),
                );
            }
            theme::section(ui, "Frames per second");
            theme::segmented(ui, &mut self.s.fps, &[(15, "15"), (24, "24"), (30, "30"), (60, "60")]);
            theme::section(ui, "Quality");
            theme::segmented(
                ui,
                &mut self.s.quality,
                &[(Quality::Low, "Small file"), (Quality::Medium, "Balanced"), (Quality::High, "Sharp")],
            );
            theme::section(ui, "Size");
            theme::segmented(ui, &mut self.s.half_size, &[(false, "Full size"), (true, "Half size")]);
            theme::section(ui, "Sound");
            if self.s.format == VideoFormat::Gif {
                ui.label(RichText::new("GIFs can't hold sound — choose MP4 to record it.").size(12.5).color(FAINT));
            } else if !cfg!(windows) {
                ui.label(RichText::new("Sound recording needs Windows for now.").size(12.5).color(FAINT));
            } else {
                theme::toggle(ui, &mut self.s.system_audio, "Computer sound", Some("What plays through your speakers"));
                theme::toggle(ui, &mut self.s.mic_audio, "Microphone", Some("Your voice, from the default microphone"));
            }
            theme::section(ui, "Options");
            theme::toggle(ui, &mut self.s.pointer, "Show the mouse pointer", None);
            theme::toggle(ui, &mut self.s.clicks, "Highlight clicks", None);
            theme::toggle(ui, &mut self.s.countdown, "Count down 3 seconds first", None);
        });
    }

    /// The pinned buttons at the bottom of the record panel (laid out from
    /// the bottom up: the shortcut hint first, then the buttons above it).
    fn record_actions(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new("Or press").size(12.0).color(FAINT));
            theme::keys(ui, &["Alt", "Shift", "R"]);
            ui.label(RichText::new("anywhere").size(12.0).color(FAINT));
        });
        ui.add_space(4.0);
        let width = ui.available_width();
        if let Some(rec) = &self.recording {
            let stopping = rec.is_stopping();
            let paused = rec.is_paused();
            if stopping {
                theme::big_button(ui, Some(Icon::Timer), "Finishing the video…", CARD_HI, CARD_HI, vec2(width, 48.0));
            } else {
                let mut stop = false;
                let mut pause = false;
                ui.horizontal(|ui| {
                    let half = (width - 8.0) / 2.0;
                    if theme::big_button(ui, Some(Icon::Stop), "Stop", RED, RED_HI, vec2(half, 48.0)).clicked() {
                        stop = true;
                    }
                    let (icon, text) = if paused { (Icon::Play, "Resume") } else { (Icon::Pause, "Pause") };
                    if theme::big_button(ui, Some(icon), text, CARD_HI, Color32::from_rgb(48, 54, 72), vec2(half, 48.0))
                        .clicked()
                    {
                        pause = true;
                    }
                });
                if stop {
                    self.toggle_recording(&ctx);
                } else if pause {
                    if let Some(rec) = &self.recording {
                        rec.toggle_pause();
                    }
                }
            }
        } else if self.rec_countdown.is_some() {
            if theme::big_button(ui, Some(Icon::Close), "Cancel", CARD_HI, BORDER, vec2(width, 48.0)).clicked() {
                self.cancel_countdown();
            }
        } else if theme::big_button(ui, Some(Icon::Video), "Start recording", RED, RED_HI, vec2(width, 48.0)).clicked() {
            self.start_recording(&ctx);
        }
    }

    fn record_main(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let has_last = self.last_video.is_some() && self.recording.is_none();
        let bottom_h = if has_last { 96.0 } else { 0.0 };
        let gap = if has_last { 14.0 } else { 0.0 };
        let preview_rect = Rect::from_min_max(full.min, pos2(full.right(), full.bottom() - bottom_h - gap));
        self.preview_card(ui, preview_rect);
        if has_last {
            let r = Rect::from_min_max(pos2(full.left(), full.bottom() - bottom_h), full.max);
            ui.scope_builder(UiBuilder::new().max_rect(r), |ui| self.last_video_card(ui));
        }
    }

    fn preview_card(&mut self, ui: &mut Ui, rect: Rect) {
        let recording = self.recording.is_some();
        let painter = ui.painter_at(rect);
        let border = if recording {
            let t = ui.input(|i| i.time) as f32;
            Stroke::new(2.0, RED.gamma_multiply(0.6 + 0.4 * (t * 3.0).sin().abs()))
        } else {
            Stroke::new(1.0, BORDER)
        };
        painter.rect(rect, 16, Color32::from_rgb(17, 20, 28), border, StrokeKind::Inside);

        // Header.
        let header = Rect::from_min_size(rect.min + vec2(18.0, 12.0), vec2(rect.width() - 36.0, 32.0));
        ui.scope_builder(UiBuilder::new().max_rect(header).layout(Layout::left_to_right(Align::Center)), |ui| {
            if recording {
                let t = ui.input(|i| i.time);
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                let paused = self.recording.as_ref().is_some_and(|r| r.is_paused());
                let a = if paused { 0.5 } else { 0.55 + 0.45 * ((t * 4.0).sin() as f32).abs() };
                ui.painter().circle_filled(r.center(), 5.5, RED.gamma_multiply(a));
                ui.label(RichText::new(if paused { "Paused" } else { "Recording" }).strong().color(TEXT));
            } else {
                ui.label(RichText::new("Preview").strong().color(TEXT));
                ui.label(RichText::new("what will be recorded").color(FAINT).size(12.5));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !recording {
                    let mut on = self.s.live_preview;
                    ui.allocate_ui(vec2(140.0, 28.0), |ui| {
                        theme::toggle(ui, &mut on, "Live preview", None);
                    });
                    if on != self.s.live_preview {
                        self.s.live_preview = on;
                        if !on {
                            self.preview = None;
                        }
                    }
                }
            });
        });

        let body = Rect::from_min_max(
            pos2(rect.left() + 18.0, header.bottom() + 10.0),
            pos2(rect.right() - 18.0, rect.bottom() - 54.0),
        );

        if let Some(rec) = &self.recording {
            let stats = rec.stats();
            let paused = rec.is_paused();
            if let Some(tex) = &self.preview {
                paint_contain(&painter, body, tex, Color32::from_white_alpha(50));
            }
            let c = body.center();
            painter.text(
                c - vec2(0.0, 18.0),
                Align2::CENTER_CENTER,
                library::human_duration(stats.elapsed),
                FontId::monospace(64.0),
                if paused { MUTED } else { TEXT },
            );
            let size = if rec.settings.format == VideoFormat::Mp4 {
                library::human_size(stats.bytes)
            } else {
                format!("{} frames", stats.encoded)
            };
            painter.text(
                c + vec2(0.0, 34.0),
                Align2::CENTER_CENTER,
                format!(
                    "{} × {}  ·  {} fps  ·  {}{}",
                    stats.width,
                    stats.height,
                    rec.settings.fps,
                    size,
                    if stats.audio { "  ·  with sound" } else { "" }
                ),
                FontId::proportional(14.0),
                MUTED,
            );
            if stats.captured > 30 && stats.dropped * 10 > stats.captured {
                painter.text(
                    c + vec2(0.0, 60.0),
                    Align2::CENTER_CENTER,
                    "Your computer is skipping frames — try Half size or fewer frames per second.",
                    FontId::proportional(12.5),
                    AMBER,
                );
            }
        } else if !self.s.live_preview {
            empty_hint(&painter, body, Icon::Video, "Live preview is off", "Turn it on to see what will be recorded.");
        } else if let Some(tex) = &self.preview {
            paint_contain(&painter, body, tex, Color32::WHITE);
        } else if let Some(err) = &self.preview_error {
            let (title, sub) = match self.s.video_mode {
                Mode::Window if self.video_window.is_none() => ("Choose a window", "Pick the window to record on the left."),
                Mode::Area if self.s.video_region.is_none() => ("Choose an area", "Press “Choose area” and drag over your screen."),
                _ => ("Preview unavailable", err.as_str()),
            };
            empty_hint(&painter, body, icon_for_mode(self.s.video_mode), title, sub);
        } else {
            empty_hint(&painter, body, Icon::Video, "Getting the preview…", "");
        }

        // Footer: what the file will be.
        let footer = Rect::from_min_max(
            pos2(rect.left() + 18.0, rect.bottom() - 46.0),
            pos2(rect.right() - 18.0, rect.bottom() - 12.0),
        );
        let info = self.output_summary();
        let dir = self.s.videos_dir.clone();
        ui.scope_builder(UiBuilder::new().max_rect(footer).layout(Layout::left_to_right(Align::Center)), |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui.painter(), r, Icon::Film, FAINT);
            ui.label(RichText::new(info).size(12.5).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(egui::Label::new(RichText::new("Open videos folder").size(12.5).color(ACCENT_HI)).sense(Sense::click()))
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .clicked()
                {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = open::that(&dir);
                }
            });
        });

        if recording {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }

    fn output_summary(&self) -> String {
        let source = match self.s.video_mode {
            Mode::Screen => self
                .monitor_for(self.s.video_screen)
                .map(|m| (m.width, m.height))
                .unwrap_or((0, 0)),
            Mode::Window => self
                .video_window
                .and_then(|id| self.windows.iter().find(|w| w.id == id))
                .map(|w| (w.width, w.height))
                .unwrap_or((0, 0)),
            Mode::Area => match &self.s.video_region {
                Some(Target::Region { w, h, .. }) => (*w, *h),
                _ => (0, 0),
            },
        };
        let format = match self.s.format {
            VideoFormat::Mp4 => "MP4",
            VideoFormat::Gif => "GIF",
        };
        if source.0 == 0 {
            return format!("{format}  ·  {} fps", self.s.fps);
        }
        let (mut w, mut h) = source;
        if self.s.half_size {
            w /= 2;
            h /= 2;
        }
        if self.s.format == VideoFormat::Gif {
            (w, h) = capture::fit_within(w, h, GIF_MAX_SIDE, GIF_MAX_SIDE);
        }
        format!("{format}  ·  {} × {}  ·  {} fps  ·  saved to your Videos folder", w & !1, h & !1, self.s.fps)
    }

    fn last_video_card(&mut self, ui: &mut Ui) {
        let Some(v) = self.last_video.clone() else { return };
        let rect = ui.max_rect();
        ui.painter().rect(rect, 14, CARD, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let inner = rect.shrink2(vec2(14.0, 12.0));
        let ctx = ui.ctx().clone();
        ui.scope_builder(UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)), |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(112.0, inner.height()), Sense::hover());
            ui.painter().rect_filled(r, 8, BG);
            let item = Item {
                path: v.path.clone(),
                name: String::new(),
                kind: if v.path.extension().is_some_and(|e| e == "gif") { Kind::Gif } else { Kind::Video },
                modified: std::time::SystemTime::now(),
                size: v.bytes,
            };
            self.want_thumb(&ctx, &item);
            if let Some(tex) = self.lib_thumbs.get(&v.path) {
                paint_cover(ui.painter(), r, tex);
            } else {
                icons::paint(ui.painter(), r.shrink(22.0), Icon::Film, FAINT);
            }
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Recording saved").strong());
                    chip(ui, "Done", GREEN);
                });
                let name = v.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                ui.label(RichText::new(capture::shorten(&name, 48)).color(MUTED).size(13.0));
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {} × {}  ·  {}",
                        library::human_duration(v.duration),
                        v.width,
                        v.height,
                        library::human_size(v.bytes)
                    ))
                    .color(FAINT)
                    .size(12.0),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(ui, Icon::Close, false, true, "Dismiss").clicked() {
                    self.last_video = None;
                }
                if theme::soft_button(ui, Icon::Folder, "Show in folder").clicked() {
                    platform::show_in_folder(&v.path);
                }
                if theme::big_button(ui, Some(Icon::Play), "Play", ACCENT, ACCENT_HI, vec2(92.0, 34.0)).clicked() {
                    let _ = open::that(&v.path);
                }
            });
        });
    }

    // -----------------------------------------------------------------------
    // Library page
    // -----------------------------------------------------------------------

    fn library_view(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let items: Vec<Item> = self
            .lib_items
            .iter()
            .filter(|i| match self.lib_filter {
                LibFilter::All => true,
                LibFilter::Pictures => i.kind == Kind::Image,
                LibFilter::Videos => i.kind != Kind::Image,
            })
            .cloned()
            .collect();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Library").size(24.0).strong());
            ui.label(RichText::new(format!("{} items", items.len())).color(FAINT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::soft_button(ui, Icon::Folder, "Videos folder").clicked() {
                    let _ = std::fs::create_dir_all(&self.s.videos_dir);
                    let _ = open::that(&self.s.videos_dir);
                }
                if theme::soft_button(ui, Icon::Folder, "Pictures folder").clicked() {
                    let _ = std::fs::create_dir_all(&self.s.shots_dir);
                    let _ = open::that(&self.s.shots_dir);
                }
                ui.allocate_ui(vec2(300.0, 30.0), |ui| {
                    theme::segmented(
                        ui,
                        &mut self.lib_filter,
                        &[(LibFilter::All, "All"), (LibFilter::Pictures, "Screenshots"), (LibFilter::Videos, "Videos")],
                    );
                });
            });
        });
        ui.add_space(10.0);

        if items.is_empty() {
            let rect = ui.available_rect_before_wrap();
            ui.allocate_rect(rect, Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect(rect, 18, Color32::from_rgb(17, 20, 28), Stroke::new(1.0, BORDER), StrokeKind::Inside);
            empty_hint(
                &painter,
                rect,
                Icon::Library,
                "Nothing here yet",
                "Your screenshots and recordings will show up here.",
            );
            return;
        }

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let gap = 14.0;
            let avail = ui.available_width();
            let cols = ((avail + gap) / (230.0 + gap)).floor().max(1.0) as usize;
            let card_w = ((avail - gap * (cols as f32 - 1.0)) / cols as f32).floor();
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for item in row {
                        self.lib_card(ui, &ctx, item, card_w);
                    }
                });
                ui.add_space(gap - 8.0);
            }
        });
    }

    fn lib_card(&mut self, ui: &mut Ui, ctx: &egui::Context, item: &Item, w: f32) {
        self.want_thumb(ctx, item);
        let thumb_h = (w * 0.6).round();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, thumb_h + 58.0), Sense::click());
        let hovered = resp.hovered();
        let painter = ui.painter();
        painter.rect_filled(rect, 14, if hovered { CARD_HI } else { CARD });
        let thumb = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(w - 12.0, thumb_h - 6.0));
        painter.rect_filled(thumb, 10, BG);
        if let Some(tex) = self.lib_thumbs.get(&item.path) {
            paint_cover(painter, thumb, tex);
        } else {
            let icon = if item.kind == Kind::Image { Icon::Image } else { Icon::Film };
            icons::paint(painter, Rect::from_center_size(thumb.center(), vec2(34.0, 34.0)), icon, FAINT);
        }
        if item.kind != Kind::Image {
            let label = if item.kind == Kind::Gif { "GIF" } else { "VIDEO" };
            let badge = Rect::from_min_size(
                thumb.min + vec2(8.0, 8.0),
                vec2(if item.kind == Kind::Gif { 50.0 } else { 66.0 }, 22.0),
            );
            painter.rect_filled(badge, 6, Color32::from_black_alpha(170));
            icons::paint(painter, Rect::from_min_size(badge.min + vec2(6.0, 4.0), vec2(14.0, 14.0)), Icon::Play, Color32::WHITE);
            painter.text(
                badge.left_center() + vec2(24.0, 0.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
        painter.text(
            pos2(rect.left() + 12.0, thumb.bottom() + 12.0),
            Align2::LEFT_TOP,
            capture::shorten(&item.name, ((w - 24.0) / 7.2) as usize),
            FontId::proportional(13.0),
            TEXT,
        );
        painter.text(
            pos2(rect.left() + 12.0, thumb.bottom() + 31.0),
            Align2::LEFT_TOP,
            format!("{}  ·  {}", item.when(), library::human_size(item.size)),
            FontId::proportional(11.5),
            FAINT,
        );
        if hovered {
            painter.rect_stroke(rect, 14, Stroke::new(1.5, ACCENT), StrokeKind::Inside);
        }
        let tip = if item.kind == Kind::Image { "Click to open and mark up" } else { "Click to play" };
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
        if resp.clicked() {
            if item.kind == Kind::Image {
                self.open_in_editor(item.path.clone());
            } else {
                let _ = open::that(&item.path);
            }
        }
        resp.context_menu(|ui| {
            if ui.button("Open").clicked() {
                let _ = open::that(&item.path);
                ui.close();
            }
            if item.kind == Kind::Image && ui.button("Mark up in LabCapture").clicked() {
                self.open_in_editor(item.path.clone());
                ui.close();
            }
            if ui.button("Show in folder").clicked() {
                platform::show_in_folder(&item.path);
                ui.close();
            }
            ui.separator();
            if ui.button(RichText::new("Delete…").color(RED_HI)).clicked() {
                self.confirm_delete = Some(item.path.clone());
                ui.close();
            }
        });
    }

    fn delete_modal(&mut self, ctx: &egui::Context) {
        let Some(path) = self.confirm_delete.clone() else { return };
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut decision: Option<bool> = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm-delete")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new("Delete this file?").size(18.0).strong());
            ui.label(RichText::new(capture::shorten(&name, 52)).color(MUTED));
            ui.label(RichText::new("It is removed from your computer for good.").color(FAINT).size(12.5));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if theme::big_button(ui, Some(Icon::Trash), "Delete", RED, RED_HI, vec2(120.0, 36.0)).clicked() {
                    decision = Some(true);
                }
                if theme::soft_button(ui, Icon::Close, "Keep it").clicked() {
                    decision = Some(false);
                }
            });
        });
        if modal.should_close() && decision.is_none() {
            decision = Some(false);
        }
        match decision {
            Some(true) => {
                self.confirm_delete = None;
                self.delete_file(&path);
            }
            Some(false) => self.confirm_delete = None,
            None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Settings page
    // -----------------------------------------------------------------------

    fn settings_view(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let width = ui.available_width().min(680.0);
            let pad = ((ui.available_width() - width) / 2.0).max(0.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.label(RichText::new("Settings").size(24.0).strong());
                    ui.add_space(8.0);

                    settings_card(ui, "Where captures go", |ui| {
                        let mut rescan = false;
                        for (label, is_video) in [("Screenshots", false), ("Videos", true)] {
                            let dir = if is_video { self.s.videos_dir.clone() } else { self.s.shots_dir.clone() };
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(RichText::new(label).strong());
                                    ui.label(
                                        RichText::new(capture::shorten(&dir.display().to_string(), 60))
                                            .size(12.0)
                                            .color(FAINT),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if theme::soft_button(ui, Icon::Open, "Open").clicked() {
                                        let _ = std::fs::create_dir_all(&dir);
                                        let _ = open::that(&dir);
                                    }
                                    if theme::soft_button(ui, Icon::Folder, "Change…").clicked() {
                                        let _ = std::fs::create_dir_all(&dir);
                                        if let Some(new) = rfd::FileDialog::new().set_directory(&dir).pick_folder() {
                                            if is_video {
                                                self.s.videos_dir = new;
                                            } else {
                                                self.s.shots_dir = new;
                                            }
                                            rescan = true;
                                        }
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                        if rescan {
                            self.rescan_library();
                        }
                    });

                    settings_card(ui, "Screenshots", |ui| {
                        ui.label(RichText::new("File type").color(MUTED));
                        theme::segmented(
                            ui,
                            &mut self.s.image_format,
                            &[(ImageFormat::Png, "PNG — sharpest"), (ImageFormat::Jpg, "JPG — smaller files")],
                        );
                    });

                    settings_card(ui, "Privacy", |ui| {
                        let hint = if platform::can_exclude_from_capture() {
                            "LabCapture's own window never shows up in your screenshots or videos."
                        } else {
                            "LabCapture hides its window for a moment while it takes a screenshot."
                        };
                        theme::toggle(ui, &mut self.s.keep_out_of_captures, "Keep LabCapture out of captures", Some(hint));
                    });

                    settings_card(ui, "Shortcuts", |ui| {
                        let mut on = self.s.hotkeys;
                        theme::toggle(
                            ui,
                            &mut on,
                            "Keyboard shortcuts anywhere",
                            Some("They work even when LabCapture is in the background."),
                        );
                        if on != self.s.hotkeys {
                            self.s.hotkeys = on;
                            if on && !self.is_self_shot() {
                                self.enable_hotkeys(&ctx);
                            } else {
                                self.disable_hotkeys();
                            }
                        }
                        for (label, key) in [
                            ("Screenshot of the whole screen", "S"),
                            ("Screenshot of an area", "A"),
                            ("Screenshot of the window in front", "W"),
                            ("Start or stop recording", "R"),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(label).color(MUTED));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    theme::keys(ui, &[key, "Shift", "Alt"]);
                                });
                            });
                        }
                        if let Some(err) = &self.hotkey_error {
                            ui.label(RichText::new(err).color(AMBER).size(12.5));
                        }
                    });

                    settings_card(ui, "About", |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                            theme::logo(ui.painter(), r);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(format!("LabCapture {}", env!("CARGO_PKG_VERSION"))).strong());
                                ui.label(
                                    RichText::new("Screenshots and screen recordings. Made with Rust.")
                                        .size(12.5)
                                        .color(FAINT),
                                );
                            });
                        });
                    });
                });
            });
        });
    }

    // -----------------------------------------------------------------------
    // Overlays
    // -----------------------------------------------------------------------

    fn picker_viewport(&mut self, ctx: &egui::Context) {
        let Some(picker) = self.picker.as_mut() else { return };
        let m = picker.monitor.clone();
        let ppp = m.scale.max(0.5);
        let builder = egui::ViewportBuilder::default()
            .with_title("LabCapture — choose an area")
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_position(pos2(m.x as f32 / ppp, m.y as f32 / ppp))
            .with_inner_size(vec2(m.width as f32 / ppp, m.height as f32 / ppp))
            .with_fullscreen(true);
        let outcome = ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("lapcapture-area-picker"),
            builder,
            |ui, _class| picker_ui(ui, picker),
        );
        if !matches!(outcome, PickOutcome::Open) {
            self.finish_pick(ctx, outcome);
        }
    }

    fn countdown_overlay(&mut self, ctx: &egui::Context) {
        let until = match (&self.pending, &self.rec_countdown) {
            (Pending::Countdown { until, .. }, _) => Some((*until, false)),
            (_, Some((at, _))) => Some((*at, true)),
            _ => None,
        };
        let Some((until, recording)) = until else { return };
        let left = until.saturating_duration_since(std::time::Instant::now());
        let secs = left.as_secs() + 1;
        let frac = left.as_secs_f32().fract().clamp(0.0, 1.0);
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("countdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let painter = ui.painter();
                painter.rect_filled(screen, 0, Color32::from_black_alpha(150));
                let c = screen.center();
                let color = if recording { RED } else { ACCENT };
                painter.circle_filled(c, 86.0, Color32::from_rgb(22, 25, 34));
                painter.circle_stroke(c, 86.0, Stroke::new(2.0, BORDER));
                // Progress ring for the current second.
                let n = 48;
                let pts: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac * i as f32 / n as f32;
                        c + vec2(a.cos(), a.sin()) * 86.0
                    })
                    .collect();
                painter.add(Shape::line(pts, Stroke::new(5.0, color)));
                painter.text(c, Align2::CENTER_CENTER, secs.to_string(), FontId::proportional(72.0), TEXT);
                let label = if recording { "Recording starts in…" } else { "Screenshot in…" };
                painter.text(c + vec2(0.0, 120.0), Align2::CENTER_CENTER, label, FontId::proportional(18.0), TEXT);
                painter.text(
                    c + vec2(0.0, 146.0),
                    Align2::CENTER_CENTER,
                    "Press Esc to cancel",
                    FontId::proportional(13.0),
                    MUTED,
                );
                ui.allocate_rect(screen, Sense::click());
            });
        ctx.request_repaint_after(Duration::from_millis(30));
    }

    fn toasts_overlay(&mut self, ctx: &egui::Context) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::unique("toasts"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                for t in &self.toasts {
                    let age = t.at.elapsed().as_secs_f32();
                    let alpha = if age > 3.8 { ((4.2 - age) / 0.4).clamp(0.0, 1.0) } else { (age / 0.15).clamp(0.0, 1.0) };
                    ui.scope(|ui| {
                        ui.multiply_opacity(alpha);
                        let color = match t.kind {
                            ToastKind::Info => ACCENT,
                            ToastKind::Success => GREEN,
                            ToastKind::Error => RED,
                        };
                        Frame::new()
                            .fill(CARD_HI)
                            .stroke(Stroke::new(1.0, BORDER))
                            .corner_radius(12)
                            .inner_margin(Margin::symmetric(14, 10))
                            .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(100) })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                                    let icon = if t.kind == ToastKind::Error { Icon::Close } else { Icon::Check };
                                    ui.painter().circle_filled(r.center(), 10.0, color.gamma_multiply(0.25));
                                    icons::paint(ui.painter(), r.shrink(3.0), icon, color);
                                    ui.label(RichText::new(&t.text).color(TEXT).size(13.5));
                                });
                            });
                    });
                    ui.add_space(4.0);
                }
            });
    }
}

// ---------------------------------------------------------------------------
// The full-screen area picker
// ---------------------------------------------------------------------------

fn picker_ui(ui: &mut Ui, p: &mut Picker) -> PickOutcome {
    p.frames += 1;
    let ctx = ui.ctx().clone();
    if p.frames == 2 {
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
    let rect = ctx.content_rect();
    let painter = ui.painter().clone();
    painter.image(p.texture.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

    let resp = ui.interact(rect, ui.scope_id().with("area-drag"), Sense::click_and_drag());
    ctx.set_cursor_icon(CursorIcon::Crosshair);
    let (iw, ih) = p.image.dimensions();
    let sx = iw as f32 / rect.width().max(1.0);
    let sy = ih as f32 / rect.height().max(1.0);
    let pointer = ctx.input(|i| i.pointer.latest_pos());

    if resp.drag_started() {
        p.drag_from = ctx.input(|i| i.pointer.press_origin()).or(pointer);
    }
    let dragging = resp.dragged() || resp.drag_stopped();
    let sel = match (p.drag_from, dragging, pointer) {
        (Some(a), true, Some(b)) => Some(Rect::from_two_pos(a, b).intersect(rect)),
        _ => None,
    };

    let shade = Color32::from_black_alpha(140);
    match sel {
        Some(s) => {
            painter.rect_filled(Rect::from_min_max(rect.min, pos2(rect.right(), s.top())), 0, shade);
            painter.rect_filled(Rect::from_min_max(pos2(rect.left(), s.bottom()), rect.max), 0, shade);
            painter.rect_filled(Rect::from_min_max(pos2(rect.left(), s.top()), pos2(s.left(), s.bottom())), 0, shade);
            painter.rect_filled(Rect::from_min_max(pos2(s.right(), s.top()), pos2(rect.right(), s.bottom())), 0, shade);
            painter.rect_stroke(s, 0, Stroke::new(2.0, ACCENT), StrokeKind::Outside);
            for c in [s.left_top(), s.right_top(), s.left_bottom(), s.right_bottom()] {
                painter.rect_filled(Rect::from_center_size(c, vec2(8.0, 8.0)), 2, Color32::WHITE);
            }
            let label = format!("{} × {}", (s.width() * sx).round() as u32, (s.height() * sy).round() as u32);
            let pos = if s.top() > rect.top() + 40.0 { s.left_top() - vec2(0.0, 10.0) } else { s.left_top() + vec2(8.0, 30.0) };
            pill(&painter, pos, Align2::LEFT_BOTTOM, &label, FontId::proportional(13.0));
        }
        None => {
            painter.rect_filled(rect, 0, Color32::from_black_alpha(80));
            if let Some(pt) = pointer {
                let st = Stroke::new(1.0, Color32::from_white_alpha(140));
                painter.line_segment([pos2(rect.left(), pt.y), pos2(rect.right(), pt.y)], st);
                painter.line_segment([pos2(pt.x, rect.top()), pos2(pt.x, rect.bottom())], st);
                let coords = format!(
                    "{}, {}",
                    ((pt.x - rect.left()) * sx).round() as i32,
                    ((pt.y - rect.top()) * sy).round() as i32
                );
                pill(&painter, pt + vec2(14.0, 26.0), Align2::LEFT_TOP, &coords, FontId::proportional(12.0));
            }
        }
    }

    let text = if p.for_video {
        "Drag to choose the area to record   ·   Esc to cancel"
    } else {
        "Drag to choose an area   ·   Esc to cancel"
    };
    pill(&painter, pos2(rect.center().x, rect.top() + 28.0), Align2::CENTER_TOP, text, FontId::proportional(15.0));

    let cancel = ctx.input(|i| i.key_pressed(Key::Escape) || i.viewport().close_requested()) || resp.secondary_clicked();
    if cancel {
        return PickOutcome::Cancel;
    }
    if resp.drag_stopped() {
        p.drag_from = None;
        if let Some(s) = sel {
            let x = ((s.left() - rect.left()) * sx).round().max(0.0) as u32;
            let y = ((s.top() - rect.top()) * sy).round().max(0.0) as u32;
            let w = ((s.width() * sx).round() as u32).min(iw.saturating_sub(x));
            let h = ((s.height() * sy).round() as u32).min(ih.saturating_sub(y));
            if w >= 8 && h >= 8 {
                return PickOutcome::Done { x, y, w, h };
            }
        }
    }
    ctx.request_repaint();
    PickOutcome::Open
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn settings_card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(15.0).strong());
        ui.add_space(4.0);
        add(ui);
    });
    ui.add_space(10.0);
}

fn pill(painter: &egui::Painter, pos: Pos2, anchor: Align2, text: &str, font: FontId) {
    let galley = painter.layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let size = galley.size() + vec2(24.0, 12.0);
    let rect = anchor.anchor_size(pos, size);
    painter.rect(
        rect,
        9,
        Color32::from_rgba_unmultiplied(16, 18, 26, 230),
        Stroke::new(1.0, Color32::from_white_alpha(40)),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(12.0, 6.0), galley, Color32::WHITE);
}

fn nav_pill(ui: &mut Ui, selected: bool, icon: Icon, label: &str) -> egui::Response {
    let font = FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, TEXT);
    let size = vec2(galley.size().x + 50.0, 38.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect(rect, 10, CARD_HI, Stroke::new(1.0, Color32::from_rgb(58, 66, 88)), StrokeKind::Inside);
    } else if resp.hovered() {
        painter.rect_filled(rect, 10, CARD);
    }
    let color = if selected { ACCENT_HI } else if resp.hovered() { TEXT } else { MUTED };
    icons::paint(painter, Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(17.0, 17.0)), icon, color);
    let text_color = if selected { TEXT } else { color };
    let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), text_color);
    painter.galley(pos2(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0), galley, text_color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn rec_pill(ui: &mut Ui, time: &str, paused: bool) -> egui::Response {
    let font = FontId::monospace(13.0);
    let galley = ui.painter().layout_no_wrap(time.to_string(), font, Color32::WHITE);
    let size = vec2(galley.size().x + 70.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 16, if resp.hovered() { RED_HI } else { RED });
    let t = ui.input(|i| i.time) as f32;
    let a = if paused { 0.5 } else { 0.6 + 0.4 * (t * 4.0).sin().abs() };
    painter.circle_filled(pos2(rect.left() + 16.0, rect.center().y), 5.0, Color32::WHITE.gamma_multiply(a));
    painter.galley(pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
    icons::paint(
        painter,
        Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(14.0, 14.0)),
        Icon::Stop,
        Color32::WHITE,
    );
    resp.on_hover_text("Stop recording").on_hover_cursor(CursorIcon::PointingHand)
}

fn chip(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.16))
        .corner_radius(6)
        .inner_margin(Margin::symmetric(7, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).color(color));
        });
}

fn divider(ui: &mut Ui) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 26.0), Sense::hover());
    ui.painter().rect_filled(r, 0, BORDER);
    ui.add_space(4.0);
}

fn hint_box(ui: &mut Ui, icon: Icon, text: &str) {
    Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(9)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                icons::paint(ui.painter(), r, icon, ACCENT_HI);
                ui.add(egui::Label::new(RichText::new(text).size(12.5).color(MUTED)).wrap());
            });
        });
}

fn soft_width(ui: &Ui, text: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.5), TEXT);
    galley.size().x + 44.0
}

fn icon_for_mode(mode: Mode) -> Icon {
    match mode {
        Mode::Screen => Icon::Screen,
        Mode::Window => Icon::Window,
        Mode::Area => Icon::Area,
    }
}

fn empty_hint(painter: &egui::Painter, rect: Rect, icon: Icon, title: &str, sub: &str) {
    let c = rect.center();
    let ir = Rect::from_center_size(c - vec2(0.0, 40.0), vec2(64.0, 64.0));
    painter.rect_filled(ir, 18, CARD);
    painter.rect_stroke(ir, 18, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    icons::paint(painter, ir.shrink(18.0), icon, MUTED);
    painter.text(c + vec2(0.0, 12.0), Align2::CENTER_TOP, title, FontId::proportional(17.0), TEXT);
    if !sub.is_empty() {
        painter.text(c + vec2(0.0, 38.0), Align2::CENTER_TOP, sub, FontId::proportional(13.0), MUTED);
    }
}

fn fit_rect(area: Rect, size: Vec2) -> Rect {
    let s = (area.width() / size.x.max(1.0)).min(area.height() / size.y.max(1.0));
    Rect::from_center_size(area.center(), size * s)
}

/// Draw a texture so it fits inside `area`, keeping its shape.
fn paint_contain(painter: &egui::Painter, area: Rect, tex: &egui::TextureHandle, tint: Color32) {
    let r = fit_rect(area, tex.size_vec2());
    painter.rect_filled(r.translate(vec2(0.0, 6.0)), 6, Color32::from_black_alpha(60));
    painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
}

/// Draw a texture so it fills `area`, cropping what doesn't fit.
fn paint_cover(painter: &egui::Painter, area: Rect, tex: &egui::TextureHandle) {
    let size = tex.size_vec2();
    let target = area.width() / area.height().max(1.0);
    let src = size.x / size.y.max(1.0);
    let uv = if src > target {
        let w = target / src;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = src / target;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    };
    painter.image(tex.id(), area, uv, Color32::WHITE);
}

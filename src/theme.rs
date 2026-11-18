//! Colours, fonts and the small custom widgets that give LabCapture its look.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Frame, Margin, Rect, Response, RichText, Sense, Shadow, Stroke,
    StrokeKind, Ui, Vec2, vec2,
};

use crate::icons::{self, Icon};

pub const BG: Color32 = Color32::from_rgb(13, 15, 21);
pub const PANEL: Color32 = Color32::from_rgb(19, 22, 30);
pub const CARD: Color32 = Color32::from_rgb(27, 31, 42);
pub const CARD_HI: Color32 = Color32::from_rgb(35, 40, 54);
pub const BORDER: Color32 = Color32::from_rgb(44, 50, 66);
pub const TEXT: Color32 = Color32::from_rgb(233, 236, 243);
pub const MUTED: Color32 = Color32::from_rgb(140, 147, 168);
pub const FAINT: Color32 = Color32::from_rgb(92, 99, 120);
pub const ACCENT: Color32 = Color32::from_rgb(99, 132, 255);
pub const ACCENT_HI: Color32 = Color32::from_rgb(128, 156, 255);
pub const VIOLET: Color32 = Color32::from_rgb(150, 105, 255);
pub const RED: Color32 = Color32::from_rgb(255, 77, 94);
pub const RED_HI: Color32 = Color32::from_rgb(255, 108, 122);
pub const GREEN: Color32 = Color32::from_rgb(61, 220, 151);
pub const AMBER: Color32 = Color32::from_rgb(255, 196, 64);

pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals());
    ctx.set_visuals_of(egui::Theme::Light, visuals());
    ctx.all_styles_mut(|style| {
        use egui::{FontFamily, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(12.0, 6.0);
        style.spacing.interact_size = vec2(36.0, 30.0);
        style.spacing.combo_height = 320.0;
        style.spacing.menu_margin = Margin::same(8);
        style.visuals = visuals();
    });
}

fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = CARD;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(110) };
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = BG;
    v.faint_bg_color = CARD;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, ACCENT_HI);
    v.hyperlink_color = ACCENT_HI;
    let radius = CornerRadius::same(8);
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, PANEL, BORDER),
        (&mut v.widgets.inactive, CARD, BORDER),
        (&mut v.widgets.hovered, CARD_HI, Color32::from_rgb(64, 72, 94)),
        (&mut v.widgets.active, Color32::from_rgb(44, 50, 68), ACCENT),
        (&mut v.widgets.open, CARD_HI, BORDER),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.corner_radius = radius;
        w.fg_stroke = Stroke::new(1.0, TEXT);
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v
}

pub fn glow(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32) {
    // Many faint rings blend into a soft glow without visible bands.
    let rings = 40;
    for i in 0..rings {
        let t = i as f32 / rings as f32;
        let r = radius * (1.0 - t).powf(0.8);
        painter.circle_filled(center, r, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 2));
    }
}

pub fn card_frame() -> Frame {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(14))
}

/// Small uppercase heading above a group of settings.
pub fn section(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text.to_uppercase()).size(11.0).color(FAINT).strong());
}

/// A big square choice with an icon and a label (Screen / Window / Area).
pub fn tile(ui: &mut Ui, selected: bool, icon: Icon, label: &str, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let fill = if selected {
        ACCENT.gamma_multiply(0.18)
    } else if hovered {
        CARD_HI
    } else {
        CARD
    };
    let stroke = if selected { Stroke::new(1.5, ACCENT) } else { Stroke::new(1.0, BORDER) };
    let painter = ui.painter();
    painter.rect(rect, 12, fill, stroke, StrokeKind::Inside);
    let color = if selected { ACCENT_HI } else if hovered { TEXT } else { MUTED };
    let icon_rect = Rect::from_center_size(rect.center() - vec2(0.0, 10.0), vec2(26.0, 26.0));
    icons::paint(painter, icon_rect, icon, color);
    painter.text(
        rect.center_bottom() - vec2(0.0, 12.0),
        Align2::CENTER_BOTTOM,
        label,
        FontId::proportional(13.0),
        if selected { TEXT } else { MUTED },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of pill buttons, one of which is chosen.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let total_w = ui.available_width();
    let h = 30.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total_w, h), Sense::hover());
    ui.painter().rect(rect, 9, BG, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let n = options.len().max(1) as f32;
    let seg_w = (rect.width() - 6.0) / n;
    for (i, (opt, label)) in options.iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(3.0 + i as f32 * seg_w, 3.0), vec2(seg_w, h - 6.0));
        let resp = ui.interact(r, ui.scope_id().with(("seg", label, i)), Sense::click());
        let selected = *value == *opt;
        if selected {
            ui.painter().rect_filled(r, 7, CARD_HI);
            ui.painter().rect_stroke(r, 7, Stroke::new(1.0, Color32::from_rgb(70, 78, 100)), StrokeKind::Inside);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7, CARD);
        }
        ui.painter().text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::proportional(13.0),
            if selected { TEXT } else { MUTED },
        );
        if resp.clicked() && !selected {
            *value = *opt;
            changed = true;
        }
    }
    changed
}

/// A filled call-to-action button with an optional icon.
pub fn big_button(ui: &mut Ui, icon: Option<Icon>, text: &str, color: Color32, hover: Color32, size: Vec2) -> Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        color.gamma_multiply(0.85)
    } else if resp.hovered() {
        hover
    } else {
        color
    };
    let painter = ui.painter();
    painter.rect_filled(rect.translate(vec2(0.0, 2.0)), 12, Color32::from_black_alpha(60));
    painter.rect_filled(rect, 12, fill);
    let font = FontId::proportional(15.0);
    let galley = painter.layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let icon_w = if icon.is_some() { 26.0 } else { 0.0 };
    let total = galley.size().x + icon_w;
    let start = rect.center().x - total / 2.0;
    if let Some(icon) = icon {
        let ir = Rect::from_center_size(egui::pos2(start + 9.0, rect.center().y), vec2(18.0, 18.0));
        icons::paint(painter, ir, icon, Color32::WHITE);
    }
    painter.galley(
        egui::pos2(start + icon_w, rect.center().y - galley.size().y / 2.0),
        galley,
        Color32::WHITE,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A quiet button with an icon and text.
pub fn soft_button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let font = FontId::proportional(13.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, TEXT);
    let size = vec2(galley.size().x + 44.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if resp.hovered() { CARD_HI } else { CARD };
    let painter = ui.painter();
    painter.rect(rect, 9, fill, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let ir = Rect::from_center_size(egui::pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0));
    icons::paint(painter, ir, icon, if resp.hovered() { TEXT } else { MUTED });
    painter.galley(egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0), galley, TEXT);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A square icon-only button (used in the markup toolbar).
pub fn icon_button(ui: &mut Ui, icon: Icon, selected: bool, enabled: bool, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), if enabled { Sense::click() } else { Sense::hover() });
    let painter = ui.painter();
    if selected {
        painter.rect(rect, 9, ACCENT.gamma_multiply(0.22), Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    } else if resp.hovered() && enabled {
        painter.rect_filled(rect, 9, CARD_HI);
    }
    let color = if !enabled {
        FAINT.gamma_multiply(0.6)
    } else if selected {
        ACCENT_HI
    } else if resp.hovered() {
        TEXT
    } else {
        MUTED
    };
    icons::paint(painter, rect.shrink(9.0), icon, color);
    resp.on_hover_text(tip)
}

/// An on/off switch with a label on the left.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str, hint: Option<&str>) -> Response {
    let width = ui.available_width();
    let h = if hint.is_some() { 40.0 } else { 28.0 };
    let (rect, mut resp) = ui.allocate_exact_size(vec2(width, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let painter = ui.painter();
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let sw = Rect::from_min_size(egui::pos2(rect.right() - 38.0, rect.center().y - 10.0), vec2(38.0, 20.0));
    let track = lerp_color(Color32::from_rgb(52, 58, 76), ACCENT, t);
    painter.rect_filled(sw, 10, track);
    let knob_x = egui::lerp(sw.left() + 10.0..=sw.right() - 10.0, t);
    painter.circle_filled(egui::pos2(knob_x, sw.center().y), 7.5, Color32::WHITE);
    let text_color = if resp.hovered() { TEXT } else { Color32::from_rgb(210, 214, 225) };
    if let Some(hint) = hint {
        painter.text(rect.left_top() + vec2(0.0, 3.0), Align2::LEFT_TOP, label, FontId::proportional(13.5), text_color);
        painter.text(rect.left_bottom() - vec2(0.0, 3.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(11.5), FAINT);
    } else {
        painter.text(rect.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.5), text_color);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A rounded rectangle filled with a diagonal gradient from `c0` (top-left)
/// to `c1` (bottom-right).
pub fn gradient_rounded_rect(painter: &egui::Painter, rect: Rect, radius: f32, c0: Color32, c1: Color32) {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (egui::pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (egui::pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (egui::pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (egui::pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    let color_at = |p: egui::Pos2| {
        let t = ((p.x - rect.left()) + (p.y - rect.top())) / (rect.width() + rect.height()).max(1.0);
        lerp_color(c0, c1, t.clamp(0.0, 1.0))
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), color_at(rect.center()));
    let steps = 8;
    for (center, start) in corners {
        for i in 0..=steps {
            let a = (start + 90.0 * i as f32 / steps as f32).to_radians();
            let p = center + vec2(a.cos(), a.sin()) * r;
            mesh.colored_vertex(p, color_at(p));
        }
    }
    let n = mesh.vertices.len() as u32;
    for i in 1..n {
        let next = if i + 1 < n { i + 1 } else { 1 };
        mesh.add_triangle(0, i, next);
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// A keyboard shortcut drawn as little key caps.
pub fn keys(ui: &mut Ui, keys: &[&str]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (i, k) in keys.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new("+").size(11.0).color(FAINT));
            }
            Frame::new()
                .fill(BG)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(CornerRadius::same(5))
                .inner_margin(Margin::symmetric(6, 1))
                .show(ui, |ui| {
                    ui.label(RichText::new(*k).size(11.0).color(MUTED));
                });
        }
    });
}

/// The app logo: a rounded gradient square with a lens.
pub fn logo(painter: &egui::Painter, rect: Rect) {
    gradient_rounded_rect(painter, rect, rect.width() * 0.28, ACCENT, Color32::from_rgb(214, 92, 220));
    let c = rect.center();
    painter.circle_stroke(c, rect.width() * 0.26, Stroke::new(2.2, Color32::WHITE));
    painter.circle_filled(c, rect.width() * 0.1, Color32::WHITE);
    painter.circle_filled(c + vec2(rect.width() * 0.26, -rect.width() * 0.26), 2.2, Color32::WHITE);
}

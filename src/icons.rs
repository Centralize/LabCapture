//! Small line icons drawn with egui's painter, so they look the same on
//! every computer (no icon font needed).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // a few spares for later screens
pub enum Icon {
    Camera,
    Video,
    Screen,
    Window,
    Area,
    Library,
    Settings,
    Folder,
    Copy,
    Save,
    Pen,
    Highlighter,
    Rect,
    Arrow,
    Pixelate,
    Undo,
    Redo,
    Trash,
    Pause,
    Play,
    Stop,
    Timer,
    Pointer,
    Check,
    Close,
    Plus,
    Film,
    Image,
    Open,
    Keyboard,
}

pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let r = Rect::from_center_size(c, vec2(s, s));
    let p = |x: f32, y: f32| -> Pos2 { pos2(r.left() + x * s, r.top() + y * s) };
    let w = (s / 12.0).clamp(1.2, 2.6);
    let st = Stroke::new(w, color);
    let rr = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(p(x0, y0), p(x1, y1));
    let rad = (s * 0.12) as u8;

    match icon {
        Icon::Camera => {
            painter.rect_stroke(rr(0.08, 0.28, 0.92, 0.84), rad, st, StrokeKind::Middle);
            painter.line(vec![p(0.32, 0.28), p(0.38, 0.16), p(0.62, 0.16), p(0.68, 0.28)], st);
            painter.circle_stroke(p(0.5, 0.56), s * 0.16, st);
            painter.circle_filled(p(0.78, 0.40), s * 0.035, color);
        }
        Icon::Video => {
            painter.rect_stroke(rr(0.06, 0.26, 0.66, 0.78), rad, st, StrokeKind::Middle);
            painter.add(Shape::convex_polygon(
                vec![p(0.70, 0.46), p(0.94, 0.30), p(0.94, 0.74), p(0.70, 0.58)],
                Color32::TRANSPARENT,
                st,
            ));
        }
        Icon::Screen => {
            painter.rect_stroke(rr(0.06, 0.14, 0.94, 0.72), rad, st, StrokeKind::Middle);
            painter.line_segment([p(0.5, 0.72), p(0.5, 0.86)], st);
            painter.line_segment([p(0.30, 0.88), p(0.70, 0.88)], st);
        }
        Icon::Window => {
            painter.rect_stroke(rr(0.08, 0.14, 0.92, 0.86), rad, st, StrokeKind::Middle);
            painter.line_segment([p(0.08, 0.32), p(0.92, 0.32)], st);
            painter.circle_filled(p(0.18, 0.23), s * 0.03, color);
            painter.circle_filled(p(0.27, 0.23), s * 0.03, color);
        }
        Icon::Area => {
            let l = 0.22;
            for (x, y, dx, dy) in [(0.1, 0.1, 1.0, 1.0), (0.9, 0.1, -1.0, 1.0), (0.1, 0.9, 1.0, -1.0), (0.9, 0.9, -1.0, -1.0)] {
                painter.line(vec![p(x + dx * l, y), p(x, y), p(x, y + dy * l)], st);
            }
            painter.line_segment([p(0.5, 0.38), p(0.5, 0.62)], Stroke::new(w * 0.8, color));
            painter.line_segment([p(0.38, 0.5), p(0.62, 0.5)], Stroke::new(w * 0.8, color));
        }
        Icon::Library => {
            for (x, y) in [(0.1, 0.1), (0.54, 0.1), (0.1, 0.54), (0.54, 0.54)] {
                painter.rect_stroke(rr(x, y, x + 0.36, y + 0.36), (s * 0.07) as u8, st, StrokeKind::Middle);
            }
        }
        Icon::Settings => {
            let center = p(0.5, 0.5);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sa, ca) = a.sin_cos();
                painter.line_segment(
                    [center + vec2(ca, sa) * s * 0.30, center + vec2(ca, sa) * s * 0.44],
                    Stroke::new(w * 1.6, color),
                );
            }
            painter.circle_stroke(center, s * 0.28, st);
            painter.circle_stroke(center, s * 0.11, st);
        }
        Icon::Folder | Icon::Open => {
            painter.line(
                vec![p(0.08, 0.82), p(0.08, 0.20), p(0.38, 0.20), p(0.46, 0.30), p(0.92, 0.30), p(0.92, 0.82), p(0.08, 0.82)],
                st,
            );
            if icon == Icon::Open {
                painter.line_segment([p(0.5, 0.72), p(0.5, 0.44)], st);
                painter.line(vec![p(0.38, 0.54), p(0.5, 0.42), p(0.62, 0.54)], st);
            }
        }
        Icon::Copy => {
            painter.rect_stroke(rr(0.30, 0.30, 0.88, 0.88), rad, st, StrokeKind::Middle);
            painter.line(vec![p(0.70, 0.30), p(0.70, 0.12), p(0.12, 0.12), p(0.12, 0.70), p(0.30, 0.70)], st);
        }
        Icon::Save => {
            painter.line_segment([p(0.5, 0.12), p(0.5, 0.64)], st);
            painter.line(vec![p(0.30, 0.46), p(0.5, 0.66), p(0.70, 0.46)], st);
            painter.line(vec![p(0.12, 0.66), p(0.12, 0.88), p(0.88, 0.88), p(0.88, 0.66)], st);
        }
        Icon::Pen => {
            painter.line(vec![p(0.18, 0.82), p(0.24, 0.62), p(0.70, 0.16), p(0.84, 0.30), p(0.38, 0.76), p(0.18, 0.82)], st);
            painter.line_segment([p(0.60, 0.26), p(0.74, 0.40)], st);
        }
        Icon::Highlighter => {
            painter.line(vec![p(0.30, 0.66), p(0.62, 0.14), p(0.86, 0.30), p(0.54, 0.82), p(0.30, 0.66)], st);
            painter.line(vec![p(0.30, 0.66), p(0.20, 0.80), p(0.40, 0.88), p(0.54, 0.82)], st);
        }
        Icon::Rect => {
            painter.rect_stroke(rr(0.14, 0.22, 0.86, 0.78), (s * 0.05) as u8, st, StrokeKind::Middle);
        }
        Icon::Arrow => {
            painter.line_segment([p(0.16, 0.84), p(0.80, 0.20)], st);
            painter.add(Shape::convex_polygon(
                vec![p(0.86, 0.14), p(0.80, 0.46), p(0.54, 0.20)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pixelate => {
            let cells = 3;
            let size = 0.76 / cells as f32;
            for iy in 0..cells {
                for ix in 0..cells {
                    let x = 0.12 + ix as f32 * size;
                    let y = 0.12 + iy as f32 * size;
                    let a = if (ix + iy) % 2 == 0 { 255 } else { 90 };
                    painter.rect_filled(
                        rr(x + 0.02, y + 0.02, x + size - 0.02, y + size - 0.02),
                        1,
                        color.gamma_multiply(a as f32 / 255.0),
                    );
                }
            }
        }
        Icon::Undo | Icon::Redo => {
            let flip = icon == Icon::Redo;
            let fx = |x: f32| if flip { 1.0 - x } else { x };
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.1 - i as f32 / 12.0 * 1.2);
                    let (sa, ca) = a.sin_cos();
                    p(fx(0.54 + ca * 0.30), 0.58 - sa * 0.30)
                })
                .collect();
            painter.line(pts, st);
            painter.line(vec![p(fx(0.10), 0.30), p(fx(0.24), 0.56), p(fx(0.46), 0.42)], st);
        }
        Icon::Trash => {
            painter.line_segment([p(0.14, 0.26), p(0.86, 0.26)], st);
            painter.line(vec![p(0.38, 0.26), p(0.40, 0.14), p(0.60, 0.14), p(0.62, 0.26)], st);
            painter.line(vec![p(0.22, 0.26), p(0.28, 0.88), p(0.72, 0.88), p(0.78, 0.26)], st);
        }
        Icon::Pause => {
            painter.rect_filled(rr(0.24, 0.18, 0.42, 0.82), 2, color);
            painter.rect_filled(rr(0.58, 0.18, 0.76, 0.82), 2, color);
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![p(0.26, 0.16), p(0.84, 0.5), p(0.26, 0.84)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            painter.rect_filled(rr(0.2, 0.2, 0.8, 0.8), (s * 0.08) as u8, color);
        }
        Icon::Timer => {
            painter.circle_stroke(p(0.5, 0.56), s * 0.34, st);
            painter.line_segment([p(0.5, 0.56), p(0.5, 0.36)], st);
            painter.line_segment([p(0.5, 0.56), p(0.64, 0.64)], st);
            painter.line_segment([p(0.40, 0.08), p(0.60, 0.08)], st);
        }
        Icon::Pointer => {
            painter.add(Shape::convex_polygon(
                vec![p(0.24, 0.10), p(0.78, 0.56), p(0.52, 0.58), p(0.24, 0.84)],
                Color32::TRANSPARENT,
                st,
            ));
            painter.line_segment([p(0.52, 0.58), p(0.68, 0.90)], st);
        }
        Icon::Check => {
            painter.line(vec![p(0.16, 0.52), p(0.40, 0.76), p(0.86, 0.26)], Stroke::new(w * 1.3, color));
        }
        Icon::Close => {
            painter.line_segment([p(0.22, 0.22), p(0.78, 0.78)], st);
            painter.line_segment([p(0.78, 0.22), p(0.22, 0.78)], st);
        }
        Icon::Plus => {
            painter.line_segment([p(0.5, 0.16), p(0.5, 0.84)], st);
            painter.line_segment([p(0.16, 0.5), p(0.84, 0.5)], st);
        }
        Icon::Film => {
            painter.rect_stroke(rr(0.12, 0.12, 0.88, 0.88), rad, st, StrokeKind::Middle);
            painter.line_segment([p(0.30, 0.12), p(0.30, 0.88)], st);
            painter.line_segment([p(0.70, 0.12), p(0.70, 0.88)], st);
            painter.line_segment([p(0.12, 0.5), p(0.30, 0.5)], st);
            painter.line_segment([p(0.70, 0.5), p(0.88, 0.5)], st);
        }
        Icon::Image => {
            painter.rect_stroke(rr(0.10, 0.16, 0.90, 0.84), rad, st, StrokeKind::Middle);
            painter.line(vec![p(0.10, 0.74), p(0.36, 0.48), p(0.56, 0.66), p(0.70, 0.54), p(0.90, 0.72)], st);
            painter.circle_stroke(p(0.68, 0.34), s * 0.07, st);
        }
        Icon::Keyboard => {
            painter.rect_stroke(rr(0.06, 0.24, 0.94, 0.78), rad, st, StrokeKind::Middle);
            for row in 0..2 {
                for col in 0..5 {
                    painter.circle_filled(p(0.2 + col as f32 * 0.15, 0.38 + row as f32 * 0.13), s * 0.025, color);
                }
            }
            painter.line_segment([p(0.30, 0.64), p(0.70, 0.64)], st);
        }
    }
}

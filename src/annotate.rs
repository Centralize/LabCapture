//! Markup on screenshots: pen, highlighter, boxes, arrows and blur
//! (pixelate). Marks are kept in picture pixels and painted onto a copy of
//! the screenshot, so what you see is exactly what gets saved.

use image::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Highlighter,
    Rectangle,
    Arrow,
    Pixelate,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Pen, Tool::Highlighter, Tool::Rectangle, Tool::Arrow, Tool::Pixelate];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Pen => "Pen",
            Tool::Highlighter => "Highlighter",
            Tool::Rectangle => "Box",
            Tool::Arrow => "Arrow",
            Tool::Pixelate => "Blur",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Pen => "Draw freely",
            Tool::Highlighter => "Highlight text",
            Tool::Rectangle => "Draw a box around something",
            Tool::Arrow => "Point at something",
            Tool::Pixelate => "Hide private details",
        }
    }
}

pub type Point = [f32; 2];

#[derive(Clone, Debug)]
pub enum Mark {
    Pen { points: Vec<Point>, color: [u8; 4], width: f32 },
    Highlight { points: Vec<Point>, color: [u8; 4], width: f32 },
    Rect { a: Point, b: Point, color: [u8; 4], width: f32 },
    Arrow { a: Point, b: Point, color: [u8; 4], width: f32 },
    Pixelate { a: Point, b: Point },
}

impl Mark {
    /// Marks too small to mean anything (a click without a drag).
    pub fn is_meaningful(&self) -> bool {
        let dist = |a: Point, b: Point| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        match self {
            Mark::Pen { points, .. } | Mark::Highlight { points, .. } => !points.is_empty(),
            Mark::Rect { a, b, .. } | Mark::Pixelate { a, b } => (a[0] - b[0]).abs() > 2.0 && (a[1] - b[1]).abs() > 2.0,
            Mark::Arrow { a, b, .. } => dist(*a, *b) > 4.0,
        }
    }
}

/// Arrow head size for a given line width.
pub fn arrow_head(width: f32) -> (f32, f32) {
    let len = width * 3.2 + 10.0;
    let half_w = width * 1.8 + 6.0;
    (len, half_w)
}

/// Paint all marks onto a copy of the picture.
pub fn render(base: &RgbaImage, marks: &[Mark]) -> RgbaImage {
    let mut img = base.clone();
    for mark in marks {
        apply(&mut img, mark);
    }
    img
}

fn apply(img: &mut RgbaImage, mark: &Mark) {
    match mark {
        Mark::Pen { points, color, width } | Mark::Highlight { points, color, width } => {
            let mut mask = Mask::around(img, points, *width / 2.0 + 2.0);
            if points.len() == 1 {
                mask.segment(points[0], points[0], *width / 2.0);
            }
            for pair in points.windows(2) {
                mask.segment(pair[0], pair[1], *width / 2.0);
            }
            mask.blend(img, *color);
        }
        Mark::Rect { a, b, color, width } => {
            let corners = [*a, [b[0], a[1]], *b, [a[0], b[1]]];
            let mut mask = Mask::around(img, &corners, *width / 2.0 + 2.0);
            for i in 0..4 {
                mask.segment(corners[i], corners[(i + 1) % 4], *width / 2.0);
            }
            mask.blend(img, *color);
        }
        Mark::Arrow { a, b, color, width } => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            let (ux, uy) = (dx / len, dy / len);
            let (head_len, head_half) = arrow_head(*width);
            let head_len = head_len.min(len);
            let base_pt = [b[0] - ux * head_len, b[1] - uy * head_len];
            let shaft_end = [b[0] - ux * head_len * 0.7, b[1] - uy * head_len * 0.7];
            let left = [base_pt[0] - uy * head_half, base_pt[1] + ux * head_half];
            let right = [base_pt[0] + uy * head_half, base_pt[1] - ux * head_half];
            let mut mask = Mask::around(img, &[*a, *b, left, right], *width / 2.0 + 2.0);
            mask.segment(*a, shaft_end, *width / 2.0);
            mask.triangle(*b, left, right);
            mask.blend(img, *color);
        }
        Mark::Pixelate { a, b } => pixelate(img, *a, *b),
    }
}

fn pixelate(img: &mut RgbaImage, a: Point, b: Point) {
    let (w, h) = img.dimensions();
    let x0 = a[0].min(b[0]).max(0.0) as u32;
    let y0 = a[1].min(b[1]).max(0.0) as u32;
    let x1 = (a[0].max(b[0]).ceil() as u32).min(w);
    let y1 = (a[1].max(b[1]).ceil() as u32).min(h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let block = (w.max(h) / 110).max(10);
    let mut by = y0;
    while by < y1 {
        let bh = block.min(y1 - by);
        let mut bx = x0;
        while bx < x1 {
            let bw = block.min(x1 - bx);
            let mut sum = [0u64; 3];
            for y in by..by + bh {
                for x in bx..bx + bw {
                    let p = img.get_pixel(x, y).0;
                    sum[0] += p[0] as u64;
                    sum[1] += p[1] as u64;
                    sum[2] += p[2] as u64;
                }
            }
            let n = (bw * bh) as u64;
            let avg = image::Rgba([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]);
            for y in by..by + bh {
                for x in bx..bx + bw {
                    img.put_pixel(x, y, avg);
                }
            }
            bx += bw;
        }
        by += bh;
    }
}

/// Coverage mask over the part of the picture a mark touches. Building the
/// whole shape first and blending once keeps see-through colours even.
struct Mask {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
    data: Vec<u8>,
}

impl Mask {
    fn around(img: &RgbaImage, pts: &[Point], pad: f32) -> Mask {
        let (iw, ih) = (img.width() as i32, img.height() as i32);
        let mut minx = f32::MAX;
        let mut miny = f32::MAX;
        let mut maxx = f32::MIN;
        let mut maxy = f32::MIN;
        for p in pts {
            minx = minx.min(p[0]);
            miny = miny.min(p[1]);
            maxx = maxx.max(p[0]);
            maxy = maxy.max(p[1]);
        }
        if pts.is_empty() {
            minx = 0.0;
            miny = 0.0;
            maxx = 0.0;
            maxy = 0.0;
        }
        let x0 = ((minx - pad).floor() as i32).clamp(0, iw);
        let y0 = ((miny - pad).floor() as i32).clamp(0, ih);
        let x1 = ((maxx + pad).ceil() as i32 + 1).clamp(0, iw);
        let y1 = ((maxy + pad).ceil() as i32 + 1).clamp(0, ih);
        let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
        Mask { x0, y0, w, h, data: vec![0; (w * h) as usize] }
    }

    fn put(&mut self, x: i32, y: i32, cov: f32) {
        let (lx, ly) = (x - self.x0, y - self.y0);
        if lx < 0 || ly < 0 || lx >= self.w || ly >= self.h {
            return;
        }
        let v = (cov.clamp(0.0, 1.0) * 255.0) as u8;
        let i = (ly * self.w + lx) as usize;
        if v > self.data[i] {
            self.data[i] = v;
        }
    }

    /// A thick line with round ends.
    fn segment(&mut self, a: Point, b: Point, r: f32) {
        let r = r.max(0.5);
        let x0 = (a[0].min(b[0]) - r - 1.0).floor() as i32;
        let x1 = (a[0].max(b[0]) + r + 1.0).ceil() as i32;
        let y0 = (a[1].min(b[1]) - r - 1.0).floor() as i32;
        let y1 = (a[1].max(b[1]) + r + 1.0).ceil() as i32;
        let x0 = x0.max(self.x0);
        let y0 = y0.max(self.y0);
        let x1 = x1.min(self.x0 + self.w);
        let y1 = y1.min(self.y0 + self.h);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = if len2 > 0.0 {
                    (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (cx, cy) = (a[0] + t * dx, a[1] + t * dy);
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let cov = r + 0.5 - d;
                if cov > 0.0 {
                    self.put(x, y, cov);
                }
            }
        }
    }

    fn triangle(&mut self, a: Point, b: Point, c: Point) {
        let pts = [a, b, c];
        // Make the winding consistent.
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let pts = if area < 0.0 { [a, c, b] } else { pts };
        let x0 = pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor() as i32 - 1;
        let x1 = pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil() as i32 + 1;
        let y0 = pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor() as i32 - 1;
        let y1 = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil() as i32 + 1;
        for y in y0.max(self.y0)..y1.min(self.y0 + self.h) {
            for x in x0.max(self.x0)..x1.min(self.x0 + self.w) {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut sd = f32::MAX;
                for i in 0..3 {
                    let (p, q) = (pts[i], pts[(i + 1) % 3]);
                    let (ex, ey) = (q[0] - p[0], q[1] - p[1]);
                    let len = (ex * ex + ey * ey).sqrt().max(0.0001);
                    // Signed distance, positive inside.
                    let d = (ex * (py - p[1]) - ey * (px - p[0])) / len;
                    sd = sd.min(d);
                }
                let cov = sd + 0.5;
                if cov > 0.0 {
                    self.put(x, y, cov);
                }
            }
        }
    }

    fn blend(&self, img: &mut RgbaImage, color: [u8; 4]) {
        for ly in 0..self.h {
            for lx in 0..self.w {
                let m = self.data[(ly * self.w + lx) as usize];
                if m == 0 {
                    continue;
                }
                let a = (m as u32 * color[3] as u32) / 255;
                let p = img.get_pixel_mut((self.x0 + lx) as u32, (self.y0 + ly) as u32);
                for c in 0..3 {
                    p.0[c] = ((color[c] as u32 * a + p.0[c] as u32 * (255 - a)) / 255) as u8;
                }
                p.0[3] = 255;
            }
        }
    }
}

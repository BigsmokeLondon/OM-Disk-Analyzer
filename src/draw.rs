//! Painting helpers: colours, icons, gradient bars, pie slices and the treemap layout.

use egui::{pos2, vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

// OM Industries brand colours (from the logo pack)
pub const NAVY: Color32 = Color32::from_rgb(0x1B, 0x2A, 0x3A);
pub const RUST: Color32 = Color32::from_rgb(0xB5, 0x47, 0x2A);
pub const GOLD: Color32 = Color32::from_rgb(0xDC, 0xA9, 0x3F);
pub const CREAM: Color32 = Color32::from_rgb(0xF2, 0xE8, 0xD0);
pub const BG: Color32 = Color32::from_rgb(0xF0, 0xF0, 0xF0);
pub const PANEL: Color32 = Color32::WHITE;
pub const TEXT: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x1B);
pub const MUTED: Color32 = Color32::from_rgb(0x6B, 0x72, 0x80);
pub const LINE: Color32 = Color32::from_rgb(0xC8, 0xC8, 0xC8);
pub const SELECT: Color32 = Color32::from_rgb(0xCC, 0xE4, 0xF7);
pub const BIG_TEXT: Color32 = Color32::from_rgb(0x10, 0x20, 0x9A);
pub const COMPRESSED: Color32 = Color32::from_rgb(0x00, 0x50, 0xD0);
pub const ERROR: Color32 = Color32::from_rgb(0xB0, 0x00, 0x20);
pub const PCT_BAR: Color32 = Color32::from_rgb(0x96, 0x96, 0xF5);
pub const FREE_BAR: Color32 = Color32::from_rgb(0x22, 0xD0, 0x22);
pub const BAR_BACK: Color32 = Color32::from_rgb(0xE4, 0xE4, 0xE4);
pub const OTHER: Color32 = Color32::from_rgb(0xB9, 0xB6, 0xAB);
pub const FILES: Color32 = Color32::from_rgb(0xE6, 0xD6, 0xB0);
pub const PALETTE: [Color32; 12] = [
    Color32::from_rgb(0x1B, 0x2A, 0x3A),
    Color32::from_rgb(0xB5, 0x47, 0x2A),
    Color32::from_rgb(0xDC, 0xA9, 0x3F),
    Color32::from_rgb(0x3E, 0x6E, 0x8E),
    Color32::from_rgb(0x8A, 0x3F, 0x2E),
    Color32::from_rgb(0x6F, 0x8F, 0x5A),
    Color32::from_rgb(0xC9, 0x8B, 0x2B),
    Color32::from_rgb(0x5B, 0x4A, 0x6E),
    Color32::from_rgb(0x2E, 0x8C, 0x8C),
    Color32::from_rgb(0x9C, 0x6B, 0x3F),
    Color32::from_rgb(0x4F, 0x5D, 0x75),
    Color32::from_rgb(0xD0, 0x7A, 0x55),
];

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn mix(a: Color32, b: Color32, f: f32) -> Color32 {
    let f = f.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn lighten(c: Color32, f: f32) -> Color32 {
    mix(c, Color32::WHITE, f)
}

/// A bar filled with a left-to-right gradient.
pub fn gradient_rect(p: &Painter, r: Rect, left: Color32, right: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), left);
    m.colored_vertex(r.right_top(), right);
    m.colored_vertex(r.right_bottom(), right);
    m.colored_vertex(r.left_bottom(), left);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

pub fn folder_icon(p: &Painter, x: f32, cy: f32, s: f32, warn: bool) {
    let tab = Rect::from_min_max(pos2(x, cy - 6.0 * s), pos2(x + 6.5 * s, cy - 3.0 * s));
    p.rect_filled(tab, 1.0, Color32::from_rgb(0xE8, 0xA3, 0x17));
    let body = Rect::from_min_max(pos2(x, cy - 4.0 * s), pos2(x + 15.0 * s, cy + 6.5 * s));
    p.rect_filled(body, 1.0, Color32::from_rgb(0xFF, 0xC8, 0x3D));
    p.rect_stroke(body, 1.0, Stroke::new(1.0, Color32::from_rgb(0xD6, 0x9A, 0x1C)), StrokeKind::Inside);
    if warn {
        let c = pos2(x + 7.5 * s, cy + 1.2 * s);
        p.rect_filled(Rect::from_center_size(c, vec2(1.8 * s, 5.0 * s)), 0.0, Color32::BLACK);
        p.rect_filled(Rect::from_center_size(pos2(c.x, cy + 4.6 * s), vec2(1.8 * s, 1.5 * s)), 0.0, Color32::BLACK);
    }
}

pub fn file_icon(p: &Painter, x: f32, cy: f32, s: f32) {
    let pts = vec![
        pos2(x + 2.0 * s, cy - 7.5 * s),
        pos2(x + 9.5 * s, cy - 7.5 * s),
        pos2(x + 13.0 * s, cy - 4.0 * s),
        pos2(x + 13.0 * s, cy + 7.5 * s),
        pos2(x + 2.0 * s, cy + 7.5 * s),
    ];
    p.add(Shape::convex_polygon(pts, PANEL, Stroke::new(1.0, Color32::from_rgb(0x77, 0x77, 0x77))));
}

pub fn drive_icon(p: &Painter, x: f32, cy: f32, s: f32) {
    let r = Rect::from_min_max(pos2(x, cy - 4.5 * s), pos2(x + 16.0 * s, cy + 4.5 * s));
    p.rect_filled(r, 1.5, Color32::from_rgb(0xD7, 0xDC, 0xE3));
    p.rect_stroke(r, 1.5, Stroke::new(1.0, Color32::from_rgb(0x59, 0x60, 0x6B)), StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(pos2(x + 11.0 * s, cy - 1.0 * s), vec2(3.0 * s, 2.5 * s)), 0.0, Color32::from_rgb(0x2B, 0xB6, 0x73));
}

pub fn expander(p: &Painter, c: Pos2, open: bool) {
    let r = Rect::from_center_size(c, vec2(9.0, 9.0));
    p.rect_filled(r, 0.0, PANEL);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0x8A, 0x8A, 0x8A)), StrokeKind::Inside);
    let st = Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33));
    p.line_segment([pos2(c.x - 2.5, c.y), pos2(c.x + 2.5, c.y)], st);
    if !open {
        p.line_segment([pos2(c.x, c.y - 2.5), pos2(c.x, c.y + 2.5)], st);
    }
}

/// A percentage bar with its text centred on it (Details, drives, statistics).
pub fn pct_bar(p: &Painter, r: Rect, frac: f32, fill: Color32, text: &str, font: egui::FontId) {
    let r = r.shrink2(vec2(2.0, 2.0));
    p.rect_filled(r, 0.0, BAR_BACK);
    let w = r.width() * frac.clamp(0.0, 1.0);
    if w >= 1.0 {
        p.rect_filled(Rect::from_min_size(r.min, vec2(w, r.height())), 0.0, fill);
    }
    p.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)), StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, text, font, TEXT);
}

/// One pie slice from angle a0 to a1 (radians, clockwise from 12 o'clock), with a hole.
pub fn ring_slice(p: &Painter, c: Pos2, r_out: f32, r_in: f32, a0: f32, a1: f32, fill: Color32) {
    let steps = (((a1 - a0).abs() / std::f32::consts::TAU) * 180.0).ceil().max(2.0) as usize;
    let at = |a: f32, r: f32| pos2(c.x + r * a.sin(), c.y - r * a.cos());
    let mut m = Mesh::default();
    for i in 0..=steps {
        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
        m.colored_vertex(at(a, r_out), fill);
        m.colored_vertex(at(a, r_in), fill);
    }
    for i in 0..steps as u32 {
        let k = i * 2;
        m.add_triangle(k, k + 1, k + 2);
        m.add_triangle(k + 1, k + 3, k + 2);
    }
    p.add(Shape::mesh(m));
    if (a1 - a0).abs() < std::f32::consts::TAU - 0.001 {
        p.line_segment([at(a0, r_in), at(a0, r_out)], Stroke::new(1.5, PANEL));
    }
}

/// Squarified treemap: rectangles for `values` (largest first) inside `r`.
pub fn squarify(values: &[f64], r: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    let mut out = vec![Rect::NOTHING; values.len()];
    if total <= 0.0 || r.width() < 1.0 || r.height() < 1.0 {
        return out;
    }
    let (mut x, mut y, mut w, mut h) = (r.min.x as f64, r.min.y as f64, r.width() as f64, r.height() as f64);
    let scale = w * h / total;
    let areas: Vec<f64> = values.iter().map(|v| v * scale).collect();
    let worst = |row: &[f64], side: f64| {
        let s: f64 = row.iter().sum();
        let mx = row.iter().cloned().fold(0.0, f64::max);
        let mn = row.iter().cloned().fold(f64::INFINITY, f64::min);
        (side * side * mx / (s * s)).max((s * s) / (side * side * mn))
    };
    let mut i = 0;
    while i < areas.len() {
        if areas[i] <= 0.0 {
            i += 1;
            continue;
        }
        let side = w.min(h);
        if side <= 0.0 {
            break;
        }
        let mut j = i + 1;
        while j < areas.len() && areas[j] > 0.0 && worst(&areas[i..=j], side) <= worst(&areas[i..j], side) {
            j += 1;
        }
        let s: f64 = areas[i..j].iter().sum();
        if w >= h {
            let cw = s / h;
            let mut yy = y;
            for k in i..j {
                let hh = areas[k] / cw;
                out[k] = Rect::from_min_size(pos2(x as f32, yy as f32), vec2(cw as f32, hh as f32));
                yy += hh;
            }
            x += cw;
            w -= cw;
        } else {
            let rh = s / w;
            let mut xx = x;
            for k in i..j {
                let ww = areas[k] / rh;
                out[k] = Rect::from_min_size(pos2(xx as f32, y as f32), vec2(ww as f32, rh as f32));
                xx += ww;
            }
            y += rh;
            h -= rh;
        }
        i = j;
    }
    out
}

//! Shape generators matching InDesign's anchor layouts (so later edits behave the same).

use std::f64::consts::{PI, TAU};

use kurbo::{Point, Rect, Vec2};

use crate::path::{Anchor, AnchorKind, PathData, SubPath};

/// Circle-approximation handle ratio for a quarter arc.
pub const KAPPA: f64 = 0.552_284_749_830_793_6;

/// Rectangle with 4 corner anchors, clockwise from the top-left.
pub fn rectangle(r: Rect) -> PathData {
    let r = r.abs();
    PathData::single(SubPath::polyline(&[Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)], true))
}

/// Rounded rectangle (8 anchors). The radius is clamped to half the shorter side.
pub fn rounded_rectangle(r: Rect, radius: f64) -> PathData {
    let r = r.abs();
    let rad = radius.max(0.0).min(r.width() / 2.0).min(r.height() / 2.0);
    if rad <= 1e-9 {
        return rectangle(r);
    }
    let k = rad * KAPPA;
    let (x0, y0, x1, y1) = (r.x0, r.y0, r.x1, r.y1);
    let a = |p: (f64, f64), hin: (f64, f64), hout: (f64, f64)| Anchor { p: p.into(), h_in: hin.into(), h_out: hout.into(), kind: AnchorKind::Corner };
    let anchors = vec![
        a((x0 + rad, y0), (x0 + rad - k, y0), (x0 + rad, y0)),
        a((x1 - rad, y0), (x1 - rad, y0), (x1 - rad + k, y0)),
        a((x1, y0 + rad), (x1, y0 + rad - k), (x1, y0 + rad)),
        a((x1, y1 - rad), (x1, y1 - rad), (x1, y1 - rad + k)),
        a((x1 - rad, y1), (x1 - rad + k, y1), (x1 - rad, y1)),
        a((x0 + rad, y1), (x0 + rad, y1), (x0 + rad - k, y1)),
        a((x0, y1 - rad), (x0, y1 - rad + k), (x0, y1 - rad)),
        a((x0, y0 + rad), (x0, y0 + rad), (x0, y0 + rad - k)),
    ];
    PathData::single(SubPath::new(anchors, true))
}

/// Ellipse inscribed in `r` with 4 smooth anchors (left, top, right, bottom).
pub fn ellipse(r: Rect) -> PathData {
    let r = r.abs();
    let c = r.center();
    let rx = r.width() / 2.0;
    let ry = r.height() / 2.0;
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    let s = |p: Point, hin: Vec2| Anchor { p, h_in: p + hin, h_out: p - hin, kind: AnchorKind::Smooth };
    let anchors = vec![
        s(Point::new(c.x - rx, c.y), Vec2::new(0.0, ky)),
        s(Point::new(c.x, c.y - ry), Vec2::new(-kx, 0.0)),
        s(Point::new(c.x + rx, c.y), Vec2::new(0.0, -ky)),
        s(Point::new(c.x, c.y + ry), Vec2::new(kx, 0.0)),
    ];
    PathData::single(SubPath::new(anchors, true))
}

/// Regular polygon with `sides` (≥ 3), first vertex straight up from the centre.
pub fn polygon(center: Point, radius: f64, sides: u32, rotation_deg: f64) -> PathData {
    let n = sides.max(3);
    let rot = rotation_deg.to_radians();
    let pts: Vec<Point> = (0..n)
        .map(|i| {
            let a = -PI / 2.0 + rot + TAU * i as f64 / n as f64;
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();
    PathData::single(SubPath::polyline(&pts, true))
}

/// Star with `points` tips (≥ 2), outer radius `r1`, inner radius `r2`, first tip up.
pub fn star(center: Point, r1: f64, r2: f64, points: u32, rotation_deg: f64) -> PathData {
    let n = points.max(2) * 2;
    let rot = rotation_deg.to_radians();
    let pts: Vec<Point> = (0..n)
        .map(|i| {
            let r = if i % 2 == 0 { r1 } else { r2 };
            let a = -PI / 2.0 + rot + TAU * i as f64 / n as f64;
            Point::new(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect();
    PathData::single(SubPath::polyline(&pts, true))
}

/// The names [`preset`] draws.
pub const PRESETS: &[&str] = &[
    "roundedRect",
    "snippedRect",
    "pill",
    "triangle",
    "rightTriangle",
    "parallelogram",
    "trapezoid",
    "diamond",
    "pentagon",
    "hexagon",
    "octagon",
    "cross",
    "heart",
    "lightning",
    "sun",
    "moon",
    "cloud",
    "arrowRight",
    "arrowLeft",
    "arrowUp",
    "arrowDown",
    "arrowLeftRight",
    "arrowUpDown",
    "chevron",
    "homePlate",
    "notchedArrow",
    "star4",
    "star5",
    "star6",
    "star8",
    "star12",
    "star16",
    "star24",
    "ribbon",
    "wave",
    "speech",
    "roundSpeech",
    "ovalSpeech",
    "flowDocument",
];

/// A closed polygon through points given in the unit square, mapped onto `r`.
fn unit_poly(r: Rect, pts: &[(f64, f64)]) -> PathData {
    let at = |(x, y): (f64, f64)| Point::new(r.x0 + x * r.width(), r.y0 + y * r.height());
    let pts: Vec<Point> = pts.iter().copied().map(at).collect();
    PathData::single(SubPath::polyline(&pts, true))
}

/// A closed curve through anchors given in the unit square: (point, handle in, handle out).
fn unit_curve(r: Rect, anchors: &[((f64, f64), (f64, f64), (f64, f64))]) -> PathData {
    let at = |(x, y): (f64, f64)| Point::new(r.x0 + x * r.width(), r.y0 + y * r.height());
    let anchors = anchors.iter().map(|&(p, i, o)| Anchor { p: at(p), h_in: at(i), h_out: at(o), kind: AnchorKind::Corner }).collect();
    PathData::single(SubPath::new(anchors, true))
}

/// A star with `points` tips filling `r`, its inner radius `inner` of the outer.
fn star_in(r: Rect, points: u32, inner: f64) -> PathData {
    let mut p = star(Point::ZERO, 1.0, inner, points, 0.0);
    if let Some(b) = p.bounds() {
        let a = kurbo::Affine::translate(r.center().to_vec2())
            * kurbo::Affine::scale_non_uniform(r.width() / b.width().max(1e-9), r.height() / b.height().max(1e-9))
            * kurbo::Affine::translate(-b.center().to_vec2());
        p.transform(a);
    }
    p
}

/// A scalloped ring of bumps, the cloud's outline, in the unit square.
fn cloud_anchors() -> Vec<((f64, f64), (f64, f64), (f64, f64))> {
    const BUMPS: usize = 9;
    let dir = |t: f64| (t.cos(), t.sin());
    (0..BUMPS)
        .map(|i| {
            let t = TAU * i as f64 / BUMPS as f64;
            let step = TAU / BUMPS as f64;
            let (cx, cy) = dir(t);
            let (bx, by) = dir(t - step * 0.45);
            let (ax, ay) = dir(t + step * 0.45);
            let on = |x: f64, y: f64, k: f64| (0.5 + 0.5 * k * x, 0.5 + 0.5 * k * y);
            (on(cx, cy, 0.72), on(bx, by, 1.02), on(ax, ay, 1.02))
        })
        .collect()
}

/// A named shape filling `r`: PressRoom's own drawings for the Shapes gallery. None for an unknown
/// name.
pub fn preset(name: &str, r: Rect) -> Option<PathData> {
    let r = r.abs();
    let short = r.width().min(r.height());
    Some(match name {
        "roundedRect" => rounded_rectangle(r, short / 6.0),
        "pill" => rounded_rectangle(r, short / 2.0),
        "snippedRect" => unit_poly(r, &[(0.0, 0.0), (0.82, 0.0), (1.0, 0.18), (1.0, 1.0), (0.0, 1.0)]),
        "triangle" => unit_poly(r, &[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "rightTriangle" => unit_poly(r, &[(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "parallelogram" => unit_poly(r, &[(0.25, 0.0), (1.0, 0.0), (0.75, 1.0), (0.0, 1.0)]),
        "trapezoid" => unit_poly(r, &[(0.2, 0.0), (0.8, 0.0), (1.0, 1.0), (0.0, 1.0)]),
        "diamond" => unit_poly(r, &[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)]),
        "pentagon" => unit_poly(r, &[(0.5, 0.0), (1.0, 0.38), (0.81, 1.0), (0.19, 1.0), (0.0, 0.38)]),
        "hexagon" => unit_poly(r, &[(0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5)]),
        "octagon" => unit_poly(r, &[(0.29, 0.0), (0.71, 0.0), (1.0, 0.29), (1.0, 0.71), (0.71, 1.0), (0.29, 1.0), (0.0, 0.71), (0.0, 0.29)]),
        "cross" => unit_poly(
            r,
            &[
                (0.33, 0.0),
                (0.67, 0.0),
                (0.67, 0.33),
                (1.0, 0.33),
                (1.0, 0.67),
                (0.67, 0.67),
                (0.67, 1.0),
                (0.33, 1.0),
                (0.33, 0.67),
                (0.0, 0.67),
                (0.0, 0.33),
                (0.33, 0.33),
            ],
        ),
        "heart" => unit_curve(
            r,
            &[
                ((0.5, 0.22), (0.45, 0.08), (0.55, 0.08)),
                ((0.75, 0.0), (0.62, 0.0), (0.9, 0.0)),
                ((1.0, 0.3), (1.0, 0.12), (1.0, 0.55)),
                ((0.5, 1.0), (0.75, 0.75), (0.25, 0.75)),
                ((0.0, 0.3), (0.0, 0.55), (0.0, 0.12)),
                ((0.25, 0.0), (0.1, 0.0), (0.38, 0.0)),
            ],
        ),
        "lightning" => unit_poly(r, &[(0.55, 0.0), (0.2, 0.5), (0.45, 0.5), (0.3, 1.0), (0.85, 0.38), (0.55, 0.38), (0.75, 0.0)]),
        "sun" => star_in(r, 12, 0.72),
        "moon" => unit_curve(
            r,
            &[
                ((0.8, 0.0), (0.55, 0.15), (0.36, 0.0)),
                ((0.0, 0.5), (0.0, 0.22), (0.0, 0.78)),
                ((0.8, 1.0), (0.36, 1.0), (0.55, 0.85)),
                ((0.4, 0.5), (0.4, 0.7), (0.4, 0.3)),
            ],
        ),
        "cloud" => unit_curve(r, &cloud_anchors()),
        "arrowRight" => unit_poly(r, &[(0.0, 0.25), (0.6, 0.25), (0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.75), (0.0, 0.75)]),
        "arrowLeft" => unit_poly(r, &[(1.0, 0.25), (0.4, 0.25), (0.4, 0.0), (0.0, 0.5), (0.4, 1.0), (0.4, 0.75), (1.0, 0.75)]),
        "arrowUp" => unit_poly(r, &[(0.25, 1.0), (0.25, 0.4), (0.0, 0.4), (0.5, 0.0), (1.0, 0.4), (0.75, 0.4), (0.75, 1.0)]),
        "arrowDown" => unit_poly(r, &[(0.25, 0.0), (0.25, 0.6), (0.0, 0.6), (0.5, 1.0), (1.0, 0.6), (0.75, 0.6), (0.75, 0.0)]),
        "arrowLeftRight" => unit_poly(
            r,
            &[(0.0, 0.5), (0.25, 0.0), (0.25, 0.25), (0.75, 0.25), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.75, 0.75), (0.25, 0.75), (0.25, 1.0)],
        ),
        "arrowUpDown" => unit_poly(
            r,
            &[(0.5, 0.0), (1.0, 0.25), (0.75, 0.25), (0.75, 0.75), (1.0, 0.75), (0.5, 1.0), (0.0, 0.75), (0.25, 0.75), (0.25, 0.25), (0.0, 0.25)],
        ),
        "chevron" => unit_poly(r, &[(0.0, 0.0), (0.7, 0.0), (1.0, 0.5), (0.7, 1.0), (0.0, 1.0), (0.3, 0.5)]),
        "homePlate" => unit_poly(r, &[(0.0, 0.0), (0.7, 0.0), (1.0, 0.5), (0.7, 1.0), (0.0, 1.0)]),
        "notchedArrow" => unit_poly(r, &[(0.0, 0.25), (0.6, 0.25), (0.6, 0.0), (1.0, 0.5), (0.6, 1.0), (0.6, 0.75), (0.0, 0.75), (0.15, 0.5)]),
        "star4" => star_in(r, 4, 0.35),
        "star5" => star_in(r, 5, 0.382),
        "star6" => star_in(r, 6, 0.55),
        "star8" => star_in(r, 8, 0.6),
        "star12" => star_in(r, 12, 0.7),
        "star16" => star_in(r, 16, 0.75),
        "star24" => star_in(r, 24, 0.8),
        "ribbon" => unit_poly(
            r,
            &[
                (0.15, 0.0),
                (0.85, 0.0),
                (0.85, 0.2),
                (1.0, 0.2),
                (0.9, 0.5),
                (1.0, 0.8),
                (0.7, 0.8),
                (0.7, 0.7),
                (0.3, 0.7),
                (0.3, 0.8),
                (0.0, 0.8),
                (0.1, 0.5),
                (0.0, 0.2),
                (0.15, 0.2),
            ],
        ),
        "wave" => unit_curve(
            r,
            &[
                ((0.0, 0.2), (0.0, 0.2), (0.3, -0.1)),
                ((1.0, 0.2), (0.7, 0.5), (1.0, 0.2)),
                ((1.0, 0.8), (1.0, 0.8), (0.7, 1.1)),
                ((0.0, 0.8), (0.3, 0.5), (0.0, 0.8)),
            ],
        ),
        "speech" => unit_poly(r, &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.75), (0.45, 0.75), (0.2, 1.0), (0.25, 0.75), (0.0, 0.75)]),
        "roundSpeech" => unit_curve(
            r,
            &[
                ((0.1, 0.0), (0.04, 0.0), (0.1, 0.0)),
                ((0.9, 0.0), (0.9, 0.0), (0.96, 0.0)),
                ((1.0, 0.1), (1.0, 0.04), (1.0, 0.1)),
                ((1.0, 0.65), (1.0, 0.65), (1.0, 0.71)),
                ((0.9, 0.75), (0.96, 0.75), (0.9, 0.75)),
                ((0.45, 0.75), (0.45, 0.75), (0.45, 0.75)),
                ((0.2, 1.0), (0.2, 1.0), (0.2, 1.0)),
                ((0.25, 0.75), (0.25, 0.75), (0.25, 0.75)),
                ((0.1, 0.75), (0.1, 0.75), (0.04, 0.75)),
                ((0.0, 0.65), (0.0, 0.71), (0.0, 0.65)),
                ((0.0, 0.1), (0.0, 0.1), (0.0, 0.04)),
            ],
        ),
        "ovalSpeech" => unit_curve(
            r,
            &[
                ((0.5, 0.0), (0.22, 0.0), (0.78, 0.0)),
                ((1.0, 0.4), (1.0, 0.18), (1.0, 0.62)),
                ((0.5, 0.8), (0.78, 0.8), (0.5, 0.8)),
                ((0.15, 1.0), (0.15, 1.0), (0.15, 1.0)),
                ((0.27, 0.74), (0.27, 0.74), (0.1, 0.68)),
                ((0.0, 0.4), (0.0, 0.58), (0.0, 0.18)),
            ],
        ),
        "flowDocument" => unit_curve(
            r,
            &[
                ((0.0, 0.0), (0.0, 0.0), (0.0, 0.0)),
                ((1.0, 0.0), (1.0, 0.0), (1.0, 0.0)),
                ((1.0, 0.82), (1.0, 0.82), (0.72, 0.7)),
                ((0.0, 0.9), (0.3, 1.1), (0.0, 0.9)),
            ],
        ),
        _ => return None,
    })
}

/// Inner radius of a "straight-armed" star (InDesign's Alt-drag behaviour): shoulders aligned.
pub fn straight_star_inner_radius(r1: f64, points: u32) -> f64 {
    let n = points.max(3) as f64;
    r1 * (PI / n * 2.0).cos() / (PI / n).cos()
}

/// Open straight line.
pub fn line(a: Point, b: Point) -> PathData {
    PathData::single(SubPath::polyline(&[a, b], false))
}

/// Quarter-ellipse arc from `a` to `b` (Arc tool default: concave, open, x-axis base).
pub fn arc(a: Point, b: Point, slope: f64, closed: bool) -> PathData {
    // Corner of the arc's bounding box is at (a.x, b.y): the arc bows away from it.
    let k = KAPPA * (1.0 + slope.clamp(-1.0, 1.0) * 0.0);
    let corner = Point::new(b.x, a.y);
    let h1 = a + (corner - a) * k;
    let h2 = b + (corner - b) * k;
    let mut anchors =
        vec![Anchor { p: a, h_in: a, h_out: h1, kind: AnchorKind::Corner }, Anchor { p: b, h_in: h2, h_out: b, kind: AnchorKind::Corner }];
    if closed {
        let o = Point::new(a.x, b.y);
        anchors.push(Anchor::corner(o));
    }
    PathData::single(SubPath::new(anchors, closed))
}

/// Archimedean-style spiral approximating InDesign's Spiral tool (decay-based, `segments` quarter turns).
pub fn spiral(center: Point, radius: f64, decay_percent: f64, segments: u32, clockwise: bool) -> PathData {
    let decay = (decay_percent / 100.0).clamp(0.05, 0.9999);
    let dir = if clockwise { 1.0 } else { -1.0 };
    let mut anchors = Vec::new();
    let mut r = radius;
    for i in 0..=segments {
        let a = dir * (i as f64) * PI / 2.0;
        let p = Point::new(center.x + r * a.cos(), center.y + r * a.sin());
        // Tangent direction for a quarter arc of radius r.
        let t = Vec2::new(-a.sin(), a.cos()) * dir;
        let r_prev = r / decay;
        let r_next = r * decay;
        let h_in = p - t * (KAPPA * (r + r_prev) / 2.0);
        let h_out = p + t * (KAPPA * (r + r_next) / 2.0);
        anchors.push(Anchor { p, h_in: if i == 0 { p } else { h_in }, h_out: if i == segments { p } else { h_out }, kind: AnchorKind::Smooth });
        r *= decay;
    }
    // InDesign spirals wind inward from the outer point; reverse so the path starts at the centre end.
    let mut sp = SubPath::new(anchors, false);
    sp.reverse();
    PathData::single(sp)
}

/// Rectangular grid as separate open lines (plus an optional frame rectangle).
pub fn rectangular_grid(r: Rect, h_dividers: u32, v_dividers: u32, frame: bool) -> Vec<PathData> {
    let r = r.abs();
    let mut out = Vec::new();
    for i in 1..=h_dividers {
        let y = r.y0 + r.height() * i as f64 / (h_dividers + 1) as f64;
        out.push(line(Point::new(r.x0, y), Point::new(r.x1, y)));
    }
    for i in 1..=v_dividers {
        let x = r.x0 + r.width() * i as f64 / (v_dividers + 1) as f64;
        out.push(line(Point::new(x, r.y0), Point::new(x, r.y1)));
    }
    if frame {
        out.push(rectangle(r));
    }
    out
}

/// Polar grid: concentric ellipses plus radial dividers.
pub fn polar_grid(r: Rect, concentric: u32, radial: u32) -> Vec<PathData> {
    let r = r.abs();
    let c = r.center();
    let mut out = Vec::new();
    for i in 1..=concentric + 1 {
        let f = i as f64 / (concentric + 1) as f64;
        out.push(ellipse(Rect::from_center_size(c, (r.width() * f, r.height() * f))));
    }
    for i in 0..radial.max(1) {
        let a = -PI / 2.0 + TAU * i as f64 / radial.max(1) as f64;
        out.push(line(c, Point::new(c.x + r.width() / 2.0 * a.cos(), c.y + r.height() / 2.0 * a.sin())));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_4_corners_clockwise() {
        let p = rectangle(Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(p.anchor_count(), 4);
        assert!(p.subpaths[0].area() > 0.0);
        assert_eq!(p.bounds(), Some(Rect::new(0.0, 0.0, 100.0, 50.0)));
    }

    #[test]
    fn rect_normalizes_negative() {
        let p = rectangle(Rect::new(10.0, 10.0, 0.0, 0.0));
        assert_eq!(p.bounds(), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn ellipse_bounds_and_area() {
        let p = ellipse(Rect::new(0.0, 0.0, 100.0, 60.0));
        let b = p.bounds().unwrap();
        assert!((b.width() - 100.0).abs() < 1e-6 && (b.height() - 60.0).abs() < 1e-6);
        let area = p.subpaths[0].area().abs();
        assert!((area - PI * 50.0 * 30.0).abs() / area < 0.001);
    }

    #[test]
    fn rounded_rect_clamps() {
        let p = rounded_rectangle(Rect::new(0.0, 0.0, 20.0, 10.0), 100.0);
        assert_eq!(p.anchor_count(), 8);
        assert_eq!(p.bounds(), Some(Rect::new(0.0, 0.0, 20.0, 10.0)));
        assert_eq!(rounded_rectangle(Rect::new(0.0, 0.0, 20.0, 10.0), 0.0).anchor_count(), 4);
    }

    #[test]
    fn polygon_first_vertex_up() {
        let p = polygon(Point::new(0.0, 0.0), 10.0, 6, 0.0);
        assert_eq!(p.anchor_count(), 6);
        let a = p.subpaths[0].anchors[0].p;
        assert!(a.x.abs() < 1e-9 && (a.y + 10.0).abs() < 1e-9);
    }

    #[test]
    fn star_alternates_radii() {
        let p = star(Point::ZERO, 10.0, 5.0, 5, 0.0);
        assert_eq!(p.anchor_count(), 10);
        let d: Vec<f64> = p.subpaths[0].anchors.iter().map(|a| a.p.to_vec2().hypot()).collect();
        assert!((d[0] - 10.0).abs() < 1e-9 && (d[1] - 5.0).abs() < 1e-9);
    }

    #[test]
    fn straight_star_five_points() {
        let r2 = straight_star_inner_radius(100.0, 5);
        assert!((r2 - 38.196_601).abs() < 1e-3);
    }

    #[test]
    fn spiral_has_segments() {
        let p = spiral(Point::ZERO, 100.0, 80.0, 10, true);
        assert_eq!(p.anchor_count(), 11);
        assert!(!p.subpaths[0].closed);
    }

    #[test]
    fn grids() {
        assert_eq!(rectangular_grid(Rect::new(0.0, 0.0, 10.0, 10.0), 5, 5, true).len(), 11);
        assert_eq!(polar_grid(Rect::new(0.0, 0.0, 10.0, 10.0), 5, 5).len(), 11);
    }

    #[test]
    fn arc_open_and_closed() {
        assert_eq!(arc(Point::ZERO, Point::new(10.0, 10.0), 0.0, false).anchor_count(), 2);
        assert!(arc(Point::ZERO, Point::new(10.0, 10.0), 0.0, true).subpaths[0].closed);
    }
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn every_preset_fills_its_box() {
        let r = Rect::new(10.0, 20.0, 110.0, 80.0);
        for name in PRESETS {
            let p = preset(name, r).unwrap_or_else(|| panic!("{name}"));
            let b = p.bounds().unwrap_or_else(|| panic!("{name}: empty"));
            assert!(b.x0 >= 9.0 && b.y0 >= 15.0 && b.x1 <= 111.0 && b.y1 <= 90.0, "{name}: {b:?}");
            assert!(b.width() > 50.0 && b.height() > 25.0, "{name}: {b:?}");
        }
        assert!(preset("blob", r).is_none());
    }

    #[test]
    fn a_tiny_or_backwards_box_doesnt_panic() {
        for name in PRESETS {
            assert!(preset(name, Rect::new(5.0, 5.0, 5.0, 5.0)).is_some(), "{name}");
            assert!(preset(name, Rect::new(50.0, 60.0, 10.0, 20.0)).is_some(), "{name}");
        }
    }
}

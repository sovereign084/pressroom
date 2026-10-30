//! PressRoom's ribbon icons, drawn in code on a 32-unit grid (large buttons) or the same grid
//! scaled into 16 px (small buttons). Original art in today's Office manner: thin grey outlines
//! with soft rounded corners, a colour shown as a light tint inside an outline of that colour,
//! and solid colour kept for small details.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

pub const INK: Color32 = Color32::from_rgb(72, 72, 72);
pub const BLUE: Color32 = Color32::from_rgb(28, 112, 207);
pub const TEAL: Color32 = Color32::from_rgb(7, 117, 104);
pub const GOLD: Color32 = Color32::from_rgb(224, 152, 30);
pub const RED: Color32 = Color32::from_rgb(214, 58, 62);
pub const GREEN: Color32 = Color32::from_rgb(52, 150, 78);
pub const PAPER: Color32 = Color32::WHITE;
pub const SKY: Color32 = Color32::from_rgb(160, 205, 240);
pub const GREY: Color32 = Color32::from_rgb(140, 140, 140);
/// Ink and paper on the dark themes.
const DARK_INK: Color32 = Color32::from_rgb(222, 222, 222);
const DARK_PAPER: Color32 = Color32::from_rgb(58, 58, 58);
/// Corner rounding of boxes, in grid units.
const ROUND: f32 = 1.8;

/// A pen on a 32 × 32 grid laid over `r`.
struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    /// Grid units → pixels.
    k: f32,
    /// Fade everything for a disabled button.
    dim: bool,
    /// Drawn on a dark theme: ink and paper swap to light outlines on dark fills.
    dark: bool,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.min.x + x * self.k, self.r.min.y + y * self.k)
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }
    fn c(&self, c: Color32) -> Color32 {
        let c = if !self.dark {
            c
        } else if c == INK {
            DARK_INK
        } else if c == PAPER {
            DARK_PAPER
        } else {
            c
        };
        self.faded(c)
    }
    /// Greyed out about as far as Publisher greys a disabled button.
    fn faded(&self, c: Color32) -> Color32 {
        if self.dim { c.gamma_multiply(0.42) } else { c }
    }
    /// A colour as the light tint that fills a shape outlined in it.
    fn tint(&self, c: Color32) -> Color32 {
        let (paper, t) = if self.dark { (DARK_PAPER, 0.45) } else { (PAPER, 0.24) };
        self.faded(paper.lerp_to_gamma(c, t))
    }
    /// The outline drawn round a tint: the colour itself, a touch deeper on light paper and a
    /// touch lighter on dark.
    fn edge(&self, c: Color32) -> Color32 {
        let e = if self.dark { c.lerp_to_gamma(Color32::WHITE, 0.25) } else { c.lerp_to_gamma(Color32::BLACK, 0.12) };
        self.faded(e)
    }
    /// Fill and outline for a shape: a coloured fill outlined in ink becomes Office's tint inside
    /// an outline of the colour; paper, see-through and same-colour fills are drawn as given.
    fn paint(&self, fill: Color32, line: Color32) -> (Color32, Color32) {
        if fill == PAPER || fill == line || fill.a() == 0 || line != INK { (self.c(fill), self.c(line)) } else { (self.tint(fill), self.edge(fill)) }
    }
    fn w(&self, w: f32) -> f32 {
        (w * self.k).max(1.0)
    }
    fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        let r = self.rect(x0, y0, x1, y1);
        let round = (ROUND * 0.5 * self.k).min(r.height() / 2.0).min(r.width() / 2.0);
        self.p.rect_filled(r, round, self.c(c));
    }
    fn boxed(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32, line: Color32) {
        let r = self.rect(x0, y0, x1, y1);
        let (fill, line) = self.paint(fill, line);
        self.p.rect(r, ROUND * self.k, fill, Stroke::new(self.w(1.0), line), StrokeKind::Inside);
    }
    fn outline(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32, w: f32) {
        self.p.rect_stroke(self.rect(x0, y0, x1, y1), ROUND * self.k, Stroke::new(self.w(w), self.c(c)), StrokeKind::Inside);
    }
    /// Round ends on a stroke heavy enough for its square ends to show.
    fn caps(&self, ends: [(f32, f32); 2], c: Color32, w: f32) {
        let px = self.w(w);
        if px >= 1.6 && !self.dim {
            for (x, y) in ends {
                self.p.circle_filled(self.at(x, y), px / 2.0, self.c(c));
            }
        }
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), c: Color32, w: f32) {
        self.p.line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], Stroke::new(self.w(w), self.c(c)));
        self.caps([a, b], c, w);
    }
    /// A convex polygon with its corners softly rounded.
    fn poly(&self, pts: &[(f32, f32)], fill: Color32, line: Option<Color32>) {
        let v = self.rounded(pts, 0.9);
        let (fill, stroke) = match line {
            Some(l) => {
                let (f, l) = self.paint(fill, l);
                (f, Stroke::new(self.w(1.0), l))
            }
            None => (self.c(fill), Stroke::NONE),
        };
        self.p.add(Shape::convex_polygon(v, fill, stroke));
    }
    /// `pts` with each corner replaced by a short curve of radius about `rad` grid units (less
    /// where the sides are short).
    fn rounded(&self, pts: &[(f32, f32)], rad: f32) -> Vec<Pos2> {
        let n = pts.len();
        let mut out = Vec::with_capacity(n * 4);
        for i in 0..n {
            let (Some(&prev), Some(&cur), Some(&next)) = (pts.get((i + n - 1) % n), pts.get(i), pts.get((i + 1) % n)) else {
                continue;
            };
            let (a, b, c) = (vec2(prev.0, prev.1), vec2(cur.0, cur.1), vec2(next.0, next.1));
            let (la, lc) = ((a - b).length(), (c - b).length());
            if la < 1e-3 || lc < 1e-3 {
                out.push(self.at(cur.0, cur.1));
                continue;
            }
            let d = rad.min(la * 0.4).min(lc * 0.4);
            let p0 = b + (a - b) * (d / la);
            let p2 = b + (c - b) * (d / lc);
            // A quadratic curve from p0 to p2 bent towards the corner.
            for s in 0..=3 {
                let t = s as f32 / 3.0;
                let q = p0 * (1.0 - t) * (1.0 - t) + b * 2.0 * t * (1.0 - t) + p2 * t * t;
                out.push(self.at(q.x, q.y));
            }
        }
        out
    }
    fn path(&self, pts: &[(f32, f32)], c: Color32, w: f32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::line(v, Stroke::new(self.w(w), self.c(c))));
        if let (Some(&a), Some(&b)) = (pts.first(), pts.last()) {
            self.caps([a, b], c, w);
        }
    }
    /// An arc round (x, y) from angle `a0` to `a1` (degrees, 0 = east, clockwise on screen).
    fn arc(&self, x: f32, y: f32, rad: f32, a0: f32, a1: f32, c: Color32, w: f32) {
        let steps = 18;
        let pts: Vec<(f32, f32)> = (0..=steps)
            .map(|s| {
                let a = (a0 + (a1 - a0) * s as f32 / steps as f32).to_radians();
                (x + rad * a.cos(), y + rad * a.sin())
            })
            .collect();
        self.path(&pts, c, w);
    }
    fn circle(&self, x: f32, y: f32, rad: f32, fill: Color32, line: Option<Color32>) {
        let (fill, stroke) = match line {
            Some(l) => {
                let (f, l) = self.paint(fill, l);
                (f, Stroke::new(self.w(1.0), l))
            }
            None => (self.c(fill), Stroke::NONE),
        };
        self.p.circle(self.at(x, y), rad * self.k, fill, stroke);
    }
    /// A light tint of `c` laid over part of a shape (a table's header row).
    fn wash(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        let r = self.rect(x0, y0, x1, y1);
        let round = (ROUND * 0.5 * self.k).min(r.height() / 2.0);
        self.p.rect_filled(r, round, self.tint(c));
    }
    fn ring(&self, x: f32, y: f32, rad: f32, c: Color32, w: f32) {
        self.p.circle_stroke(self.at(x, y), rad * self.k, Stroke::new(self.w(w), self.c(c)));
    }
    fn text(&self, x: f32, y: f32, s: &str, size: f32, c: Color32, bold: bool) {
        let font = if bold { crate::theme::semibold(size * self.k) } else { egui::FontId::proportional(size * self.k) };
        self.p.text(self.at(x, y), egui::Align2::CENTER_CENTER, s, font, self.c(c));
    }
    /// A sheet of paper with a folded corner.
    fn page(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let f = 5.0;
        self.poly(&[(x0, y0), (x1 - f, y0), (x1, y0 + f), (x1, y1), (x0, y1)], PAPER, Some(INK));
        let fold = [self.at(x1 - f, y0), self.at(x1 - f + 0.6, y0 + f - 0.6), self.at(x1, y0 + f)];
        self.p.add(Shape::convex_polygon(fold.to_vec(), self.tint(GREY), Stroke::NONE));
        self.path(&[(x1 - f, y0 + 0.4), (x1 - f, y0 + f - 0.8), (x1 - f + 0.8, y0 + f), (x1 - 0.4, y0 + f)], INK, 1.0);
    }
    /// Text lines across a span.
    fn lines(&self, x0: f32, x1: f32, y0: f32, n: usize, gap: f32, c: Color32) {
        for i in 0..n {
            let y = y0 + i as f32 * gap;
            let end = if i + 1 == n { x0 + (x1 - x0) * 0.6 } else { x1 };
            self.line((x0, y), (end, y), c, 1.2);
        }
    }
    /// A small landscape photo: a framed sky with a hill, a mountain and the sun.
    fn photo(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (w, h) = (x1 - x0, y1 - y0);
        self.p.rect(self.rect(x0, y0, x1, y1), ROUND * self.k, self.tint(BLUE), Stroke::new(self.w(1.0), self.c(INK)), StrokeKind::Inside);
        let base = y1 - 1.0;
        self.poly(&[(x0 + 1.0, base), (x0 + w * 0.38, y0 + h * 0.42), (x0 + w * 0.66, base)], GREEN, Some(INK));
        self.poly(&[(x0 + w * 0.46, base), (x0 + w * 0.72, y0 + h * 0.58), (x1 - 1.0, base)], TEAL, Some(INK));
        self.circle(x1 - w * 0.22, y0 + h * 0.28, h * 0.12, GOLD, None);
    }
}

/// Paint icon `name` into `r` (any square). `enabled == false` fades it.
pub fn paint(p: &Painter, r: Rect, name: &str, enabled: bool) {
    let side = r.width().min(r.height());
    let r = Rect::from_center_size(r.center(), vec2(side, side));
    let k = side / 32.0;
    let g = Pen { p, r, k, dim: !enabled, dark: super::pal().dark };
    match name {
        // Clipboard
        "paste" => {
            g.boxed(5.0, 5.0, 23.0, 29.0, GOLD, INK);
            g.boxed(10.0, 3.0, 18.0, 8.0, Color32::from_rgb(120, 120, 120), INK);
            g.page(13.0, 12.0, 28.0, 30.0);
            g.lines(16.0, 25.0, 19.0, 3, 3.5, BLUE);
        }
        "cut" => {
            g.ring(9.0, 24.0, 4.0, INK, 1.6);
            g.ring(23.0, 24.0, 4.0, INK, 1.6);
            g.line((11.5, 20.8), (22.0, 4.0), INK, 1.6);
            g.line((20.5, 20.8), (10.0, 4.0), INK, 1.6);
            g.circle(16.0, 13.0, 1.2, BLUE, None);
        }
        "copy" => {
            g.page(4.0, 3.0, 20.0, 22.0);
            g.page(12.0, 10.0, 28.0, 29.0);
            g.lines(15.0, 25.0, 17.0, 3, 3.5, BLUE);
        }
        "format-painter" => {
            g.boxed(6.0, 3.0, 26.0, 11.0, GOLD, INK);
            g.path(&[(26.0, 7.0), (29.0, 7.0), (29.0, 14.0), (16.0, 14.0), (16.0, 18.0)], INK, 1.6);
            g.boxed(13.5, 18.0, 18.5, 29.0, INK, INK);
        }
        // Undo / redo / save (Quick Access Toolbar)
        // White on Publisher's teal ("undo"), or in ink on a light title bar ("undo-ink").
        "undo" | "undo-ink" => {
            let c = if name == "undo" { PAPER } else { INK };
            g.path(&[(8.5, 11.0), (17.0, 11.0)], c, 2.2);
            g.arc(17.0, 18.5, 7.5, -90.0, 90.0, c, 2.2);
            g.path(&[(17.0, 26.0), (12.0, 26.0)], c, 2.2);
            g.path(&[(12.5, 6.5), (8.0, 11.0), (12.5, 15.5)], c, 2.2);
        }
        "redo" | "redo-ink" => {
            let c = if name == "redo" { PAPER } else { INK };
            g.path(&[(23.5, 11.0), (15.0, 11.0)], c, 2.2);
            g.arc(15.0, 18.5, 7.5, -90.0, -270.0, c, 2.2);
            g.path(&[(15.0, 26.0), (20.0, 26.0)], c, 2.2);
            g.path(&[(19.5, 6.5), (24.0, 11.0), (19.5, 15.5)], c, 2.2);
        }
        // A disk in outline: white on Publisher's teal ("save"), or ink on a plain title bar.
        "save" | "save-dark" => {
            let c = if name == "save" { PAPER } else { INK };
            g.poly(&[(5.0, 5.0), (23.0, 5.0), (27.0, 9.0), (27.0, 27.0), (5.0, 27.0)], Color32::TRANSPARENT, Some(c));
            g.outline(10.0, 5.0, 21.0, 11.5, c, 1.0);
            g.outline(9.0, 16.5, 23.0, 27.0, c, 1.0);
            g.line((17.5, 7.0), (17.5, 9.5), c, 1.2);
        }
        // Objects
        "textbox" => {
            g.boxed(4.0, 6.0, 28.0, 27.0, PAPER, INK);
            g.text(11.0, 13.0, "A", 11.0, BLUE, true);
            g.lines(17.0, 25.0, 11.0, 2, 3.5, INK);
            g.lines(7.0, 25.0, 19.0, 2, 3.5, INK);
            for (x, y) in [(4.0, 6.0), (28.0, 6.0), (4.0, 27.0), (28.0, 27.0)] {
                g.boxed(x - 1.5, y - 1.5, x + 1.5, y + 1.5, PAPER, BLUE);
            }
        }
        "table" => {
            g.boxed(3.0, 6.0, 29.0, 27.0, PAPER, INK);
            g.wash(3.5, 6.5, 28.5, 11.0, BLUE);
            for x in [11.6, 20.3] {
                g.line((x, 6.0), (x, 27.0), INK, 1.0);
            }
            for y in [11.0, 16.3, 21.6] {
                g.line((3.0, y), (29.0, y), INK, 1.0);
            }
        }
        "picture" => g.photo(3.0, 6.0, 29.0, 26.0),
        "online-picture" => {
            g.photo(3.0, 9.0, 23.0, 27.0);
            g.circle(23.0, 9.0, 7.0, BLUE, Some(PAPER));
            g.line((16.0, 9.0), (30.0, 9.0), PAPER, 1.0);
            g.line((23.0, 2.0), (23.0, 16.0), PAPER, 1.0);
        }
        "placeholder" => {
            g.outline(3.0, 6.0, 29.0, 26.0, INK, 1.0);
            g.photo(9.0, 10.0, 23.0, 22.0);
        }
        "shapes" => {
            g.boxed(3.0, 14.0, 16.0, 27.0, BLUE, INK);
            g.circle(21.0, 11.0, 7.5, GOLD, Some(INK));
            g.poly(&[(20.0, 28.0), (25.0, 18.0), (30.0, 28.0)], GREEN, Some(INK));
        }
        "wordart" => {
            g.text(16.0, 17.0, "A", 26.0, BLUE, true);
            g.line((5.0, 28.0), (27.0, 28.0), GOLD, 2.0);
        }
        "symbol" => g.text(16.0, 16.0, "Ω", 22.0, INK, false),
        "datetime" => {
            g.boxed(4.0, 6.0, 28.0, 28.0, PAPER, INK);
            g.fill(4.5, 6.5, 27.5, 11.0, RED);
            for row in 0..3 {
                for col in 0..4 {
                    let (x, y) = (7.5 + col as f32 * 5.0, 14.0 + row as f32 * 4.5);
                    g.fill(x, y, x + 3.0, y + 2.5, GREY);
                }
            }
        }
        "insert-file" => {
            g.page(6.0, 3.0, 26.0, 29.0);
            g.lines(10.0, 22.0, 12.0, 4, 3.5, INK);
        }
        "link" => {
            g.boxed(4.0, 11.0, 17.0, 21.0, Color32::TRANSPARENT, BLUE);
            g.boxed(15.0, 11.0, 28.0, 21.0, Color32::TRANSPARENT, BLUE);
            g.line((11.0, 16.0), (21.0, 16.0), BLUE, 2.0);
        }
        "bookmark" => g.poly(&[(9.0, 4.0), (23.0, 4.0), (23.0, 28.0), (16.0, 22.0), (9.0, 28.0)], BLUE, Some(INK)),
        "header" | "footer" => {
            g.page(6.0, 3.0, 26.0, 29.0);
            let y = if name == "header" { 6.0 } else { 23.0 };
            g.fill(8.0, y, 24.0, y + 3.5, BLUE);
            g.lines(9.0, 22.0, if name == "header" { 14.0 } else { 9.0 }, 3, 3.5, GREY);
        }
        "page-number" => {
            g.page(6.0, 3.0, 26.0, 29.0);
            g.text(16.0, 17.0, "#", 14.0, BLUE, true);
        }
        "page-parts" | "borders" | "calendars" | "ads" => {
            g.page(6.0, 3.0, 26.0, 29.0);
            match name {
                "page-parts" => {
                    g.fill(9.0, 7.0, 23.0, 12.0, TEAL);
                    g.lines(9.0, 23.0, 16.0, 3, 3.5, GREY);
                }
                "borders" => {
                    g.outline(9.0, 7.0, 23.0, 26.0, GOLD, 2.0);
                }
                "calendars" => {
                    g.fill(8.5, 7.0, 23.5, 10.0, RED);
                    for row in 0..4 {
                        for col in 0..4 {
                            let (x, y) = (9.0 + col as f32 * 3.8, 12.5 + row as f32 * 3.5);
                            g.fill(x, y, x + 2.4, y + 2.0, GREY);
                        }
                    }
                }
                _ => {
                    g.boxed(9.0, 8.0, 23.0, 22.0, GOLD, INK);
                    g.text(16.0, 15.0, "$", 10.0, PAPER, true);
                }
            }
        }
        // Pages
        "page" | "page-insert" => {
            g.page(5.0, 3.0, 24.0, 28.0);
            g.lines(9.0, 20.0, 9.0, 3, 3.5, GREY);
            if name == "page-insert" {
                g.circle(24.0, 24.0, 6.5, GREEN, None);
                g.line((20.5, 24.0), (27.5, 24.0), PAPER, 2.0);
                g.line((24.0, 20.5), (24.0, 27.5), PAPER, 2.0);
            }
        }
        "page-delete" => {
            g.page(5.0, 3.0, 24.0, 28.0);
            g.circle(24.0, 24.0, 6.5, RED, None);
            g.line((21.0, 21.0), (27.0, 27.0), PAPER, 2.0);
            g.line((27.0, 21.0), (21.0, 27.0), PAPER, 2.0);
        }
        "page-move" => {
            g.page(4.0, 3.0, 20.0, 24.0);
            g.path(&[(18.0, 26.0), (28.0, 26.0)], BLUE, 2.0);
            g.poly(&[(30.0, 26.0), (25.0, 22.0), (25.0, 30.0)], BLUE, None);
        }
        "master" => {
            g.page(9.0, 3.0, 28.0, 25.0);
            g.page(4.0, 8.0, 23.0, 30.0);
            g.fill(7.0, 11.0, 20.0, 14.0, TEAL);
        }
        "rename" => {
            g.boxed(3.0, 11.0, 29.0, 21.0, PAPER, INK);
            g.text(11.0, 16.0, "ab", 9.0, INK, false);
            g.line((22.0, 9.0), (22.0, 23.0), BLUE, 1.5);
        }
        // Arrange
        "wrap" => {
            g.lines(3.0, 29.0, 5.0, 1, 0.0, INK);
            g.boxed(10.0, 9.0, 22.0, 21.0, BLUE, INK);
            for y in [10.0, 14.0, 18.0] {
                g.line((3.0, y), (8.0, y), INK, 1.2);
                g.line((24.0, y), (29.0, y), INK, 1.2);
            }
            g.line((3.0, 24.0), (29.0, 24.0), INK, 1.2);
            g.line((3.0, 28.0), (20.0, 28.0), INK, 1.2);
        }
        "bring-forward" | "bring-front" => {
            g.boxed(4.0, 4.0, 18.0, 18.0, PAPER, INK);
            g.boxed(12.0, 12.0, 28.0, 28.0, BLUE, INK);
            if name == "bring-front" {
                g.boxed(8.0, 8.0, 22.0, 22.0, GREY, INK);
                g.boxed(12.0, 12.0, 28.0, 28.0, BLUE, INK);
            }
        }
        "send-backward" | "send-back" => {
            g.boxed(12.0, 12.0, 28.0, 28.0, BLUE, INK);
            g.boxed(4.0, 4.0, 18.0, 18.0, PAPER, INK);
        }
        "group" => {
            g.outline(3.0, 3.0, 29.0, 29.0, GREY, 1.0);
            g.boxed(6.0, 6.0, 17.0, 17.0, BLUE, INK);
            g.circle(21.0, 21.0, 5.5, GOLD, Some(INK));
        }
        "ungroup" => {
            g.boxed(3.0, 3.0, 14.0, 14.0, BLUE, INK);
            g.circle(23.0, 23.0, 5.5, GOLD, Some(INK));
        }
        "align" => {
            g.line((4.0, 3.0), (4.0, 29.0), INK, 1.5);
            g.boxed(7.0, 6.0, 26.0, 13.0, BLUE, INK);
            g.boxed(7.0, 18.0, 19.0, 25.0, BLUE, INK);
        }
        "rotate" => {
            g.boxed(8.0, 12.0, 24.0, 28.0, BLUE, INK);
            g.path(&[(6.0, 10.0), (10.0, 5.0), (16.0, 3.5), (22.0, 5.0)], INK, 1.6);
            g.poly(&[(26.0, 7.0), (20.0, 2.0), (20.0, 9.0)], INK, None);
        }
        "select" => {
            g.poly(&[(9.0, 3.0), (9.0, 25.0), (14.0, 20.0), (19.0, 29.0)], PAPER, Some(INK));
            g.poly(&[(9.0, 3.0), (24.0, 18.0), (16.0, 19.0)], PAPER, Some(INK));
        }
        "find" => {
            g.ring(13.0, 13.0, 8.0, INK, 1.8);
            g.line((19.0, 19.0), (27.5, 27.5), INK, 3.0);
        }
        "replace" => {
            g.text(10.0, 11.0, "ab", 10.0, INK, false);
            g.text(22.0, 23.0, "ac", 10.0, BLUE, false);
            g.path(&[(8.0, 18.0), (8.0, 24.0), (14.0, 24.0)], INK, 1.2);
        }
        // Page design
        "template" => {
            g.page(4.0, 3.0, 22.0, 27.0);
            g.wash(7.0, 7.0, 19.0, 12.0, TEAL);
            g.wash(7.0, 14.0, 12.0, 23.0, GOLD);
            g.lines(14.0, 19.0, 15.0, 3, 3.0, GREY);
            g.boxed(18.0, 18.0, 29.0, 29.0, BLUE, INK);
        }
        "margins" => {
            g.page(5.0, 3.0, 27.0, 29.0);
            g.outline(9.0, 7.0, 23.0, 25.0, BLUE, 1.0);
        }
        "orientation" => {
            g.page(3.0, 10.0, 18.0, 30.0);
            g.boxed(13.0, 3.0, 30.0, 17.0, PAPER, INK);
            g.path(&[(6.0, 7.0), (6.0, 4.0), (10.0, 4.0)], BLUE, 1.5);
        }
        "size" => {
            g.page(8.0, 7.0, 25.0, 29.0);
            g.line((4.0, 3.0), (29.0, 3.0), BLUE, 1.2);
            g.line((4.0, 1.0), (4.0, 5.0), BLUE, 1.2);
            g.line((29.0, 1.0), (29.0, 5.0), BLUE, 1.2);
            g.line((3.0, 7.0), (3.0, 29.0), BLUE, 1.2);
        }
        "guides" => {
            g.page(5.0, 3.0, 27.0, 29.0);
            for x in [11.0, 21.0] {
                g.line((x, 3.5), (x, 28.5), BLUE, 1.0);
            }
            g.line((5.5, 12.0), (26.5, 12.0), BLUE, 1.0);
        }
        "align-to" => {
            g.outline(4.0, 4.0, 28.0, 28.0, GREY, 1.0);
            g.fill(8.0, 13.0, 24.0, 19.0, BLUE);
        }
        "schemes" => {
            for (i, c) in [TEAL, GOLD, BLUE, RED].into_iter().enumerate() {
                let x = 4.0 + i as f32 * 6.5;
                g.boxed(x, 6.0, x + 6.0, 26.0, c, INK);
            }
        }
        "fonts" => {
            g.text(11.0, 13.0, "A", 16.0, INK, true);
            g.text(22.0, 21.0, "a", 14.0, BLUE, false);
        }
        "background" => {
            g.boxed(5.0, 3.0, 27.0, 29.0, Color32::from_rgb(190, 225, 220), INK);
            g.page(10.0, 8.0, 22.0, 24.0);
        }
        // Mailings / review
        "mail-merge" => {
            g.boxed(3.0, 8.0, 29.0, 26.0, PAPER, INK);
            g.path(&[(3.5, 8.5), (16.0, 18.0), (28.5, 8.5)], INK, 1.2);
        }
        "spelling" => {
            g.text(12.0, 11.0, "ABC", 10.0, INK, true);
            g.path(&[(9.0, 21.0), (14.0, 27.0), (26.0, 15.0)], GREEN, 2.6);
        }
        "research" | "thesaurus" | "translate" | "language" => {
            g.boxed(5.0, 4.0, 27.0, 28.0, PAPER, INK);
            g.line((16.0, 4.0), (16.0, 28.0), INK, 1.0);
            g.lines(7.0, 14.0, 9.0, 4, 4.0, GREY);
            g.lines(18.0, 25.0, 9.0, 4, 4.0, BLUE);
        }
        // View
        "normal-view" => {
            g.page(7.0, 3.0, 25.0, 29.0);
            g.lines(10.0, 22.0, 9.0, 4, 4.0, GREY);
        }
        "master-view" => {
            g.page(7.0, 3.0, 25.0, 29.0);
            g.fill(9.0, 6.0, 23.0, 10.0, TEAL);
            g.fill(9.0, 23.0, 23.0, 26.0, TEAL);
        }
        "single-page" => g.page(9.0, 4.0, 23.0, 28.0),
        "two-page" => {
            g.page(2.0, 5.0, 16.0, 27.0);
            g.page(16.0, 5.0, 30.0, 27.0);
        }
        "boundaries" => {
            g.page(5.0, 3.0, 27.0, 29.0);
            g.p.rect_stroke(g.rect(9.0, 7.0, 23.0, 25.0), 0.0, Stroke::new(g.w(1.0), g.c(BLUE)), StrokeKind::Inside);
            g.outline(11.0, 10.0, 21.0, 16.0, GOLD, 1.0);
        }
        "zoom" => {
            g.ring(13.0, 13.0, 8.0, INK, 1.8);
            g.line((19.0, 19.0), (27.5, 27.5), INK, 3.0);
            g.line((9.0, 13.0), (17.0, 13.0), BLUE, 1.6);
            g.line((13.0, 9.0), (13.0, 17.0), BLUE, 1.6);
        }
        "zoom-100" => {
            g.ring(13.0, 13.0, 9.0, INK, 1.6);
            g.line((20.0, 20.0), (28.0, 28.0), INK, 3.0);
            g.text(13.0, 13.0, "100", 6.5, BLUE, true);
        }
        "whole-page" => {
            g.outline(3.0, 3.0, 29.0, 29.0, BLUE, 1.0);
            g.page(9.0, 6.0, 23.0, 26.0);
        }
        "page-width" => {
            g.page(3.0, 6.0, 29.0, 29.0);
            g.path(&[(6.0, 15.0), (26.0, 15.0)], BLUE, 1.5);
            g.poly(&[(4.0, 15.0), (8.0, 12.0), (8.0, 18.0)], BLUE, None);
            g.poly(&[(28.0, 15.0), (24.0, 12.0), (24.0, 18.0)], BLUE, None);
        }
        "selected-objects" => {
            g.ring(13.0, 13.0, 8.0, INK, 1.8);
            g.line((19.0, 19.0), (27.5, 27.5), INK, 3.0);
            g.boxed(9.0, 9.0, 17.0, 17.0, BLUE, INK);
        }
        "nav-pane" => {
            g.boxed(3.0, 4.0, 29.0, 28.0, PAPER, INK);
            g.fill(4.0, 5.0, 11.0, 27.0, Color32::from_rgb(220, 220, 220));
            g.page(5.5, 7.0, 9.5, 13.0);
            g.page(5.5, 16.0, 9.5, 22.0);
        }
        "rulers" => {
            g.boxed(3.0, 3.0, 29.0, 9.0, Color32::from_rgb(245, 240, 220), INK);
            g.boxed(3.0, 9.0, 9.0, 29.0, Color32::from_rgb(245, 240, 220), INK);
            for x in [13.0, 17.0, 21.0, 25.0] {
                g.line((x, 3.0), (x, 6.0), INK, 1.0);
            }
        }
        "baselines" => {
            g.page(5.0, 3.0, 27.0, 29.0);
            for y in [9.0, 13.0, 17.0, 21.0, 25.0] {
                g.line((7.0, y), (25.0, y), Color32::from_rgb(110, 170, 230), 1.0);
            }
        }
        "arrange-all" | "cascade" | "switch-windows" | "new-window" => {
            g.boxed(3.0, 3.0, 22.0, 19.0, PAPER, INK);
            g.fill(3.5, 3.5, 21.5, 7.0, TEAL);
            g.boxed(10.0, 12.0, 29.0, 28.0, PAPER, INK);
            g.fill(10.5, 12.5, 28.5, 16.0, TEAL);
        }
        // Text-box tools
        "create-link" => {
            g.boxed(2.0, 4.0, 14.0, 20.0, PAPER, INK);
            g.boxed(18.0, 12.0, 30.0, 28.0, PAPER, INK);
            g.path(&[(10.0, 20.0), (10.0, 24.0), (18.0, 24.0)], BLUE, 1.8);
            g.lines(4.0, 12.0, 8.0, 3, 3.5, GREY);
        }
        "break-link" => {
            g.boxed(2.0, 4.0, 14.0, 20.0, PAPER, INK);
            g.boxed(18.0, 12.0, 30.0, 28.0, PAPER, INK);
            g.line((12.0, 22.0), (20.0, 30.0), RED, 2.0);
            g.line((20.0, 22.0), (12.0, 30.0), RED, 2.0);
        }
        "text-fit" => {
            g.boxed(4.0, 6.0, 28.0, 26.0, PAPER, INK);
            g.text(16.0, 16.0, "A", 14.0, BLUE, true);
            g.line((6.0, 29.0), (26.0, 29.0), INK, 1.0);
        }
        "drop-cap" => {
            g.text(10.0, 13.0, "A", 16.0, BLUE, true);
            g.lines(18.0, 28.0, 6.0, 3, 4.0, INK);
            g.lines(4.0, 28.0, 22.0, 2, 4.0, INK);
        }
        "columns" => {
            g.page(4.0, 3.0, 28.0, 29.0);
            g.lines(7.0, 14.0, 8.0, 5, 3.5, GREY);
            g.lines(18.0, 25.0, 8.0, 5, 3.5, GREY);
        }
        "shape-fill" => {
            g.poly(&[(6.0, 14.0), (15.0, 5.0), (25.0, 15.0), (16.0, 24.0)], PAPER, Some(INK));
            g.fill(3.0, 26.0, 29.0, 30.0, GOLD);
        }
        "shape-outline" => {
            g.path(&[(6.0, 22.0), (20.0, 8.0), (24.0, 12.0), (10.0, 26.0), (6.0, 26.0), (6.0, 22.0)], INK, 1.4);
            g.fill(3.0, 27.0, 29.0, 30.0, BLUE);
        }
        "crop" => {
            g.path(&[(8.0, 2.0), (8.0, 24.0), (30.0, 24.0)], INK, 2.4);
            g.path(&[(2.0, 8.0), (24.0, 8.0), (24.0, 30.0)], INK, 2.4);
        }
        "change-picture" => {
            g.photo(3.0, 4.0, 22.0, 20.0);
            g.circle(23.0, 23.0, 7.0, BLUE, None);
            g.path(&[(19.5, 23.0), (22.0, 20.0), (26.0, 20.5)], PAPER, 1.6);
        }
        "corrections" | "recolor" | "compress" | "reset-picture" => {
            g.photo(3.0, 6.0, 29.0, 26.0);
            let c = match name {
                "corrections" => GOLD,
                "recolor" => RED,
                "compress" => GREY,
                _ => BLUE,
            };
            g.fill(3.0, 27.0, 29.0, 30.0, c);
        }
        // File (backstage)
        "new" => g.page(7.0, 3.0, 25.0, 29.0),
        "open" => {
            g.poly(&[(3.0, 8.0), (12.0, 8.0), (14.0, 11.0), (27.0, 11.0), (27.0, 26.0), (3.0, 26.0)], GOLD, Some(INK));
            g.poly(&[(3.0, 26.0), (8.0, 15.0), (30.0, 15.0), (27.0, 26.0)], Color32::from_rgb(240, 196, 100), Some(INK));
        }
        "print" => {
            g.boxed(9.0, 3.0, 23.0, 12.0, PAPER, INK);
            g.boxed(3.0, 11.0, 29.0, 23.0, GREY, INK);
            g.boxed(9.0, 18.0, 23.0, 29.0, PAPER, INK);
            g.lines(11.0, 21.0, 22.0, 2, 3.0, INK);
        }
        "export" | "pdf" => {
            g.page(6.0, 3.0, 26.0, 29.0);
            g.fill(6.5, 17.0, 25.5, 25.0, RED);
            g.text(16.0, 21.0, "PDF", 6.0, PAPER, true);
        }
        "share" => {
            g.circle(8.0, 16.0, 4.0, BLUE, None);
            g.circle(24.0, 7.0, 4.0, BLUE, None);
            g.circle(24.0, 25.0, 4.0, BLUE, None);
            g.line((8.0, 16.0), (24.0, 7.0), BLUE, 1.6);
            g.line((8.0, 16.0), (24.0, 25.0), BLUE, 1.6);
        }
        "info" => {
            g.circle(16.0, 16.0, 12.0, BLUE, Some(INK));
            g.text(16.0, 17.0, "i", 14.0, BLUE, true);
        }
        "close" => {
            g.line((8.0, 8.0), (24.0, 24.0), INK, 2.0);
            g.line((24.0, 8.0), (8.0, 24.0), INK, 2.0);
        }
        "options" => {
            // A cog: eight teeth round a ring.
            for i in 0..8 {
                let a = (i as f32 * 45.0).to_radians();
                g.line((16.0 + 8.5 * a.cos(), 16.0 + 8.5 * a.sin()), (16.0 + 12.0 * a.cos(), 16.0 + 12.0 * a.sin()), INK, 3.2);
            }
            g.circle(16.0, 16.0, 9.0, PAPER, Some(INK));
            g.ring(16.0, 16.0, 3.5, BLUE, 1.6);
        }
        // Fonts and paragraphs (small)
        // Letters fill the box, as Publisher's do.
        "grow-font" => {
            g.text(12.0, 17.0, "A", 27.0, INK, false);
            g.poly(&[(22.0, 9.0), (26.5, 2.5), (31.0, 9.0)], BLUE, None);
        }
        "shrink-font" => {
            g.text(12.0, 18.0, "A", 21.0, INK, false);
            g.poly(&[(22.0, 3.0), (26.5, 9.5), (31.0, 3.0)], BLUE, None);
        }
        "clear-format" => {
            g.text(12.0, 14.0, "A", 24.0, INK, false);
            g.poly(&[(16.0, 25.0), (23.0, 18.0), (30.0, 24.0), (23.0, 31.0)], Color32::from_rgb(235, 140, 160), Some(INK));
        }
        "font-color" => {
            g.text(16.0, 12.5, "A", 25.0, INK, false);
            g.fill(3.0, 25.0, 29.0, 30.5, RED);
        }
        "change-case" => g.text(16.0, 16.0, "Aa", 22.0, INK, false),
        "char-spacing" => {
            g.text(16.0, 12.0, "AV", 19.0, INK, false);
            g.path(&[(4.0, 26.0), (28.0, 26.0)], BLUE, 1.8);
            g.poly(&[(1.5, 26.0), (6.0, 22.5), (6.0, 29.5)], BLUE, None);
            g.poly(&[(30.5, 26.0), (26.0, 22.5), (26.0, 29.5)], BLUE, None);
        }
        "subscript" => {
            g.text(12.0, 13.0, "x", 25.0, INK, false);
            g.text(25.0, 24.0, "2", 15.0, BLUE, false);
        }
        "superscript" => {
            g.text(12.0, 19.0, "x", 25.0, INK, false);
            g.text(25.0, 7.0, "2", 15.0, BLUE, false);
        }
        "bullets" => {
            for (i, y) in [7.0, 16.0, 25.0].into_iter().enumerate() {
                let _ = i;
                g.circle(6.0, y, 2.4, INK, None);
                g.line((12.0, y), (29.0, y), INK, 1.6);
            }
        }
        "numbering" => {
            for (i, y) in [7.0, 16.0, 25.0].into_iter().enumerate() {
                g.text(5.0, y, &format!("{}", i + 1), 9.0, INK, true);
                g.line((12.0, y), (29.0, y), INK, 1.6);
            }
        }
        "indent-less" | "indent-more" => {
            for y in [5.0, 27.0] {
                g.line((4.0, y), (29.0, y), INK, 1.6);
            }
            for y in [12.0, 20.0] {
                g.line((14.0, y), (29.0, y), INK, 1.6);
            }
            if name == "indent-more" {
                g.poly(&[(3.0, 11.0), (9.0, 16.0), (3.0, 21.0)], BLUE, None);
            } else {
                g.poly(&[(9.0, 11.0), (3.0, 16.0), (9.0, 21.0)], BLUE, None);
            }
        }
        "pilcrow" => g.text(16.0, 16.0, "¶", 22.0, INK, false),
        "line-spacing" => {
            for y in [7.0, 14.0, 21.0, 28.0] {
                g.line((13.0, y), (29.0, y), INK, 1.6);
            }
            g.line((6.0, 6.0), (6.0, 26.0), BLUE, 1.5);
            g.poly(&[(6.0, 2.0), (2.5, 7.0), (9.5, 7.0)], BLUE, None);
            g.poly(&[(6.0, 30.0), (2.5, 25.0), (9.5, 25.0)], BLUE, None);
        }
        "align-left" | "align-center" | "align-right" | "justify" | "distribute" => {
            for (i, y) in [5.0, 12.0, 19.0, 26.0].into_iter().enumerate() {
                let long = i % 2 == 0;
                let (x0, x1) = match name {
                    "align-left" => (3.0, if long { 29.0 } else { 20.0 }),
                    "align-center" => {
                        if long {
                            (3.0, 29.0)
                        } else {
                            (8.0, 24.0)
                        }
                    }
                    "align-right" => (if long { 3.0 } else { 12.0 }, 29.0),
                    _ => (3.0, 29.0),
                };
                g.line((x0, y), (x1, y), INK, 2.0);
            }
        }
        "paragraph-dialog" | "font-dialog" => {
            g.path(&[(8.0, 8.0), (24.0, 24.0)], INK, 1.4);
            g.path(&[(14.0, 24.0), (24.0, 24.0), (24.0, 14.0)], INK, 1.4);
        }
        "styles" => {
            g.boxed(3.0, 5.0, 29.0, 27.0, PAPER, INK);
            g.text(11.0, 13.0, "Aa", 10.0, BLUE, false);
            g.lines(6.0, 26.0, 21.0, 1, 0.0, INK);
        }
        "draw-table" => {
            g.boxed(3.0, 6.0, 29.0, 27.0, PAPER, INK);
            g.line((14.0, 6.0), (14.0, 27.0), INK, 1.0);
            g.line((3.0, 16.0), (29.0, 16.0), INK, 1.0);
            g.line((20.0, 22.0), (28.0, 30.0), GOLD, 2.4);
        }
        // Table Tools › Layout: a grid with the new row or column in blue.
        "row-above" | "row-below" | "col-left" | "col-right" => {
            g.boxed(4.0, 6.0, 28.0, 28.0, PAPER, INK);
            match name {
                "row-above" => g.fill(4.5, 6.5, 27.5, 13.0, SKY),
                "row-below" => g.fill(4.5, 21.0, 27.5, 27.5, SKY),
                "col-left" => g.fill(4.5, 6.5, 12.0, 27.5, SKY),
                _ => g.fill(20.0, 6.5, 27.5, 27.5, SKY),
            }
            for x in [12.0, 20.0] {
                g.line((x, 6.0), (x, 28.0), INK, 1.0);
            }
            for y in [13.3, 20.6] {
                g.line((4.0, y), (28.0, y), INK, 1.0);
            }
            let (cx, cy) = match name {
                "row-above" => (16.0, 9.7),
                "row-below" => (16.0, 24.3),
                "col-left" => (8.2, 17.0),
                _ => (23.8, 17.0),
            };
            g.line((cx - 2.6, cy), (cx + 2.6, cy), BLUE, 1.6);
            g.line((cx, cy - 2.6), (cx, cy + 2.6), BLUE, 1.6);
        }
        "table-delete" => {
            g.boxed(3.0, 6.0, 25.0, 26.0, PAPER, INK);
            for x in [10.3, 17.6] {
                g.line((x, 6.0), (x, 26.0), INK, 1.0);
            }
            for y in [12.6, 19.3] {
                g.line((3.0, y), (25.0, y), INK, 1.0);
            }
            g.line((19.0, 19.0), (29.0, 29.0), RED, 2.6);
            g.line((29.0, 19.0), (19.0, 29.0), RED, 2.6);
        }
        "merge-cells" | "split-cells" => {
            g.boxed(3.0, 9.0, 29.0, 24.0, PAPER, INK);
            if name == "split-cells" {
                g.line((16.0, 9.0), (16.0, 24.0), INK, 1.0);
                g.path(&[(11.0, 14.0), (8.0, 16.5), (11.0, 19.0)], BLUE, 1.6);
                g.path(&[(21.0, 14.0), (24.0, 16.5), (21.0, 19.0)], BLUE, 1.6);
            } else {
                g.path(&[(8.0, 14.0), (11.0, 16.5), (8.0, 19.0)], BLUE, 1.6);
                g.path(&[(24.0, 14.0), (21.0, 16.5), (24.0, 19.0)], BLUE, 1.6);
                g.line((16.0, 9.0), (16.0, 12.0), INK, 1.0);
                g.line((16.0, 21.0), (16.0, 24.0), INK, 1.0);
            }
        }
        // Text at the top, middle or bottom of a cell.
        "cell-top" | "cell-middle" | "cell-bottom" => {
            g.boxed(5.0, 5.0, 27.0, 27.0, PAPER, INK);
            let y = match name {
                "cell-top" => 9.0,
                "cell-middle" => 14.5,
                _ => 20.0,
            };
            g.lines(9.0, 23.0, y, 2, 3.5, BLUE);
        }
        // Help tab.
        "help" => {
            g.circle(16.0, 16.0, 12.0, TEAL, Some(INK));
            g.text(16.0, 16.5, "?", 17.0, TEAL, true);
        }
        "feedback" => {
            g.poly(&[(4.0, 6.0), (28.0, 6.0), (28.0, 22.0), (4.0, 22.0)], PAPER, Some(INK));
            g.poly(&[(9.0, 21.5), (16.0, 21.5), (9.0, 28.0)], PAPER, Some(INK));
            g.line((8.0, 11.0), (24.0, 11.0), BLUE, 1.6);
            g.line((8.0, 16.0), (19.0, 16.0), BLUE, 1.6);
        }
        "whats-new" => {
            // A four-point sparkle, as two diamonds (shapes are filled convex).
            g.poly(&[(16.0, 3.0), (19.5, 16.0), (16.0, 29.0), (12.5, 16.0)], GOLD, None);
            g.poly(&[(3.0, 16.0), (16.0, 12.5), (29.0, 16.0), (16.0, 19.5)], GOLD, None);
            g.circle(25.0, 7.0, 2.2, TEAL, None);
        }
        "update" => {
            g.path(&[(25.0, 11.0), (21.0, 6.5), (15.0, 5.5), (9.0, 8.0), (6.0, 13.0), (6.0, 18.0)], TEAL, 2.4);
            g.poly(&[(27.5, 5.0), (27.5, 13.5), (19.5, 12.0)], TEAL, None);
            g.path(&[(7.0, 21.0), (11.0, 25.5), (17.0, 26.5), (23.0, 24.0), (26.0, 19.0), (26.0, 14.0)], TEAL, 2.4);
            g.poly(&[(4.5, 27.0), (4.5, 18.5), (12.5, 20.0)], TEAL, None);
        }
        _ => {
            g.outline(6.0, 6.0, 26.0, 26.0, GREY, 1.0);
        }
    }
}

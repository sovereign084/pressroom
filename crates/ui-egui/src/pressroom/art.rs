//! PressRoom's ribbon icons, drawn in code on a 32-unit grid (large buttons) or the same grid
//! scaled into 16 px (small buttons). Original art: flat Office-style pictograms in a few inks.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

pub const INK: Color32 = Color32::from_rgb(68, 68, 68);
pub const BLUE: Color32 = Color32::from_rgb(43, 120, 196);
pub const TEAL: Color32 = Color32::from_rgb(7, 117, 104);
pub const GOLD: Color32 = Color32::from_rgb(214, 152, 46);
pub const RED: Color32 = Color32::from_rgb(209, 52, 56);
pub const GREEN: Color32 = Color32::from_rgb(73, 150, 66);
pub const PAPER: Color32 = Color32::WHITE;
pub const SKY: Color32 = Color32::from_rgb(160, 205, 240);
pub const GREY: Color32 = Color32::from_rgb(150, 150, 150);

/// A pen on a 32 × 32 grid laid over `r`.
struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    /// Grid units → pixels.
    k: f32,
    /// Fade everything for a disabled button.
    dim: bool,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.min.x + x * self.k, self.r.min.y + y * self.k)
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.at(x0, y0), self.at(x1, y1))
    }
    fn c(&self, c: Color32) -> Color32 {
        if self.dim { c.gamma_multiply(0.35) } else { c }
    }
    fn w(&self, w: f32) -> f32 {
        (w * self.k).max(1.0)
    }
    fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_filled(self.rect(x0, y0, x1, y1), 0.0, self.c(c));
    }
    fn boxed(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32, line: Color32) {
        let r = self.rect(x0, y0, x1, y1);
        self.p.rect(r, 0.0, self.c(fill), Stroke::new(self.w(1.0), self.c(line)), StrokeKind::Inside);
    }
    fn outline(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32, w: f32) {
        self.p.rect_stroke(self.rect(x0, y0, x1, y1), 0.0, Stroke::new(self.w(w), self.c(c)), StrokeKind::Inside);
    }
    fn line(&self, a: (f32, f32), b: (f32, f32), c: Color32, w: f32) {
        self.p.line_segment([self.at(a.0, a.1), self.at(b.0, b.1)], Stroke::new(self.w(w), self.c(c)));
    }
    fn poly(&self, pts: &[(f32, f32)], fill: Color32, line: Option<Color32>) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        let stroke = line.map_or(Stroke::NONE, |c| Stroke::new(self.w(1.0), self.c(c)));
        self.p.add(Shape::convex_polygon(v, self.c(fill), stroke));
    }
    fn path(&self, pts: &[(f32, f32)], c: Color32, w: f32) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.p.add(Shape::line(v, Stroke::new(self.w(w), self.c(c))));
    }
    fn circle(&self, x: f32, y: f32, rad: f32, fill: Color32, line: Option<Color32>) {
        let stroke = line.map_or(Stroke::NONE, |c| Stroke::new(self.w(1.0), self.c(c)));
        self.p.circle(self.at(x, y), rad * self.k, self.c(fill), stroke);
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
        self.path(&[(x1 - f, y0), (x1 - f, y0 + f), (x1, y0 + f)], INK, 1.0);
    }
    /// Text lines across a span.
    fn lines(&self, x0: f32, x1: f32, y0: f32, n: usize, gap: f32, c: Color32) {
        for i in 0..n {
            let y = y0 + i as f32 * gap;
            let end = if i + 1 == n { x0 + (x1 - x0) * 0.6 } else { x1 };
            self.line((x0, y), (end, y), c, 1.2);
        }
    }
    /// A small landscape photo.
    fn photo(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.boxed(x0, y0, x1, y1, SKY, INK);
        let h = y1 - y0;
        self.poly(&[(x0 + 1.0, y1 - 1.0), (x0 + (x1 - x0) * 0.4, y0 + h * 0.45), (x0 + (x1 - x0) * 0.7, y1 - 1.0)], GREEN, None);
        self.poly(&[(x0 + (x1 - x0) * 0.45, y1 - 1.0), (x0 + (x1 - x0) * 0.72, y0 + h * 0.6), (x1 - 1.0, y1 - 1.0)], TEAL, None);
        self.circle(x1 - (x1 - x0) * 0.22, y0 + h * 0.28, h * 0.12, GOLD, None);
    }
}

/// Paint icon `name` into `r` (any square). `enabled == false` fades it.
pub fn paint(p: &Painter, r: Rect, name: &str, enabled: bool) {
    let side = r.width().min(r.height());
    let r = Rect::from_center_size(r.center(), vec2(side, side));
    let k = side / 32.0;
    let g = Pen { p, r, k, dim: !enabled };
    match name {
        // Clipboard
        "paste" => {
            g.boxed(5.0, 5.0, 23.0, 29.0, GOLD, INK);
            g.boxed(10.0, 3.0, 18.0, 8.0, Color32::from_rgb(120, 120, 120), INK);
            g.page(13.0, 12.0, 28.0, 30.0);
            g.lines(16.0, 25.0, 19.0, 3, 3.5, BLUE);
        }
        "cut" => {
            g.ring(9.0, 24.0, 4.0, INK, 2.0);
            g.ring(23.0, 24.0, 4.0, INK, 2.0);
            g.line((11.5, 21.0), (22.0, 4.0), INK, 2.0);
            g.line((20.5, 21.0), (10.0, 4.0), INK, 2.0);
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
        "undo" => {
            g.path(&[(8.0, 12.0), (20.0, 12.0), (25.0, 16.0), (25.0, 21.0), (20.0, 25.0), (12.0, 25.0)], PAPER, 2.6);
            g.poly(&[(3.0, 12.0), (10.0, 6.0), (10.0, 18.0)], PAPER, None);
        }
        "redo" => {
            g.path(&[(24.0, 12.0), (12.0, 12.0), (7.0, 16.0), (7.0, 21.0), (12.0, 25.0), (20.0, 25.0)], PAPER, 2.6);
            g.poly(&[(29.0, 12.0), (22.0, 6.0), (22.0, 18.0)], PAPER, None);
        }
        "save" | "save-dark" => {
            let c = if name == "save" { PAPER } else { BLUE };
            let inner = if name == "save" { TEAL } else { PAPER };
            g.poly(&[(5.0, 5.0), (24.0, 5.0), (27.0, 8.0), (27.0, 27.0), (5.0, 27.0)], c, None);
            g.fill(10.0, 5.0, 21.0, 12.0, inner);
            g.fill(17.0, 6.5, 19.5, 10.5, c);
            g.fill(9.0, 17.0, 23.0, 27.0, inner);
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
            g.fill(3.5, 6.5, 28.5, 11.0, BLUE);
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
            g.fill(6.0, 6.0, 24.0, 12.0, BLUE);
            g.fill(6.0, 18.0, 17.0, 24.0, BLUE);
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
            g.ring(13.0, 13.0, 8.0, INK, 2.2);
            g.line((19.0, 19.0), (28.0, 28.0), INK, 3.5);
        }
        "replace" => {
            g.text(10.0, 11.0, "ab", 10.0, INK, false);
            g.text(22.0, 23.0, "ac", 10.0, BLUE, false);
            g.path(&[(8.0, 18.0), (8.0, 24.0), (14.0, 24.0)], INK, 1.2);
        }
        // Page design
        "template" => {
            g.page(4.0, 3.0, 22.0, 27.0);
            g.fill(7.0, 7.0, 19.0, 12.0, TEAL);
            g.fill(7.0, 14.0, 12.0, 23.0, GOLD);
            g.lines(14.0, 19.0, 15.0, 3, 3.0, GREY);
            g.boxed(18.0, 18.0, 29.0, 29.0, BLUE, PAPER);
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
            g.ring(13.0, 13.0, 8.0, INK, 2.0);
            g.line((19.0, 19.0), (28.0, 28.0), INK, 3.5);
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
            g.ring(13.0, 13.0, 8.0, INK, 2.0);
            g.line((19.0, 19.0), (28.0, 28.0), INK, 3.5);
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
            g.ring(16.0, 16.0, 12.0, BLUE, 2.0);
            g.text(16.0, 17.0, "i", 14.0, BLUE, true);
        }
        "close" => {
            g.line((8.0, 8.0), (24.0, 24.0), INK, 2.4);
            g.line((24.0, 8.0), (8.0, 24.0), INK, 2.4);
        }
        "options" => {
            g.ring(16.0, 16.0, 9.0, INK, 3.0);
            g.circle(16.0, 16.0, 3.0, INK, None);
        }
        // Fonts and paragraphs (small)
        "grow-font" => {
            g.text(12.0, 18.0, "A", 20.0, INK, false);
            g.poly(&[(22.0, 10.0), (26.0, 4.0), (30.0, 10.0)], BLUE, None);
        }
        "shrink-font" => {
            g.text(12.0, 19.0, "A", 15.0, INK, false);
            g.poly(&[(22.0, 4.0), (26.0, 10.0), (30.0, 4.0)], BLUE, None);
        }
        "clear-format" => {
            g.text(12.0, 15.0, "A", 18.0, INK, false);
            g.poly(&[(16.0, 24.0), (23.0, 17.0), (29.0, 23.0), (22.0, 30.0)], Color32::from_rgb(235, 140, 160), Some(INK));
        }
        "font-color" => {
            g.text(16.0, 13.0, "A", 18.0, INK, false);
            g.fill(4.0, 25.0, 28.0, 30.0, RED);
        }
        "change-case" => g.text(16.0, 16.0, "Aa", 16.0, INK, false),
        "char-spacing" => {
            g.text(16.0, 13.0, "AV", 14.0, INK, false);
            g.path(&[(4.0, 26.0), (28.0, 26.0)], BLUE, 1.5);
            g.poly(&[(2.0, 26.0), (6.0, 23.0), (6.0, 29.0)], BLUE, None);
            g.poly(&[(30.0, 26.0), (26.0, 23.0), (26.0, 29.0)], BLUE, None);
        }
        "subscript" => {
            g.text(12.0, 14.0, "x", 18.0, INK, false);
            g.text(24.0, 24.0, "2", 11.0, BLUE, false);
        }
        "superscript" => {
            g.text(12.0, 18.0, "x", 18.0, INK, false);
            g.text(24.0, 8.0, "2", 11.0, BLUE, false);
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
        _ => {
            g.outline(6.0, 6.0, 26.0, 26.0, GREY, 1.0);
        }
    }
}

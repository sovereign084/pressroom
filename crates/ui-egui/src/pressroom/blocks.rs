//! Insert › Building Blocks: Page Parts, Calendars, Borders & Accents and Advertisements,
//! PressRoom's own designs. Each block is a list of parts (boxes, circles, stars, lines, text,
//! a month's calendar) that draws its own gallery picture and builds itself from ordinary text
//! boxes, shapes and tables, grouped, in the middle of the page in view, as one undo step.
//!
//! Blocks are coloured with the publication's colour scheme ([`super::scheme`]) and their headings
//! use its Heading style, so Color Schemes and Fonts change them afterwards, as in Publisher.

use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use super::scheme;
use crate::DesignApp;

/// A colour: a scheme colour (0 = Main, 1–5 = accents) at a tint, or paper.
#[derive(Clone, Copy)]
pub enum Ink {
    Scheme(usize, f32),
    Paper,
}

impl Ink {
    fn swatch(self) -> (&'static str, f32) {
        match self {
            Ink::Scheme(i, t) => (scheme::SWATCHES.get(i).copied().unwrap_or("Scheme Main"), t),
            Ink::Paper => ("[Paper]", 1.0),
        }
    }

    fn shown(self, colors: &[Color32; 6]) -> Color32 {
        match self {
            Ink::Scheme(i, t) => {
                let c = colors.get(i).copied().unwrap_or(Color32::BLACK);
                let mix = |v: u8| (f32::from(v) * t + 255.0 * (1.0 - t)).round() as u8;
                Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
            }
            Ink::Paper => Color32::WHITE,
        }
    }
}

/// How text sits in its box.
#[derive(Clone, Copy)]
pub struct Text {
    pub text: &'static str,
    pub size: f64,
    pub heading: bool,
    pub bold: bool,
    pub italic: bool,
    pub ink: Ink,
    pub align: &'static str,
    pub middle: bool,
    pub columns: u32,
}

const fn text(text: &'static str, size: f64, ink: Ink) -> Text {
    Text { text, size, heading: false, bold: false, italic: false, ink, align: "left", middle: false, columns: 1 }
}

/// One part of a block, in points from the block's top left.
#[derive(Clone, Copy)]
pub enum Part {
    /// A box: fill, outline (ink, weight, stroke kind), corner rounding.
    Box {
        r: [f64; 4],
        fill: Option<Ink>,
        line: Option<(Ink, f64, &'static str)>,
        round: f64,
    },
    Circle {
        r: [f64; 4],
        fill: Ink,
    },
    Star {
        r: [f64; 4],
        fill: Ink,
        points: u32,
    },
    Line {
        a: [f64; 2],
        b: [f64; 2],
        ink: Ink,
        weight: f64,
        kind: &'static str,
    },
    Words {
        r: [f64; 4],
        t: Text,
        fill: Option<Ink>,
    },
    /// A month's calendar: next month or this one, plain or with a coloured header row.
    Calendar {
        r: [f64; 4],
        next: bool,
        banded: bool,
    },
}

pub struct Block {
    pub name: &'static str,
    pub size: (f64, f64),
    pub parts: &'static [Part],
}

const A1: Ink = Ink::Scheme(1, 1.0);
const A2: Ink = Ink::Scheme(2, 1.0);
const A3: Ink = Ink::Scheme(3, 1.0);
const MAIN: Ink = Ink::Scheme(0, 1.0);
const PALE: Ink = Ink::Scheme(1, 0.14);

const fn heading(s: &'static str, size: f64, ink: Ink) -> Text {
    Text { heading: true, ..text(s, size, ink) }
}

pub const PAGE_PARTS: [Block; 6] = [
    Block {
        name: "Heading: Color Bar",
        size: (432.0, 40.0),
        parts: &[Part::Words { r: [0.0, 0.0, 432.0, 40.0], t: Text { middle: true, ..heading("Heading", 22.0, Ink::Paper) }, fill: Some(A1) }],
    },
    Block {
        name: "Heading: Underlined",
        size: (432.0, 46.0),
        parts: &[
            Part::Words { r: [0.0, 0.0, 432.0, 38.0], t: heading("Heading", 24.0, A1), fill: None },
            Part::Line { a: [0.0, 42.0], b: [432.0, 42.0], ink: A1, weight: 2.0, kind: "solid" },
        ],
    },
    Block {
        name: "Pull Quote: Quotation Marks",
        size: (252.0, 128.0),
        parts: &[
            Part::Line { a: [0.0, 2.0], b: [252.0, 2.0], ink: A1, weight: 1.5, kind: "solid" },
            Part::Words { r: [0.0, 8.0, 40.0, 56.0], t: heading("\u{201c}", 48.0, A1), fill: None },
            Part::Words {
                r: [40.0, 16.0, 252.0, 116.0],
                t: Text { italic: true, ..text("Type a quote from the story here to catch the reader\u{2019}s eye.", 14.0, MAIN) },
                fill: None,
            },
            Part::Line { a: [0.0, 124.0], b: [252.0, 124.0], ink: A1, weight: 1.5, kind: "solid" },
        ],
    },
    Block {
        name: "Pull Quote: Tinted Box",
        size: (252.0, 110.0),
        parts: &[
            Part::Box { r: [0.0, 0.0, 252.0, 110.0], fill: Some(PALE), line: None, round: 10.0 },
            Part::Words {
                r: [16.0, 10.0, 236.0, 100.0],
                t: Text { italic: true, align: "center", middle: true, ..text("\u{201c}Type a quote from the story here.\u{201d}", 15.0, A1) },
                fill: None,
            },
        ],
    },
    Block {
        name: "Sidebar: Tinted",
        size: (180.0, 252.0),
        parts: &[
            Part::Box { r: [0.0, 0.0, 180.0, 252.0], fill: Some(PALE), line: None, round: 0.0 },
            Part::Box { r: [0.0, 0.0, 180.0, 6.0], fill: Some(A1), line: None, round: 0.0 },
            Part::Words { r: [12.0, 16.0, 168.0, 46.0], t: heading("In This Issue", 15.0, A1), fill: None },
            Part::Words {
                r: [12.0, 50.0, 168.0, 240.0],
                t: text("Pastor\u{2019}s Corner\nUpcoming Events\nPrayer Requests\nBirthdays and Anniversaries\nVolunteer News", 10.5, MAIN),
                fill: None,
            },
        ],
    },
    Block {
        name: "Story: Heading and Two Columns",
        size: (432.0, 216.0),
        parts: &[
            Part::Words { r: [0.0, 0.0, 432.0, 32.0], t: heading("Story Heading", 20.0, A1), fill: None },
            Part::Words {
                r: [0.0, 38.0, 432.0, 216.0],
                t: Text {
                    columns: 2,
                    ..text(
                        "Type the story here. This text box has two columns, so the words flow from the bottom of the first column to the top of the second.\n\nTo replace this text, click in it and start typing. The font and colours follow the publication\u{2019}s scheme.",
                        10.5,
                        MAIN,
                    )
                },
                fill: None,
            },
        ],
    },
];

pub const CALENDARS: [Block; 4] = [
    Block {
        name: "This Month: Color Header",
        size: (468.0, 312.0),
        parts: &[Part::Calendar { r: [0.0, 0.0, 468.0, 312.0], next: false, banded: true }],
    },
    Block {
        name: "Next Month: Color Header",
        size: (468.0, 312.0),
        parts: &[Part::Calendar { r: [0.0, 0.0, 468.0, 312.0], next: true, banded: true }],
    },
    Block { name: "This Month: Simple", size: (468.0, 312.0), parts: &[Part::Calendar { r: [0.0, 0.0, 468.0, 312.0], next: false, banded: false }] },
    Block { name: "Next Month: Simple", size: (468.0, 312.0), parts: &[Part::Calendar { r: [0.0, 0.0, 468.0, 312.0], next: true, banded: false }] },
];

pub const BORDERS: [Block; 6] = [
    Block {
        name: "Border: Thin Line",
        size: (360.0, 240.0),
        parts: &[Part::Box { r: [0.0, 0.0, 360.0, 240.0], fill: None, line: Some((A1, 1.5, "solid")), round: 0.0 }],
    },
    Block {
        name: "Border: Double Line",
        size: (360.0, 240.0),
        parts: &[Part::Box { r: [0.0, 0.0, 360.0, 240.0], fill: None, line: Some((A1, 5.0, "thinThin")), round: 0.0 }],
    },
    Block {
        name: "Border: Rounded",
        size: (360.0, 240.0),
        parts: &[Part::Box { r: [0.0, 0.0, 360.0, 240.0], fill: None, line: Some((A1, 2.5, "solid")), round: 18.0 }],
    },
    Block {
        name: "Accent Bar: Solid",
        size: (432.0, 10.0),
        parts: &[Part::Box { r: [0.0, 0.0, 432.0, 10.0], fill: Some(A1), line: None, round: 0.0 }],
    },
    Block {
        name: "Accent Bar: Two Colors",
        size: (432.0, 14.0),
        parts: &[
            Part::Box { r: [0.0, 0.0, 432.0, 8.0], fill: Some(A1), line: None, round: 0.0 },
            Part::Box { r: [0.0, 10.0, 432.0, 14.0], fill: Some(A2), line: None, round: 0.0 },
        ],
    },
    Block {
        name: "Accent Bar: Dots",
        size: (432.0, 6.0),
        parts: &[Part::Line { a: [0.0, 3.0], b: [432.0, 3.0], ink: A1, weight: 4.0, kind: "dotted" }],
    },
];

pub const ADS: [Block; 4] = [
    Block {
        name: "Coupon",
        size: (288.0, 150.0),
        parts: &[
            Part::Box { r: [0.0, 0.0, 288.0, 150.0], fill: None, line: Some((MAIN, 1.5, "dashed")), round: 0.0 },
            Part::Words { r: [12.0, 12.0, 276.0, 36.0], t: Text { align: "center", ..heading("Name of Item or Service", 15.0, MAIN) }, fill: None },
            Part::Words {
                r: [12.0, 40.0, 276.0, 88.0],
                t: Text { align: "center", bold: true, middle: true, ..text("10% OFF", 32.0, A1) },
                fill: None,
            },
            Part::Words {
                r: [12.0, 94.0, 276.0, 138.0],
                t: Text { align: "center", ..text("Describe the offer here.\nExpires: date", 9.5, MAIN) },
                fill: None,
            },
        ],
    },
    Block {
        name: "Ad: Event",
        size: (288.0, 180.0),
        parts: &[
            Part::Box { r: [0.0, 0.0, 288.0, 180.0], fill: Some(PALE), line: Some((A1, 2.0, "solid")), round: 0.0 },
            Part::Words { r: [14.0, 14.0, 274.0, 44.0], t: heading("Event Title", 20.0, A1), fill: None },
            Part::Words { r: [14.0, 48.0, 274.0, 68.0], t: Text { bold: true, ..text("Date \u{00b7} Time \u{00b7} Place", 11.0, MAIN) }, fill: None },
            Part::Words {
                r: [14.0, 74.0, 274.0, 166.0],
                t: text("Add a sentence or two about the event, and who to call to find out more.", 10.5, MAIN),
                fill: None,
            },
        ],
    },
    Block {
        name: "Attention Getter: Circle",
        size: (110.0, 110.0),
        parts: &[
            Part::Circle { r: [0.0, 0.0, 110.0, 110.0], fill: A1 },
            Part::Words {
                r: [10.0, 10.0, 100.0, 100.0],
                t: Text { align: "center", middle: true, ..heading("Join Us!", 17.0, Ink::Paper) },
                fill: None,
            },
        ],
    },
    Block {
        name: "Attention Getter: Starburst",
        size: (120.0, 120.0),
        parts: &[
            Part::Star { r: [0.0, 0.0, 120.0, 120.0], fill: A2, points: 16 },
            Part::Words { r: [20.0, 20.0, 100.0, 100.0], t: Text { align: "center", middle: true, ..heading("New!", 20.0, A3) }, fill: None },
        ],
    },
];

// ───────────────────────────── calendars ─────────────────────────────

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Today's year and month (1–12), from the clock.
fn this_month() -> (i64, u32) {
    let secs = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (y, m, _) = civil(secs.div_euclid(86_400));
    (y, m)
}

/// The date `days` after 1970-01-01 (Howard Hinnant's civil-from-days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Days from 1970-01-01 to `y`-`m`-`d`.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// A month's layout: its title, the weekday its 1st falls on (0 = Sunday), its length and weeks.
pub fn month(next: bool) -> (String, u32, u32, u32) {
    let (y, m) = this_month();
    let (y, m) = if next { if m == 12 { (y + 1, 1) } else { (y, m + 1) } } else { (y, m) };
    let first = days_from_civil(y, m, 1);
    let following = if m == 12 { days_from_civil(y + 1, 1, 1) } else { days_from_civil(y, m + 1, 1) };
    let len = (following - first) as u32;
    let wd = (first + 4).rem_euclid(7) as u32;
    let weeks = (wd + len).div_ceil(7);
    let name = MONTHS.get(m as usize - 1).copied().unwrap_or("Month");
    (format!("{name} {y}"), wd, len, weeks)
}

// ───────────────────────────── building ─────────────────────────────

/// A stroke type as `object.stroke` takes it: a dashed line needs its dashes and gaps.
fn stroke_type(kind: &str, weight: f64) -> Value {
    match kind {
        "dashed" => json!({"kind": "dashed", "pattern": [weight * 4.0, weight * 2.5]}),
        k => json!({"kind": k}),
    }
}

/// Script steps, with the steps whose objects make up the block (to group).
struct Script {
    steps: Vec<Value>,
    made: Vec<usize>,
}

impl Script {
    fn push(&mut self, command: &str, params: Value) -> usize {
        self.steps.push(json!({"command": command, "params": params}));
        self.steps.len() - 1
    }

    fn select(&mut self, step: usize) {
        self.push("selection.set", json!({"ids": [format!("${step}.id")]}));
    }

    fn paint(&mut self, step: usize, fill: Option<Ink>, line: Option<(Ink, f64, &str)>) {
        self.select(step);
        match fill {
            Some(ink) => {
                let (s, t) = ink.swatch();
                self.push("object.fill", json!({"swatch": s, "tint": t}));
            }
            None => {
                self.push("object.fill", json!({"swatch": "[None]"}));
            }
        }
        match line {
            Some((ink, w, kind)) => {
                let (s, t) = ink.swatch();
                self.push("object.stroke", json!({"swatch": s, "tint": t, "weight": w, "type": stroke_type(kind, w)}));
            }
            None => {
                self.push("object.stroke", json!({"swatch": "[None]"}));
            }
        }
    }

    fn words(&mut self, spread: usize, r: [f64; 4], t: &Text, fill: Option<Ink>, k: f64) {
        let f = self.push("frame.create", json!({"spread": spread, "rect": r, "content": "text", "text": t.text}));
        self.made.push(f);
        self.select(f);
        if t.heading {
            self.push("style.paragraph.apply", json!({"name": scheme::HEADING, "clearOverrides": false}));
        }
        let (s, tint) = t.ink.swatch();
        let mut attrs = json!({"size": (t.size * k * 10.0).round() / 10.0, "fill": s, "fillTint": tint});
        let style = match (t.bold, t.italic) {
            (true, true) => Some("Bold Italic"),
            (true, false) => Some("Bold"),
            (false, true) => Some("Italic"),
            (false, false) => None,
        };
        if let Some(st) = style {
            attrs["fontStyle"] = json!(st);
        }
        self.push("type.char", json!({"attrs": attrs}));
        if t.align != "left" {
            self.push("type.para", json!({"attrs": {"align": t.align}}));
        }
        let mut opts = json!({"inset": if fill.is_some() { 8.0 * k } else { 0.0 }});
        if t.middle {
            opts["verticalJustification"] = json!("center");
        }
        if t.columns > 1 {
            opts["columns"] = json!(t.columns);
            opts["gutter"] = json!(14.0 * k);
        }
        self.push("object.textFrameOptions", opts);
        if let Some(ink) = fill {
            let (s, t) = ink.swatch();
            self.push("object.fill", json!({"swatch": s, "tint": t}));
        }
    }

    fn calendar(&mut self, spread: usize, r: [f64; 4], next: bool, banded: bool, k: f64) {
        let (title, wd, len, weeks) = month(next);
        let title_h = 34.0 * k;
        let title_ink = if banded { A1 } else { MAIN };
        // The title, then the table in a box of its own.
        let f = self.push("frame.create", json!({"spread": spread, "rect": [r[0], r[1], r[2], r[1] + title_h], "content": "text", "text": title}));
        self.made.push(f);
        self.select(f);
        self.push("style.paragraph.apply", json!({"name": scheme::HEADING, "clearOverrides": false}));
        let (s, tint) = title_ink.swatch();
        self.push("type.char", json!({"attrs": {"size": (22.0 * k).round(), "fill": s, "fillTint": tint}}));
        self.push("type.para", json!({"attrs": {"align": "center"}}));
        self.push("object.textFrameOptions", json!({"inset": 0}));
        let width = r[2] - r[0];
        let tf =
            self.push("frame.create", json!({"spread": spread, "rect": [r[0], r[1] + title_h + 4.0, r[2], r[3]], "content": "text", "caret": true}));
        self.made.push(tf);
        self.push("table.insert", json!({"rows": weeks, "cols": 7, "headerRows": 1, "width": width}));
        let line = if banded { A1 } else { Ink::Scheme(0, 0.45) };
        let (ls, lt) = line.swatch();
        let all = json!({"rows": [0, weeks], "cols": [0, 6]});
        let mut stroke = all.clone();
        stroke["stroke"] = json!({"weight": 0.75, "color": ls, "tint": lt});
        stroke["vj"] = json!("top");
        stroke["insets"] = json!(3.0 * k);
        self.push("table.setCell", stroke);
        for (c, d) in DAYS.iter().enumerate() {
            let mut cell = json!({"row": 0, "col": c, "text": d});
            if banded {
                cell["fill"] = json!(ls);
            }
            self.push("table.setCell", cell);
        }
        // The header row: bold, centred, white on the colour band.
        self.push("table.select", json!({"rows": [0, 0], "cols": [0, 6]}));
        let head_ink = if banded { Ink::Paper } else { A1 };
        let (hs, ht) = head_ink.swatch();
        self.push("type.char", json!({"attrs": {"fontStyle": "Bold", "size": (9.5 * k * 10.0).round() / 10.0, "fill": hs, "fillTint": ht}}));
        self.push("type.para", json!({"attrs": {"align": "center"}}));
        for day in 1..=len {
            let at = wd + day - 1;
            self.push("table.setCell", json!({"row": 1 + at / 7, "col": at % 7, "text": day.to_string()}));
        }
        self.push("table.select", json!({"rows": [1, weeks], "cols": [0, 6]}));
        self.push("type.char", json!({"attrs": {"size": (9.0 * k * 10.0).round() / 10.0}}));
        let body_h = ((r[3] - r[1] - title_h - 4.0) - 18.0 * k) / f64::from(weeks.max(1));
        self.push("table.setRowHeight", json!({"height": body_h.max(14.0), "mode": "atLeast", "rows": [1, weeks], "cols": [0, 6]}));
    }
}

/// Builds `b` in the middle of the page in view, grouped, as one undo step.
pub fn insert(app: &mut DesignApp, b: &Block) -> Result<Value, String> {
    let (spread, centre) = super::centre_rect(app, 0.0, 0.0).ok_or("Open a publication first.")?;
    let page_w = app
        .session
        .active()
        .and_then(|st| {
            let abs = crate::canvas::current_page(app).unwrap_or(0);
            let (si, pi) = st.doc.page_loc(abs)?;
            st.doc.spreads.get(si)?.pages.get(pi).map(|p| (p.width, p.height))
        })
        .unwrap_or((612.0, 792.0));
    // Smaller pages (a half-letter bulletin) get a smaller block.
    let k = (page_w.0 * 0.86 / b.size.0).min(page_w.1 * 0.86 / b.size.1).min(1.0);
    let (w, h) = (b.size.0 * k, b.size.1 * k);
    let (x0, y0) = (centre[0] - w / 2.0, centre[1] - h / 2.0);
    let at = |r: [f64; 4]| [x0 + r[0] * k, y0 + r[1] * k, x0 + r[2] * k, y0 + r[3] * k];
    let mut s = Script { steps: scheme::ensure_steps(app), made: Vec::new() };
    for part in b.parts {
        match *part {
            Part::Box { r, fill, line, round } => {
                let f = s.push("frame.create", json!({"spread": spread, "rect": at(r), "content": "unassigned"}));
                s.made.push(f);
                s.paint(f, fill, line.map(|(i, w, kind)| (i, w * k.max(0.5), kind)));
                if round > 0.0 {
                    s.push("object.cornerOptions", json!({"shape": "rounded", "size": round * k}));
                }
            }
            Part::Circle { r, fill } => {
                let f = s.push("frame.create", json!({"spread": spread, "rect": at(r), "shape": "ellipse", "content": "unassigned"}));
                s.made.push(f);
                s.paint(f, Some(fill), None);
            }
            Part::Star { r, fill, points } => {
                let f = s.push(
                    "frame.create",
                    json!({"spread": spread, "rect": at(r), "shape": "polygon", "sides": points, "starInset": 22, "content": "unassigned"}),
                );
                s.made.push(f);
                s.paint(f, Some(fill), None);
            }
            Part::Line { a, b: e, ink, weight, kind } => {
                let r = at([a[0], a[1], e[0], e[1]]);
                let f = s.push("line.create", json!({"spread": spread, "a": [r[0], r[1]], "b": [r[2], r[3]]}));
                s.made.push(f);
                s.select(f);
                let (sw, t) = ink.swatch();
                let w = weight * k.max(0.5);
                s.push("object.stroke", json!({"swatch": sw, "tint": t, "weight": w, "type": stroke_type(kind, w)}));
            }
            Part::Words { r, t, fill } => s.words(spread, at(r), &t, fill, k),
            Part::Calendar { r, next, banded } => s.calendar(spread, at(r), next, banded, k),
        }
    }
    if s.made.len() > 1 {
        let ids: Vec<String> = s.made.iter().map(|i| format!("${i}.id")).collect();
        s.push("object.group", json!({"ids": ids}));
    } else if let Some(f) = s.made.first().copied() {
        s.select(f);
    }
    let r = app.run("script.run", json!({"text": Value::Array(s.steps).to_string()}));
    if r.is_ok() {
        let _ = app.run("tool.select", json!({"tool": "selection"}));
        app.status(format!("Inserted {}. Click its text to type over it.", label(b)));
    }
    r
}

// ───────────────────────────── galleries ─────────────────────────────

/// The publication's scheme colours, for the pictures (Harbor's when it has none).
fn colors(app: &DesignApp) -> [Color32; 6] {
    let harbor = &scheme::COLOR_SCHEMES[0];
    let mut out = [Color32::BLACK; 6];
    for (i, c) in out.iter_mut().enumerate() {
        let from_doc = app.session.active().and_then(|st| {
            let name = scheme::SWATCHES.get(i)?;
            let rgb = st.doc.resolve_color(name, 1.0)?.to_rgb();
            let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            Some(Color32::from_rgb(b(rgb[0]), b(rgb[1]), b(rgb[2])))
        });
        *c = from_doc.unwrap_or_else(|| harbor.colors.get(i).map_or(Color32::BLACK, |h| scheme::hex_color(h)));
    }
    out
}

/// The block's picture in `r`.
fn picture(ui: &Ui, b: &Block, r: Rect, cols: &[Color32; 6]) {
    let p = ui.painter();
    p.rect_filled(r, 2.0, Color32::WHITE);
    let k = ((r.width() - 8.0) / b.size.0 as f32).min((r.height() - 8.0) / b.size.1 as f32);
    let o = r.center() - vec2(b.size.0 as f32 * k / 2.0, b.size.1 as f32 * k / 2.0);
    let at = |x: f64, y: f64| Pos2::new(o.x + x as f32 * k, o.y + y as f32 * k);
    let rect = |q: [f64; 4]| Rect::from_min_max(at(q[0], q[1]), at(q[2], q[3]));
    for part in b.parts {
        match *part {
            Part::Box { r: q, fill, line, round } => {
                let rr = (round as f32 * k).min(8.0);
                if let Some(f) = fill {
                    p.rect_filled(rect(q), rr, f.shown(cols));
                }
                if let Some((ink, w, _)) = line {
                    p.rect_stroke(rect(q), rr, Stroke::new((w as f32 * k).max(1.0), ink.shown(cols)), egui::StrokeKind::Inside);
                }
            }
            Part::Circle { r: q, fill } => {
                let rr = rect(q);
                p.circle_filled(rr.center(), rr.width().min(rr.height()) / 2.0, fill.shown(cols));
            }
            Part::Star { r: q, fill, points } => {
                let rr = rect(q);
                let n = (points.max(3) * 2) as usize;
                let pts: Vec<Pos2> = (0..n)
                    .map(|i| {
                        let a = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                        let rad = if i % 2 == 0 { rr.width() / 2.0 } else { rr.width() / 2.0 * 0.78 };
                        rr.center() + vec2(a.cos() * rad, a.sin() * rad)
                    })
                    .collect();
                p.add(egui::Shape::convex_polygon(pts, fill.shown(cols), Stroke::NONE));
            }
            Part::Line { a, b: e, ink, weight, .. } => {
                p.line_segment([at(a[0], a[1]), at(e[0], e[1])], Stroke::new((weight as f32 * k).max(1.0), ink.shown(cols)));
            }
            Part::Words { r: q, t, fill } => {
                let rr = rect(q);
                if let Some(f) = fill {
                    p.rect_filled(rr, 0.0, f.shown(cols));
                }
                let size = (t.size as f32 * k).max(3.0);
                if size >= 6.0 {
                    let first = t.text.lines().next().unwrap_or("");
                    let align = match t.align {
                        "center" => egui::Align2::CENTER_CENTER,
                        _ => egui::Align2::LEFT_CENTER,
                    };
                    let y = if t.middle { rr.center().y } else { rr.min.y + size * 0.7 };
                    let x = if t.align == "center" { rr.center().x } else { rr.min.x + if fill.is_some() { 3.0 } else { 0.0 } };
                    p.with_clip_rect(rr).text(pos2(x, y), align, first, FontId::proportional(size), t.ink.shown(cols));
                } else {
                    // Text too small to read: lines in its colour where it goes.
                    let ink = t.ink.shown(cols).gamma_multiply(0.75);
                    let mut y = rr.min.y + 2.0;
                    while y < rr.max.y - 1.0 {
                        p.line_segment([pos2(rr.min.x, y), pos2(rr.max.x - (y * 7.0) % 9.0, y)], Stroke::new(1.0, ink));
                        y += 4.0;
                    }
                }
            }
            Part::Calendar { r: q, banded, .. } => {
                let rr = rect(q);
                let title = Rect::from_min_size(rr.min, vec2(rr.width(), rr.height() * 0.12));
                p.rect_filled(Rect::from_center_size(title.center(), vec2(rr.width() * 0.45, 3.0)), 1.0, if banded { cols[1] } else { cols[0] });
                let grid = Rect::from_min_max(pos2(rr.min.x, title.max.y + 2.0), rr.max);
                let rows = 6;
                let (cw, rh) = (grid.width() / 7.0, grid.height() / rows as f32);
                if banded {
                    p.rect_filled(Rect::from_min_size(grid.min, vec2(grid.width(), rh)), 0.0, cols[1]);
                }
                let line = if banded { cols[1] } else { Color32::from_gray(150) };
                for i in 0..=7 {
                    let x = grid.min.x + cw * i as f32;
                    p.line_segment([pos2(x, grid.min.y), pos2(x, grid.max.y)], Stroke::new(0.7, line));
                }
                for j in 0..=rows {
                    let y = grid.min.y + rh * j as f32;
                    p.line_segment([pos2(grid.min.x, y), pos2(grid.max.x, y)], Stroke::new(0.7, line));
                }
            }
        }
    }
}

/// The name under a block's picture: a calendar's month and look, else the part after the colon.
fn label(b: &Block) -> String {
    match b.parts.first() {
        Some(Part::Calendar { next, banded, .. }) => {
            let (title, ..) = month(*next);
            let name = title.split_whitespace().next().unwrap_or("").to_string();
            format!("{name} \u{00b7} {}", if *banded { "Color" } else { "Simple" })
        }
        _ => b.name.split_once(": ").map_or(b.name, |(_, n)| n).to_string(),
    }
}

/// A Building Blocks menu: the blocks as pictures with their names.
pub(super) fn gallery(app: &mut DesignApp, ui: &mut Ui, title: &str, blocks: &'static [Block]) {
    ui.label(egui::RichText::new(title).strong().size(12.0));
    ui.add_space(4.0);
    let cols = colors(app);
    let pal = super::pal();
    let mut pick = None;
    egui::Grid::new(("pr_blocks", title)).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
        for (i, b) in blocks.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(124.0, 104.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 4.0, pal.hover);
                ui.painter().rect_stroke(r.shrink(0.5), 4.0, Stroke::new(1.0, pal.hover_border), egui::StrokeKind::Inside);
            }
            let pic = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(112.0, 72.0));
            picture(ui, b, pic, &cols);
            ui.painter().rect_stroke(pic, 2.0, Stroke::new(1.0, pal.card_edge), egui::StrokeKind::Outside);
            let label = label(b);
            ui.painter().text(pos2(r.center().x, r.max.y - 13.0), egui::Align2::CENTER_CENTER, label, FontId::proportional(11.0), pal.text);
            if resp.on_hover_text(b.name).clicked() {
                pick = Some(i);
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
    if let Some(b) = pick.and_then(|i| blocks.get(i)) {
        ui.close();
        if let Err(e) = insert(app, b) {
            app.status(format!("Couldn't insert {}: {e}", b.name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip_and_months_lay_out() {
        for days in [-1000, 0, 365, 11_016, 20_756, 30_000] {
            let (y, m, d) = civil(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil(0), (1970, 1, 1));
        // 1 November 2026 is a Sunday; 1 February 2026 too, and February 2026 has 28 days.
        let nov = days_from_civil(2026, 11, 1);
        assert_eq!((nov + 4).rem_euclid(7), 0);
        assert_eq!(days_from_civil(2026, 3, 1) - days_from_civil(2026, 2, 1), 28);
        assert_eq!(days_from_civil(2028, 3, 1) - days_from_civil(2028, 2, 1), 29);
        let (title, wd, len, weeks) = month(false);
        assert!(title.contains(' ') && wd < 7 && (28..=31).contains(&len) && (4..=6).contains(&weeks));
    }

    #[test]
    fn every_block_fits_inside_its_own_size() {
        for b in PAGE_PARTS.iter().chain(&CALENDARS).chain(&BORDERS).chain(&ADS) {
            let inside =
                |r: [f64; 4]| r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= b.size.0 + 1e-9 && r[3] <= b.size.1 + 1e-9 && r[0] <= r[2] && r[1] <= r[3];
            for p in b.parts {
                let ok = match *p {
                    Part::Box { r, .. } | Part::Circle { r, .. } | Part::Star { r, .. } | Part::Words { r, .. } | Part::Calendar { r, .. } => {
                        inside(r)
                    }
                    Part::Line { a, b: e, .. } => inside([a[0].min(e[0]), a[1].min(e[1]), a[0].max(e[0]), a[1].max(e[1])]),
                };
                assert!(ok, "{}", b.name);
            }
        }
    }
}

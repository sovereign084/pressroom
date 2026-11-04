//! Turns a publication into PressRoom's command steps: the same list `tools/pub-import/pub2pressroom.ps1`
//! builds from what Publisher reports, so one list of commands makes the document whichever way the
//! file was read, and the two lists can be compared.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use designcraft_color::cms::{Cms, Intent};
use serde_json::{Value, json};

use crate::contents::TableInfo;
use crate::model::{self, Placed, Publication, Rgb, Text};
use crate::quill::{CharProps, ParaProps, Spacing};

/// Most cells a table may have.
const MAX_TABLE_CELLS: usize = 100_000;
/// A cell's margin where it doesn't say, in points.
const DEFAULT_CELL_MARGIN: f64 = 2.85;

/// A press recipe, percentages, that PressRoom draws as `rgb`: a colour-matching ink's screen colour
/// is all the file keeps of it. The sum done by hand came out visibly bluer than the ink.
fn ink_recipe(rgb: Rgb) -> [f64; 4] {
    let srgb = [f32::from(rgb.0) / 255.0, f32::from(rgb.1) / 255.0, f32::from(rgb.2) / 255.0];
    Cms::default().srgb_to_cmyk(srgb, Intent::RelativeColorimetric).map(|v| (f64::from(v) * 1000.0).round() / 10.0)
}

/// What a font name means on this computer.
pub struct FontChoice {
    pub family: String,
    pub style: String,
    /// The font's single line spacing as a multiple of its size.
    pub line_factor: f64,
}

/// Maps a Publisher font name with bold and italic to PressRoom's family and style.
pub trait Fonts {
    fn choose(&self, name: &str, bold: bool, italic: bool) -> FontChoice;
}

/// The name as it stands, for tests and when nothing better is known.
pub struct PlainFonts;

impl Fonts for PlainFonts {
    fn choose(&self, name: &str, bold: bool, italic: bool) -> FontChoice {
        FontChoice { family: name.to_string(), style: style_name(bold, italic).to_string(), line_factor: 1.2 }
    }
}

pub fn style_name(bold: bool, italic: bool) -> &'static str {
    match (bold, italic) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (false, false) => "Regular",
    }
}

/// How the shapes came through, for the log and the person reading it.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub text: usize,
    pub linked: usize,
    pub pictures: usize,
    pub shapes: usize,
    pub lines: usize,
    pub tables: usize,
    pub mixed_cells: usize,
    pub left_out: usize,
}

pub struct Output {
    pub steps: Vec<Value>,
    pub stats: Stats,
    /// What could not be brought across, in words for a person.
    pub notes: Vec<String>,
    /// Picture files written, for removing once they are inside the publication.
    pub files: Vec<PathBuf>,
}

struct Emit<'a> {
    p: &'a Publication,
    fonts: &'a dyn Fonts,
    folder: &'a Path,
    steps: Vec<Value>,
    stats: Stats,
    notes: Vec<String>,
    files: Vec<PathBuf>,
    swatches: BTreeMap<String, String>,
    /// Story id → each frame in order: its step, and a linked frame's shape for its options.
    chains: BTreeMap<u32, BTreeMap<u32, (usize, Option<Placed>)>>,
    pictures: usize,
}

/// Publisher's stored alignment numbers → PressRoom's.
fn align_name(v: u32) -> &'static str {
    match v {
        1 => "right",
        2 => "center",
        3 | 6 => "leftJustified",
        4 => "fullyJustified",
        _ => "left",
    }
}

/// Office's arrowheads → PressRoom's line ends.
fn arrow_name(style: u32) -> Option<&'static str> {
    match style {
        1 => Some("triangle"),
        2 | 5 => Some("barbed"),
        3 => Some("square"),
        4 => Some("circleSolid"),
        _ => None,
    }
}

/// The corners of a common shape, as fractions of its box, in its standard proportions.
fn outline_of(kind: u16) -> Option<Vec<(f64, f64)>> {
    let v: &[(f64, f64)] = match kind {
        4 => &[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)],
        5 => &[(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)],
        6 => &[(0.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        7 => &[(0.25, 0.0), (1.0, 0.0), (0.75, 1.0), (0.0, 1.0)],
        8 => &[(0.25, 0.0), (0.75, 0.0), (1.0, 1.0), (0.0, 1.0)],
        9 => &[(0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5)],
        10 => &[(0.29, 0.0), (0.71, 0.0), (1.0, 0.29), (1.0, 0.71), (0.71, 1.0), (0.29, 1.0), (0.0, 0.71), (0.0, 0.29)],
        11 => &[
            (0.25, 0.0),
            (0.75, 0.0),
            (0.75, 0.25),
            (1.0, 0.25),
            (1.0, 0.75),
            (0.75, 0.75),
            (0.75, 1.0),
            (0.25, 1.0),
            (0.25, 0.75),
            (0.0, 0.75),
            (0.0, 0.25),
            (0.25, 0.25),
        ],
        12 => &[
            (0.5, 0.0),
            (0.618, 0.382),
            (1.0, 0.382),
            (0.691, 0.618),
            (0.809, 1.0),
            (0.5, 0.764),
            (0.191, 1.0),
            (0.309, 0.618),
            (0.0, 0.382),
            (0.382, 0.382),
        ],
        13 => &[(0.0, 0.25), (0.75, 0.25), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.75, 0.75), (0.0, 0.75)],
        15 => &[(0.0, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.0, 1.0)],
        55 => &[(0.0, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.0, 1.0), (0.25, 0.5)],
        56 => &[(0.5, 0.0), (1.0, 0.38), (0.81, 1.0), (0.19, 1.0), (0.0, 0.38)],
        66 => &[(1.0, 0.25), (0.25, 0.25), (0.25, 0.0), (0.0, 0.5), (0.25, 1.0), (0.25, 0.75), (1.0, 0.75)],
        67 => &[(0.25, 0.0), (0.75, 0.0), (0.75, 0.75), (1.0, 0.75), (0.5, 1.0), (0.0, 0.75), (0.25, 0.75)],
        68 => &[(0.25, 1.0), (0.75, 1.0), (0.75, 0.25), (1.0, 0.25), (0.5, 0.0), (0.0, 0.25), (0.25, 0.25)],
        _ => return None,
    };
    Some(v.to_vec())
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn r#ref(step: usize, path: &str) -> String {
    format!("${step}.{path}")
}

/// A story's text for PressRoom, and each UTF-16 unit's byte offset in it (one past the end too).
fn convert(units: &[u16]) -> (String, Vec<usize>) {
    let mut out = String::new();
    let mut offsets = Vec::with_capacity(units.len() + 1);
    let mut i = 0;
    while i < units.len() {
        offsets.push(out.len());
        let u = units.get(i).copied().unwrap_or(0);
        let next = units.get(i + 1).copied();
        let c = match u {
            0x0D => {
                out.push('\n');
                i += 1;
                continue;
            }
            0x0B => '\u{2028}',
            0xD800..=0xDBFF => match next {
                Some(low @ 0xDC00..=0xDFFF) => {
                    let code = 0x10000 + ((u32::from(u) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                    offsets.push(out.len());
                    i += 1;
                    char::from_u32(code).unwrap_or('\u{FFFD}')
                }
                _ => '\u{FFFD}',
            },
            _ => char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'),
        };
        out.push(c);
        i += 1;
    }
    offsets.push(out.len());
    (out, offsets)
}

impl<'a> Emit<'a> {
    fn add(&mut self, command: &str, params: Value) -> usize {
        self.steps.push(json!({"command": command, "params": params}));
        self.steps.len() - 1
    }

    fn rect(s: &Placed, x: f64) -> Value {
        json!([s.rect[0] + x, s.rect[1], s.rect[2] + x, s.rect[3]])
    }

    /// Text wrap: square and tight wrap around the object's box.
    fn emit_wrap(&mut self, s: &Placed, k: usize) {
        if s.wrap {
            self.add("object.textWrap", json!({"mode": "boundingBox", "offset": 2.88, "ids": [r#ref(k, "id")]}));
        }
    }

    fn emit_rotation(&mut self, s: &Placed, k: usize) {
        if s.rotation != 0.0 {
            self.add("transform.rotate", json!({"angle": -s.rotation, "ids": [r#ref(k, "id")]}));
        }
    }

    /// A colour-matching ink's swatch, made once. The file keeps the ink's name and its colour on
    /// screen, not its recipe for the press, so the recipe is worked out from the screen colour.
    fn ink_swatch(&mut self, name: &str, rgb: Rgb) -> String {
        if !self.swatches.contains_key(name) {
            let [c, m, y, k] = ink_recipe(rgb);
            self.add("swatch.create", json!({"name": name, "spot": true, "color": {"c": c, "m": m, "y": y, "k": k}}));
            self.notes.push(format!(
                "{name} came across as a named ink. Its press recipe isn't in the file, so its colour is worked out from how it looks on screen."
            ));
            self.swatches.insert(name.to_string(), name.to_string());
        }
        name.to_string()
    }

    fn emit_box_style(&mut self, s: &Placed, k: usize) {
        if let Some(fill) = s.fill {
            if s.bevel {
                // PressRoom draws no bevels, so the face takes about the shade Publisher draws it in.
                let shade = |c: u8| (f64::from(c) * 0.756).round() as u8;
                let face = Rgb(shade(fill.0), shade(fill.1), shade(fill.2));
                self.notes.push("A shape's 3-D bevel was left flat, in about the shade Publisher gives its face.".to_string());
                self.add("object.color", json!({"color": hex(face), "target": "fill", "ids": [r#ref(k, "id")]}));
            } else if let Some(name) = &s.fill_ink {
                let swatch = self.ink_swatch(name, fill);
                self.add("object.fill", json!({"swatch": swatch, "ids": [r#ref(k, "id")]}));
            } else {
                self.add("object.color", json!({"color": hex(fill), "target": "fill", "ids": [r#ref(k, "id")]}));
            }
        }
        match s.line {
            Some((weight, color)) if weight > 0.0 => {
                self.add("object.stroke", json!({"weight": weight, "ids": [r#ref(k, "id")]}));
                self.add("object.color", json!({"color": hex(color), "target": "stroke", "ids": [r#ref(k, "id")]}));
            }
            _ => {
                self.add("object.stroke", json!({"swatch": "[None]", "ids": [r#ref(k, "id")]}));
            }
        }
    }

    fn emit_corners(&mut self, s: &Placed, k: usize) {
        if s.kind != 2 {
            return;
        }
        let adjust = s.adjust.map_or(3600.0, f64::from) / 21600.0;
        let short = (s.rect[2] - s.rect[0]).abs().min((s.rect[3] - s.rect[1]).abs());
        let size = (adjust * short * 100.0).round() / 100.0;
        if size > 0.0 {
            self.add("object.cornerOptions", json!({"shape": "rounded", "size": size, "ids": [r#ref(k, "id")]}));
        }
    }

    fn emit_frame_options(&mut self, s: &Placed, k: usize) {
        let valign = match s.valign {
            1 => "center",
            2 => "bottom",
            _ => "top",
        };
        let mut o = json!({"inset": s.insets, "verticalJustification": valign, "ids": [r#ref(k, "id")]});
        if s.columns > 1 {
            o["columns"] = json!(s.columns);
            o["gutter"] = json!(s.gutter);
        }
        self.add("object.textFrameOptions", o);
    }

    fn para_attrs(pf: &ParaProps) -> Value {
        let mut a = json!({
            "align": align_name(pf.align.unwrap_or(0)),
            "composer": "singleLine",
            "wordSpaceMin": 1.0,
            "hyphZone": 18.0,
            "spaceBefore": f64::from(pf.before.unwrap_or(0)) / model::EMU_PER_POINT,
            "spaceAfter": f64::from(pf.after.unwrap_or(0)) / model::EMU_PER_POINT,
            "leftIndent": f64::from(pf.left_indent.unwrap_or(0)) / model::EMU_PER_POINT,
            "firstLineIndent": f64::from(pf.first_indent.unwrap_or(0)) / model::EMU_PER_POINT,
        });
        // Line and paragraph breaks: Publisher's keep options and widow control.
        let (mut first, mut last) = if pf.widow == Some(true) { (2, 2) } else { (0, 0) };
        if pf.keep_next == Some(true) {
            a["keepWithNext"] = json!(1);
        }
        if pf.keep_lines == Some(true) {
            a["keepLinesTogether"] = json!(true);
            a["keepAllLines"] = json!(true);
        }
        if let Some(lines) = pf.drop_lines.filter(|&l| l > 1) {
            let chars = pf.drop_chars.unwrap_or(1).max(1);
            a["dropCapLines"] = json!(lines);
            a["dropCapChars"] = json!(chars);
            // Publisher never starts a drop cap where its lines don't fit.
            first = first.max(lines);
            last = last.max(1);
        }
        if first > 0 && pf.keep_lines != Some(true) {
            a["keepLinesTogether"] = json!(true);
            a["keepAllLines"] = json!(false);
            a["keepFirst"] = json!(first);
            a["keepLast"] = json!(last);
        }
        a
    }

    /// Character formatting `f` on what is selected, with the paragraph's line spacing.
    fn emit_format(&mut self, f: &CharProps, spacing: Option<Spacing>) {
        let name = f.font().and_then(|i| self.p.fonts.get(i as usize)).map_or("Calibri", String::as_str);
        let bold = f.bold.unwrap_or(false);
        let italic = f.italic.unwrap_or(false);
        let choice = self.fonts.choose(name, bold, italic);
        let size = f64::from(f.size.unwrap_or(127_000)) / model::EMU_PER_POINT;
        let lead = match spacing {
            Some(Spacing::Points(p)) => p,
            Some(Spacing::Lines(l)) => size * choice.line_factor * l,
            None => size * choice.line_factor,
        };
        let mut attrs = json!({
            "fontFamily": choice.family,
            "size": size,
            "underline": f.underline.is_some_and(|u| u != 0),
            "leading": {"kind": "points", "value": (lead * 100.0).round() / 100.0},
        });
        // Character spacing: Publisher's tracking (percent) and kerning (points between letters).
        let tracking =
            f.tracking.map_or(0.0, |t| f64::from(t) - 1000.0) + f.kerning.map_or(0.0, |k| f64::from(k) / model::EMU_PER_POINT) / size * 1000.0;
        if tracking.round() != 0.0 {
            attrs["tracking"] = json!(tracking.round());
        }
        if let Some(sc) = f.scaling.filter(|&s| s != 1000) {
            attrs["hScale"] = json!(f64::from(sc) / 1000.0);
        }
        if f.caps == Some(true) {
            attrs["capitalization"] = json!("allCaps");
        } else if f.small_caps == Some(true) {
            attrs["capitalization"] = json!("smallCaps");
        }
        self.add("type.char", json!({"attrs": attrs}));
        self.add("type.char", json!({"attrs": {"fontStyle": choice.style}}));
        if let Some(index) = f.color {
            let c = self.p.colors.get(index as usize).copied().unwrap_or(Rgb(0, 0, 0));
            if let Some(name) = self.p.color_inks.get(&index) {
                let swatch = self.ink_swatch(&name.clone(), c);
                self.add("type.char", json!({"attrs": {"fill": swatch}}));
            } else if c != Rgb(0, 0, 0) {
                self.add("object.color", json!({"color": hex(c), "text": true}));
            }
        }
    }

    /// A drop cap's own font and colour, on its letters.
    fn emit_drop_cap(&mut self, para: &crate::quill::Run<ParaProps>, k: usize, offsets: &[usize], len: usize) {
        if para.props.drop_lines.is_none_or(|l| l < 2) {
            return;
        }
        let chars = para.props.drop_chars.unwrap_or(1).max(1) as usize;
        let (a, b) = (offsets.get(para.start.min(len)).copied().unwrap_or(0), offsets.get((para.start + chars).min(len)).copied().unwrap_or(0));
        let Some(name) = para.props.drop_font.and_then(|i| self.p.fonts.get(i as usize)).filter(|_| b > a) else { return };
        let choice = self.fonts.choose(name, false, false);
        self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": a, "focus": b}));
        self.add("type.char", json!({"attrs": {"fontFamily": choice.family}}));
        self.add("type.char", json!({"attrs": {"fontStyle": choice.style}}));
        if let Some(c) = para.props.drop_color.and_then(|i| self.p.colors.get(i as usize)).copied() {
            self.add("object.color", json!({"color": hex(c), "text": true}));
        }
    }

    fn emit_story_formats(&mut self, t: &Text, k: usize, offsets: &[usize], len: usize) {
        let at = |u: usize| offsets.get(u.min(len)).copied().unwrap_or(0);
        for para in &t.paragraphs {
            if para.start >= len {
                break;
            }
            let (a, b) = (at(para.start), at(para.end.min(len)));
            self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": a, "focus": b}));
            self.add("type.para", json!({"attrs": Self::para_attrs(&para.props)}));
            let spacing = para.props.spacing();
            let mut merged: Vec<(usize, usize, &CharProps)> = Vec::new();
            for r in &t.runs {
                let (s, e) = (r.start.max(para.start), r.end.min(para.end).min(len));
                if s >= e {
                    continue;
                }
                match merged.last_mut() {
                    Some(last) if last.1 == s && same_look(last.2, &r.props) => last.1 = e,
                    _ => merged.push((s, e, &r.props)),
                }
            }
            for (s, e, props) in merged {
                let (ra, rb) = (at(s), at(e));
                if rb <= ra {
                    continue;
                }
                self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": ra, "focus": rb}));
                self.emit_format(props, spacing);
            }
            self.emit_drop_cap(para, k, offsets, len);
        }
    }

    fn emit_text(&mut self, s: &Placed, spread: &Value, x: f64) {
        let shape = if s.kind == 3 { "ellipse" } else { "rectangle" };
        let id = s.story.unwrap_or(0);
        if s.frame_index > 0 {
            // Its text arrives through the link, threaded once every frame is made.
            let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "shape": shape, "content": "unassigned"}));
            self.chains.entry(id).or_default().insert(s.frame_index, (k, Some(s.clone())));
            self.emit_box_style(s, k);
            self.emit_corners(s, k);
            self.emit_rotation(s, k);
            self.emit_wrap(s, k);
            self.stats.linked += 1;
            return;
        }
        let story = self.p.stories.get(&id).cloned().unwrap_or_default();
        let mut units = story.units.clone();
        while units.last() == Some(&0x0D) {
            units.pop();
        }
        // Page-number fields: Publisher keeps "#" in the text, PressRoom its page-number marker.
        for r in &story.runs {
            if r.props.field == Some(5)
                && r.start.checked_add(1) == Some(r.end)
                && let Some(u) = units.get_mut(r.start)
            {
                *u = 0xE000;
            }
        }
        let (text, offsets) = convert(&units);
        let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "shape": shape, "content": "text", "text": text}));
        self.chains.entry(id).or_default().insert(0, (k, None));
        self.stats.text += 1;
        self.emit_frame_options(s, k);
        self.emit_box_style(s, k);
        self.emit_corners(s, k);
        self.emit_rotation(s, k);
        self.emit_wrap(s, k);
        self.emit_story_formats(&story, k, &offsets, units.len());
        // Direction marks, inserted from the end so the offsets before them still hold.
        for (unit, mark) in bidi_marks(&units, &story).into_iter().rev() {
            let at = offsets.get(unit).copied().unwrap_or(text.len());
            self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": at, "focus": at}));
            self.add("text.insert", json!({"text": mark.to_string(), "raw": true}));
        }
        self.add("edit.deselectAll", json!({}));
    }

    /// WordArt: the words stay, as plain text filling the box.
    fn emit_word_art(&mut self, s: &Placed, spread: &Value, x: f64) {
        let shown: String = s.art_text.chars().take(40).collect();
        self.notes.push(format!("WordArt (\u{201c}{shown}\u{201d}) came across as plain text in its colour, without its shape."));
        let k = self
            .add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "shape": "rectangle", "content": "text", "text": s.art_text}));
        self.add("object.textFrameOptions", json!({"inset": 0, "verticalJustification": "center", "ids": [r#ref(k, "id")]}));
        self.add("object.stroke", json!({"swatch": "[None]", "ids": [r#ref(k, "id")]}));
        let name = if s.art_font.is_empty() { "Arial" } else { s.art_font.as_str() };
        let choice = self.fonts.choose(name, false, false);
        let size = ((s.rect[3] - s.rect[1]).abs() * 0.8).clamp(6.0, 400.0);
        self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": 0, "focus": s.art_text.len()}));
        self.add("type.para", json!({"attrs": {"align": "center"}}));
        self.add(
            "type.char",
            json!({"attrs": {"fontFamily": choice.family, "size": size, "leading": {"kind": "points", "value": size * choice.line_factor}}}),
        );
        self.add("type.char", json!({"attrs": {"fontStyle": choice.style}}));
        if let Some(c) = s.fill {
            self.add("object.color", json!({"color": hex(c), "text": true}));
        }
        self.add("edit.deselectAll", json!({}));
        self.emit_rotation(s, k);
        self.emit_wrap(s, k);
        self.stats.shapes += 1;
    }

    fn emit_line(&mut self, s: &Placed, spread: &Value, x: f64) {
        let (mut x0, mut y0, mut x1, mut y1) = (s.rect[0] + x, s.rect[1], s.rect[2] + x, s.rect[3]);
        if s.flip_h {
            std::mem::swap(&mut x0, &mut x1);
        }
        if s.flip_v {
            std::mem::swap(&mut y0, &mut y1);
        }
        let k = self.add("line.create", json!({"spread": spread, "a": [x0, y0], "b": [x1, y1]}));
        let (weight, color) = s.line.unwrap_or((0.75, Rgb(0, 0, 0)));
        let mut stroke = json!({"weight": weight, "ids": [r#ref(k, "id")]});
        if let Some(name) = arrow_name(s.begin_arrow) {
            stroke["start"] = json!(name);
        }
        if let Some(name) = arrow_name(s.end_arrow) {
            stroke["end"] = json!(name);
        }
        self.add("object.stroke", stroke);
        self.add("object.color", json!({"color": hex(color), "target": "stroke", "ids": [r#ref(k, "id")]}));
        self.stats.lines += 1;
    }

    fn emit_picture(&mut self, s: &Placed, spread: &Value, x: f64) {
        self.pictures += 1;
        let n = s.picture.unwrap_or(0);
        let name = if s.name.is_empty() { "no name".to_string() } else { s.name.clone() };
        let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "content": "graphic"}));
        let file = self.save_picture(n, &name);
        let Some(file) = file else {
            self.emit_box_style(s, k);
            self.emit_rotation(s, k);
            self.emit_wrap(s, k);
            return;
        };
        self.files.push(file.clone());
        let placed = self.add("file.place", json!({"path": file.to_string_lossy(), "frame": r#ref(k, "id")}));
        // Kept inside the publication, as Publisher keeps its pictures: one file to move or email.
        self.add("links.embed", json!({"asset": r#ref(placed, "asset")}));
        let (w, h) = ((s.rect[2] - s.rect[0]).abs(), (s.rect[3] - s.rect[1]).abs());
        let [t, l, b, r] = s.crop;
        if s.crop.iter().any(|&c| c != 0.0) && 1.0 - l - r > 0.001 && 1.0 - t - b > 0.001 {
            // Cropping trims the picture itself and can be undone: the frame stays, the picture grows.
            let (aw, ah) = (w / (1.0 - l - r), h / (1.0 - t - b));
            self.add(
                "object.fittingOptions",
                json!({"fitting": "fitContentToFrame", "crop": [ah * t, aw * l, ah * b, aw * r], "ids": [r#ref(k, "id")]}),
            );
        }
        self.add("object.fit", json!({"mode": "fitContentToFrame", "ids": [r#ref(k, "id")]}));
        if s.brightness.is_some() || s.contrast.is_some() || s.recolor.is_some() {
            let mut a = json!({"ids": [r#ref(k, "id")]});
            if let Some(v) = s.brightness {
                a["brightness"] = json!(v.round());
            }
            if let Some(v) = s.contrast {
                a["contrast"] = json!(v.round());
            }
            if let Some(c) = s.recolor {
                a["recolor"] = json!("light");
                a["tint"] = json!(hex(c));
            }
            self.add("picture.adjust", a);
        }
        match s.line {
            Some((weight, color)) if weight > 0.0 => {
                self.add("object.stroke", json!({"weight": weight, "ids": [r#ref(k, "id")]}));
                self.add("object.color", json!({"color": hex(color), "target": "stroke", "ids": [r#ref(k, "id")]}));
            }
            _ => {
                self.add("object.stroke", json!({"swatch": "[None]", "ids": [r#ref(k, "id")]}));
            }
        }
        for (flipped, axis) in [(s.flip_h, "horizontal"), (s.flip_v, "vertical")] {
            if flipped {
                self.add("transform.flip", json!({"axis": axis, "ids": [r#ref(k, "id")]}));
            }
        }
        self.emit_rotation(s, k);
        self.emit_wrap(s, k);
        self.stats.pictures += 1;
    }

    /// Writes picture `n` where PressRoom can place it. Says so when it can't be read.
    fn save_picture(&mut self, n: u32, name: &str) -> Option<PathBuf> {
        use crate::escher::PictureKind as K;
        let Some(pic) = self.p.picture(n) else {
            self.notes.push(format!("A picture ({name}) couldn't be read, so its box is empty."));
            return None;
        };
        let (ext, data) = match pic.kind {
            K::Png => ("png", pic.data),
            K::Jpeg => ("jpg", pic.data),
            K::Tiff => ("tif", pic.data),
            K::Dib => ("bmp", pic.data),
            K::Emf => match crate::emf::to_svg(&pic.data, pic.meta) {
                Some(c) => {
                    if c.unread > 0 {
                        self.notes.push(format!("A drawing ({name}) has {} parts PressRoom can't read yet, so they are missing from it.", c.unread));
                    }
                    ("svg", c.svg.into_bytes())
                }
                None => {
                    self.notes.push(format!("A drawing ({name}) couldn't be read, so its box is empty."));
                    return None;
                }
            },
            K::Wmf | K::Pict | K::Other => {
                self.notes.push(format!("A picture ({name}) is in a format PressRoom can't read yet, so its box is empty."));
                return None;
            }
        };
        let file = self.folder.join(format!("picture-{:03}.{ext}", self.pictures));
        if std::fs::write(&file, &data).is_err() {
            self.notes.push(format!("A picture ({name}) couldn't be saved while bringing it in, so its box is empty."));
            return None;
        }
        Some(file)
    }

    /// A swatch for an RGB colour (table fills and borders take swatches): black and white are the
    /// document's own, others are made once, named as InDesign names an RGB colour.
    fn swatch_for(&mut self, c: Rgb) -> String {
        match c {
            Rgb(0, 0, 0) => "[Black]".to_string(),
            Rgb(255, 255, 255) => "[Paper]".to_string(),
            Rgb(r, g, b) => {
                let name = format!("R={r} G={g} B={b}");
                if !self.swatches.contains_key(&name) {
                    self.add("swatch.create", json!({"name": name, "color": hex(c)}));
                    self.swatches.insert(name.clone(), name.clone());
                }
                name
            }
        }
    }

    /// A table: a PressRoom table of the same columns, rows, text, fills, borders and cell margins,
    /// in a text box of its own. `false` when the table's records don't add up.
    fn emit_table(&mut self, s: &Placed, table: &TableInfo, spread: &Value, x: f64) -> bool {
        let (nr, nc) = (table.rows as usize, table.cols as usize);
        let story = s.story.and_then(|id| self.p.stories.get(&id)).cloned().unwrap_or_default();
        if nr == 0 || nc == 0 || nr.saturating_mul(nc) > MAX_TABLE_CELLS || table.sizes.len() != nr + nc || story.cell_ends.len() < nr * nc {
            return false;
        }
        let width = s.rect[2] - s.rect[0];
        let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "content": "text", "text": ""}));
        // The box grows down with the table, as Publisher's table does when a row is added.
        self.add("object.textFrameOptions", json!({"inset": 0, "autoSize": "heightOnly", "autoSizeReference": "topLeft", "ids": [r#ref(k, "id")]}));
        self.add("object.stroke", json!({"swatch": "[None]", "ids": [r#ref(k, "id")]}));
        self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": 0, "focus": 0}));
        let ti = self.add("table.insert", json!({"rows": nr, "cols": nc, "width": width}));
        let cells = |r0: usize, r1: usize, c0: usize, c1: usize| json!({"story": r#ref(k, "story"), "table": r#ref(ti, "table"), "rows": [r0, r1], "cols": [c0, c1]});
        let emu = |v: u32| f64::from(v) / model::EMU_PER_POINT;
        for (c, size) in table.sizes.iter().take(nc).enumerate() {
            let mut p = cells(0, nr - 1, c, c);
            p["width"] = json!(emu(*size));
            self.add("table.setColumnWidth", p);
        }
        for (r, size) in table.sizes.iter().skip(nc).take(nr).enumerate() {
            let mut p = cells(r, r, 0, nc - 1);
            p["height"] = json!(emu(*size));
            p["mode"] = json!(if table.grows { "atLeast" } else { "exactly" });
            self.add("table.setRowHeight", p);
        }
        // Each cell: its text, fill, margins and vertical alignment, then its formatting. The story's
        // text and the fills go in the order of the cell records, which needn't be along the rows.
        let mut placed: Vec<(usize, usize, usize, usize, usize)> = Vec::with_capacity(nr * nc);
        let mut start = 0usize;
        for i in 0..nr * nc {
            let end = story.cell_ends.get(i).copied().unwrap_or(start);
            let (r, c) = table.cells.get(i).map_or((i / nc, i % nc), |e| (e.row0 as usize, e.col0 as usize));
            placed.push((r, c, i, start, end));
            start = end.saturating_add(1);
        }
        placed.sort_unstable();
        for (r, c, i, start, end) in placed {
            if r >= nr || c >= nc {
                self.notes.push("A table cell lay outside its table, so its text was left out.".to_string());
                continue;
            }
            let units = story.units.get(start..end.min(story.units.len())).unwrap_or(&[]);
            let (text, _) = convert(units);
            let mut p = cells(r, r, c, c);
            if !text.is_empty() {
                p["text"] = json!(text);
            }
            p["fill"] = match table.fills.get(&(i as u32)) {
                Some((colour, Some(ink))) => json!(self.ink_swatch(&ink.clone(), *colour)),
                Some((colour, None)) => json!(self.swatch_for(*colour)),
                None => json!("[None]"),
            };
            let cell = table.cells.get(i);
            if cell.is_some_and(|e| e.row1 != e.row0 || e.col1 != e.col0) {
                self.notes.push("A table's merged cells came across as separate cells.".to_string());
            }
            let margin = |n: usize| cell.and_then(|e| e.margins.get(n).copied().flatten()).map_or(DEFAULT_CELL_MARGIN, emu);
            p["insets"] = json!([margin(0), margin(1), margin(2), margin(3)]);
            p["vj"] = json!(match cell.map_or(0, |e| e.valign) {
                1 => "center",
                2 => "bottom",
                _ => "top",
            });
            self.add("table.setCell", p);
            let mut sel = cells(r, r, c, c);
            sel["what"] = json!("cell");
            self.add("table.select", sel);
            let para = story.paragraphs.iter().find(|q| q.start <= start && start < q.end).map(|q| q.props.clone()).unwrap_or_default();
            self.add("type.para", json!({"attrs": Self::para_attrs(&para)}));
            // A cell's text takes one formatting (its first character's when it is mixed).
            let runs: Vec<&CharProps> = story.runs.iter().filter(|q| q.start < end.max(start + 1) && q.end > start).map(|q| &q.props).collect();
            if let Some(first) = runs.first().copied() {
                if runs.iter().any(|q| !same_look(first, q)) && end > start {
                    self.stats.mixed_cells += 1;
                }
                self.emit_format(first, para.spacing());
            }
        }
        // The borders, a run of cells at a time.
        let same = |a: Option<&(f64, Rgb)>, b: Option<&(f64, Rgb)>| match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => (a.0 - b.0).abs() < 1e-9 && a.1 == b.1,
            _ => false,
        };
        for line in 0..=nr {
            let mut c0 = 0;
            while c0 < nc {
                let e = table.across.get(&(line as u32, c0 as u32));
                let mut c1 = c0;
                while c1 + 1 < nc && same(table.across.get(&(line as u32, (c1 + 1) as u32)), e) {
                    c1 += 1;
                }
                let row = line.min(nr - 1);
                let mut p = cells(row, row, c0, c1);
                p["stroke"] = self.stroke_of(e, if line < nr { "top" } else { "bottom" });
                self.add("table.setCell", p);
                c0 = c1 + 1;
            }
        }
        for line in 0..=nc {
            let mut r0 = 0;
            while r0 < nr {
                let e = table.down.get(&(r0 as u32, line as u32));
                let mut r1 = r0;
                while r1 + 1 < nr && same(table.down.get(&((r1 + 1) as u32, line as u32)), e) {
                    r1 += 1;
                }
                let col = line.min(nc - 1);
                let mut p = cells(r0, r1, col, col);
                p["stroke"] = self.stroke_of(e, if line < nc { "left" } else { "right" });
                self.add("table.setCell", p);
                r0 = r1 + 1;
            }
        }
        // The empty line the table sits on takes no room, so the box is the table's height. (Last:
        // the table commands above need the insertion point in the table.)
        self.add("text.select", json!({"story": r#ref(k, "story"), "anchor": 0, "focus": 64}));
        self.add("type.char", json!({"attrs": {"size": 1, "leading": {"kind": "points", "value": 0.1}}}));
        self.add("edit.deselectAll", json!({}));
        self.emit_rotation(s, k);
        self.emit_wrap(s, k);
        self.stats.tables += 1;
        true
    }

    fn stroke_of(&mut self, e: Option<&(f64, Rgb)>, edges: &str) -> Value {
        match e {
            Some((weight, colour)) => json!({"weight": weight, "color": self.swatch_for(*colour), "edges": edges}),
            None => json!({"weight": 0, "edges": edges}),
        }
    }

    fn emit_shape(&mut self, s: &Placed, spread: &Value, x: f64) {
        if matches!(s.kind, 20 | 32..=40) {
            return self.emit_line(s, spread, x);
        }
        let story_len = s.story.and_then(|id| self.p.stories.get(&id)).map_or(0, |t| t.units.len());
        if s.crop.iter().any(|&c| c >= 255.0) && story_len <= 1 {
            // Publisher never draws these leftovers.
            self.stats.left_out += 1;
            return;
        }
        let is_text_box = s.kind == 202;
        if let Some(table) = &s.table {
            if self.emit_table(s, table, spread, x) {
                return;
            }
            self.notes.push("A table's records didn't add up, so its text came in as a plain text box.".to_string());
            return self.emit_text(s, spread, x);
        }
        if is_text_box || story_len > 1 || s.frame_index > 0 {
            return self.emit_text(s, spread, x);
        }
        if s.picture.is_some() {
            return self.emit_picture(s, spread, x);
        }
        match s.kind {
            1..=3 => {
                let shape = if s.kind == 3 { "ellipse" } else { "rectangle" };
                let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "shape": shape, "content": "unassigned"}));
                self.emit_box_style(s, k);
                self.emit_corners(s, k);
                self.emit_rotation(s, k);
                self.emit_wrap(s, k);
                self.stats.shapes += 1;
            }
            136..=175 | 24 if !s.art_text.is_empty() => self.emit_word_art(s, spread, x),
            // A freeform, or a shape of known corners: a closed path.
            kind if !s.outline.is_empty() || outline_of(kind).is_some() => {
                let is_free = !s.outline.is_empty();
                let points = if is_free { s.outline.clone() } else { outline_of(kind).unwrap_or_default() };
                let (w, h) = (s.rect[2] - s.rect[0], s.rect[3] - s.rect[1]);
                let anchors: Vec<Value> = points
                    .iter()
                    .map(|&(px, py)| {
                        let fx = if s.flip_h { 1.0 - px } else { px };
                        let fy = if s.flip_v { 1.0 - py } else { py };
                        json!({"p": [s.rect[0] + x + fx * w, s.rect[1] + fy * h]})
                    })
                    .collect();
                if is_free {
                    self.notes.push("A freehand shape's curves came across as straight lines.".to_string());
                } else if s.adjust.is_some() {
                    self.notes.push("A shape came across in its standard proportions, not as it was adjusted.".to_string());
                }
                let k = self.add("path.create", json!({"spread": spread, "anchors": anchors, "closed": true}));
                self.emit_box_style(s, k);
                self.emit_rotation(s, k);
                self.emit_wrap(s, k);
                self.stats.shapes += 1;
            }
            other => {
                self.notes.push(format!("A shape of a kind PressRoom can't draw yet (number {other}) became a plain box."));
                let k = self.add("frame.create", json!({"spread": spread, "rect": Self::rect(s, x), "shape": "rectangle", "content": "unassigned"}));
                self.emit_box_style(s, k);
                self.emit_rotation(s, k);
                self.stats.shapes += 1;
            }
        }
    }
}

/// Languages written right to left (the low ten bits of a language id).
const RTL_LANGUAGES: [u32; 12] = [0x01, 0x0D, 0x20, 0x29, 0x3D, 0x59, 0x5A, 0x63, 0x65, 0x80, 0x8C, 0x92];

fn rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFE)
}

/// Publisher lays out mixed right-to-left and left-to-right text by its own rules, not Unicode's:
/// punctuation reads in the direction of the language it was typed in, and a space in the direction
/// of the character before it. PressRoom follows Unicode, so invisible direction marks (U+200F right
/// to left, U+200E left to right) go where the two differ: in left-to-right paragraphs that hold
/// right-to-left letters. Returns (unit offset, mark) pairs in text order.
fn bidi_marks(units: &[u16], story: &Text) -> Vec<(usize, char)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Rtl,
        Ltr,
        Digit,
        Neutral(bool),
    }
    struct Token {
        kind: Kind,
        start: usize,
        end: usize,
    }
    let mut marks = Vec::new();
    let chars: Vec<(usize, char)> = char::decode_utf16(units.iter().copied())
        .scan(0usize, |at, r| {
            let here = *at;
            let c = r.unwrap_or('\u{FFFD}');
            *at += c.len_utf16();
            Some((here, c))
        })
        .collect();
    for para in chars.split(|(_, c)| *c == '\r') {
        if !para.iter().any(|(_, c)| rtl_letter(*c)) {
            continue;
        }
        let mut tokens: Vec<Token> = Vec::new();
        let mut prev_rtl = false;
        for &(at, c) in para {
            let next = at + c.len_utf16();
            let code = c as u32;
            let neutral = if code == 0x200F || (!c.is_numeric() && rtl_letter(c)) {
                tokens.push(Token { kind: Kind::Rtl, start: at, end: next });
                prev_rtl = true;
                continue;
            } else if c.is_ascii_digit() || (c.is_numeric() && !c.is_alphabetic()) {
                tokens.push(Token { kind: Kind::Digit, start: at, end: next });
                prev_rtl = false;
                continue;
            } else if c.is_whitespace() {
                prev_rtl
            } else if c.is_alphabetic() || code == 0x200E || (0xE000..=0xF8FF).contains(&code) {
                tokens.push(Token { kind: Kind::Ltr, start: at, end: next });
                prev_rtl = false;
                continue;
            } else if c.is_control() || matches!(code, 0x0300..=0x036F | 0x200B..=0x200D | 0xFEFF) {
                continue;
            } else {
                let language = story.runs.iter().find(|r| r.start <= at && at < r.end).and_then(|r| r.props.language).unwrap_or(0);
                RTL_LANGUAGES.contains(&(language & 0x3FF))
            };
            match tokens.last_mut() {
                Some(last) if last.kind == Kind::Neutral(neutral) && last.end == at => last.end = next,
                _ => tokens.push(Token { kind: Kind::Neutral(neutral), start: at, end: next }),
            }
            prev_rtl = neutral;
        }
        // Unicode gives a run of spaces and punctuation the direction of the letters on both sides,
        // or left to right when they differ.
        let mut last_rtl = false;
        for i in 0..tokens.len() {
            let (kind, start, end) = match tokens.get(i) {
                Some(t) => (t.kind, t.start, t.end),
                None => continue,
            };
            let next = tokens.get(i + 1).map(|t| t.kind);
            match kind {
                Kind::Rtl => last_rtl = true,
                Kind::Ltr => last_rtl = false,
                Kind::Digit => {}
                Kind::Neutral(true) => {
                    if !last_rtl {
                        marks.push((start, '\u{200F}'));
                    }
                    if next != Some(Kind::Rtl) {
                        marks.push((end, '\u{200F}'));
                    }
                    last_rtl = true;
                }
                Kind::Neutral(false) => {
                    if last_rtl && next != Some(Kind::Ltr) && next.is_some() {
                        marks.push((end, '\u{200E}'));
                        last_rtl = false;
                    }
                }
            }
        }
    }
    marks
}

fn same_look(a: &CharProps, b: &CharProps) -> bool {
    a.font() == b.font()
        && a.size == b.size
        && a.bold == b.bold
        && a.italic == b.italic
        && a.underline.is_some_and(|u| u != 0) == b.underline.is_some_and(|u| u != 0)
        && a.color == b.color
        && a.caps == b.caps
        && a.small_caps == b.small_caps
        && a.tracking == b.tracking
        && a.kerning == b.kerning
        && a.scaling == b.scaling
}

/// Where a page sits in PressRoom: its spread and its left edge in that spread.
fn page_place(abs: usize, facing: bool, width: f64) -> (usize, f64) {
    if !facing || abs == 0 {
        return (if facing { 0 } else { abs }, 0.0);
    }
    let spread = abs.div_ceil(2);
    (spread, if abs % 2 == 1 { 0.0 } else { width })
}

/// The steps that make `p`. Pictures are written into `folder`.
pub fn steps(p: &Publication, fonts: &dyn Fonts, folder: &Path) -> Output {
    let n = p.pages.len();
    let half_letter = (p.width - 396.0).abs() < 2.0 && (p.height - 612.0).abs() < 2.0;
    let facing = p.two_page_view || (half_letter && n >= 4 && n.is_multiple_of(4));
    let mut e = Emit {
        p,
        fonts,
        folder,
        steps: Vec::new(),
        stats: Stats::default(),
        notes: p.notes.clone(),
        files: Vec::new(),
        swatches: BTreeMap::new(),
        chains: BTreeMap::new(),
        pictures: 0,
    };
    e.add(
        "file.new",
        json!({"units": "inches", "width": p.width, "height": p.height, "pages": n, "facingPages": facing, "margins": p.margin, "intent": "print"}),
    );
    e.add("document.preferences", json!({"textWrapOnlyBeneath": true, "glyphFallback": true}));
    e.add("style.paragraph.edit", json!({"name": "[Basic Paragraph]", "chars": {"fontFamily": "Calibri", "fontStyle": "Regular", "size": 10.0}}));
    for (i, page) in p.pages.iter().enumerate() {
        let (spread, x) = page_place(i, facing, p.width);
        for s in &page.shapes {
            e.emit_shape(s, &json!(spread), x);
        }
    }
    if let Some(m) = &p.master {
        let lefts: &[f64] = if facing { &[0.0, p.width] } else { &[0.0] };
        for &x in lefts {
            for s in &m.shapes {
                e.emit_shape(s, &json!({"kind": "parent", "index": 0}), x);
            }
        }
    }
    let chains = std::mem::take(&mut e.chains);
    for (_, frames) in chains {
        let mut prev = None;
        for (_, (k, shape)) in frames {
            if let Some(from) = prev {
                e.add("text.thread", json!({"from": r#ref(from, "id"), "to": r#ref(k, "id")}));
            }
            if let Some(s) = shape {
                e.emit_frame_options(&s, k);
            }
            prev = Some(k);
        }
    }
    let _ = &e.swatches;
    Output { steps: e.steps, stats: e.stats, notes: e.notes, files: e.files }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PANTONE 626 C, whose hand-done sum drew as a bluish slate in PressRoom.
    #[test]
    fn an_inks_recipe_draws_as_its_screen_colour() {
        let [c, m, y, k] = ink_recipe(Rgb(0x2c, 0x5e, 0x4f));
        let drawn = Cms::default().cmyk_to_srgb([c, m, y, k].map(|v| (v / 100.0) as f32), false);
        for (got, want) in drawn.iter().zip([0x2c, 0x5e, 0x4f]) {
            assert!((got * 255.0 - f32::from(want as u8)).abs() < 6.0, "{drawn:?}");
        }
    }

    #[test]
    fn text_converts_with_byte_offsets_for_every_unit() {
        let (t, o) = convert(&[0x41, 0x0D, 0x05D0, 0x0B, 0xD83D, 0xDE00, 0x42]);
        assert_eq!(t, "A\n\u{5D0}\u{2028}\u{1F600}B");
        assert_eq!(o, vec![0, 1, 2, 4, 7, 7, 11, 12]);
        // A lone surrogate is a replacement mark, not a crash.
        assert_eq!(convert(&[0xD83D]).0, "\u{FFFD}");
        assert_eq!(convert(&[0xDE00, 0x41]).0, "\u{FFFD}A");
    }

    #[test]
    fn pages_go_on_spreads_as_publisher_shows_them() {
        assert_eq!(page_place(0, true, 100.0), (0, 0.0));
        assert_eq!(page_place(1, true, 100.0), (1, 0.0));
        assert_eq!(page_place(2, true, 100.0), (1, 100.0));
        assert_eq!(page_place(3, true, 100.0), (2, 0.0));
        assert_eq!(page_place(3, false, 100.0), (3, 0.0));
    }
}

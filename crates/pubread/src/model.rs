//! The publication as PressRoom needs it: the streams read and put together, in points, with each
//! page's shapes back to front and group members in their place on the page.

use std::collections::BTreeMap;

use crate::contents::{Contents, TableInfo};
use crate::escher::{self, Drawings, Shape as EShape};
use crate::quill::{self, CharProps, ParaProps, TextStream};
use crate::{PubError, cfb, contents};

pub const EMU_PER_POINT: f64 = 12700.0;
/// Deepest group followed when placing shapes.
const MAX_GROUP_DEPTH: usize = 64;
/// The numbers of the pages Publisher keeps for itself.
const WORKING_PAGES: [u32; 4] = [0x10D, 0x110, 0x113, 0x117];

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// One shape on a page.
#[derive(Clone, Debug, Default)]
pub struct Placed {
    pub seq: u32,
    /// The shape type number.
    pub kind: u16,
    /// Left, top, right, bottom in points on the page, before rotation.
    pub rect: [f64; 4],
    /// Clockwise degrees.
    pub rotation: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub fill: Option<Rgb>,
    /// Weight in points and colour.
    pub line: Option<(f64, Rgb)>,
    pub story: Option<u32>,
    pub frame_index: u32,
    /// 0 top, 1 middle, 2 bottom.
    pub valign: u32,
    /// Top, left, bottom, right in points.
    pub insets: [f64; 4],
    pub columns: u32,
    pub gutter: f64,
    /// The picture's number in the store, from 1.
    pub picture: Option<u32>,
    /// Trimmed from the top, left, bottom and right, as fractions of the picture.
    pub crop: [f64; 4],
    /// Brightness and contrast from -100 to 100, where the picture has them.
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    /// The colour a picture is recoloured with.
    pub recolor: Option<Rgb>,
    /// Its first adjust value, for a rounded rectangle's corner.
    pub adjust: Option<i32>,
    /// A freeform's points, as fractions of its box.
    pub outline: Vec<(f64, f64)>,
    /// WordArt's words and font.
    pub art_text: String,
    pub art_font: String,
    pub table: Option<TableInfo>,
    /// Whether text wraps around it.
    pub wrap: bool,
    /// The colour-matching ink of its fill.
    pub fill_ink: Option<String>,
    /// Its face is drawn with a 3-D bevel.
    pub bevel: bool,
    pub begin_arrow: u32,
    pub end_arrow: u32,
    /// The picture's name, or a shape's.
    pub name: String,
    /// Every property as written, for what isn't read yet.
    pub props: BTreeMap<u16, u32>,
    pub tertiary: BTreeMap<u16, u32>,
    pub complex: BTreeMap<u16, Vec<u8>>,
    pub layout: BTreeMap<u8, u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub seq: u32,
    pub shapes: Vec<Placed>,
}

/// A story's character formatting with its style applied, and the same for its paragraphs.
#[derive(Clone, Debug, Default)]
pub struct Text {
    pub id: u32,
    pub units: Vec<u16>,
    pub paragraphs: Vec<quill::Run<ParaProps>>,
    pub runs: Vec<quill::Run<CharProps>>,
    pub cell_ends: Vec<usize>,
}

/// A colour-matching ink, by the name Publisher keeps with it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ink {
    pub name: String,
    pub rgb: Rgb,
}

#[derive(Clone, Debug, Default)]
pub struct Publication {
    /// Page size in points.
    pub width: f64,
    pub height: f64,
    pub pages: Vec<Page>,
    pub master: Option<Page>,
    pub stories: BTreeMap<u32, Text>,
    pub fonts: Vec<String>,
    pub colors: Vec<Rgb>,
    /// The inks named in the colour list, by the colour's place in it.
    pub color_inks: BTreeMap<u32, String>,
    /// The text styles' character formatting, for what refers to them.
    pub char_styles: Vec<CharProps>,
    pub store: Vec<escher::StoreEntry>,
    pub delay: Vec<u8>,
    /// Facing pages in Publisher's view.
    pub two_page_view: bool,
    /// The layout guides' margin in points.
    pub margin: f64,
    /// What could not be read, in words for a person.
    pub notes: Vec<String>,
}

impl Publication {
    /// The picture numbered `n` in the store (from 1).
    pub fn picture(&self, n: u32) -> Option<escher::Picture> {
        let entry = self.store.get((n as usize).checked_sub(1)?)?;
        if !entry.used {
            return None;
        }
        escher::picture(&self.delay, entry.offset)
    }
}

/// Decodes a colour reference: a scheme colour's number with 0x08 in the top byte, otherwise red,
/// green and blue from the low byte up. `None` for "no colour".
pub fn color(reference: u32, palette: &[u32]) -> Option<Rgb> {
    if reference == 0xFFFF_FFFF {
        return None;
    }
    let bytes = reference.to_le_bytes();
    if reference >> 24 == 0x08 {
        let v = *palette.get((reference & 0x00FF_FFFF) as usize)?;
        let b = v.to_le_bytes();
        return Some(Rgb(b[0], b[1], b[2]));
    }
    Some(Rgb(bytes[0], bytes[1], bytes[2]))
}

/// A shape's place in points on the page, with its group's coordinates applied.
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// What the group's own coordinates map to.
    system: [f64; 4],
    onto: [f64; 4],
}

fn map(f: &Frame, a: [i32; 4]) -> [f64; 4] {
    let sx = f.system[2] - f.system[0];
    let sy = f.system[3] - f.system[1];
    let kx = if sx == 0.0 { 1.0 } else { (f.onto[2] - f.onto[0]) / sx };
    let ky = if sy == 0.0 { 1.0 } else { (f.onto[3] - f.onto[1]) / sy };
    [
        f.onto[0] + (f64::from(a[0]) - f.system[0]) * kx,
        f.onto[1] + (f64::from(a[1]) - f.system[1]) * ky,
        f.onto[0] + (f64::from(a[2]) - f.system[0]) * kx,
        f.onto[1] + (f64::from(a[3]) - f.system[1]) * ky,
    ]
}

struct Build<'a> {
    c: &'a Contents,
    /// What can't be brought across, as it is met.
    notes: std::cell::RefCell<Vec<String>>,
    /// Each table's cell fills and border lines.
    parts: BTreeMap<u32, Vec<&'a EShape>>,
    half_w: f64,
    half_h: f64,
}

impl Build<'_> {
    fn client_rect(&self, a: [i32; 4]) -> [f64; 4] {
        [
            f64::from(a[0]) / EMU_PER_POINT + self.half_w,
            f64::from(a[1]) / EMU_PER_POINT + self.half_h,
            f64::from(a[2]) / EMU_PER_POINT + self.half_w,
            f64::from(a[3]) / EMU_PER_POINT + self.half_h,
        ]
    }

    fn rect_of(&self, s: &EShape, parent: Option<&Frame>) -> Option<[f64; 4]> {
        if let (Some(a), Some(f)) = (s.child, parent) {
            return Some(map(f, a));
        }
        s.client.map(|a| self.client_rect(a))
    }

    fn place(&self, s: &EShape, rect: [f64; 4]) -> Placed {
        let info = s.seq.and_then(|q| self.c.shapes.get(&q));
        let fp = |id: u16, default: f64| s.prop(id).map_or(default, |v| f64::from(v) / EMU_PER_POINT);
        let filled = fill_on(s);
        let fill = if filled { color(s.prop(0x181).unwrap_or(0x0800_0001), &self.c.palette) } else { None };
        let line = if line_on(s) {
            color(s.prop(0x1C0).unwrap_or(0x0800_0000), &self.c.palette).map(|c| (s.prop(0x1CB).map_or(0.75, |w| f64::from(w) / EMU_PER_POINT), c))
        } else {
            None
        };
        let crop_of = |id: u16| s.prop(id).map_or(0.0, |v| f64::from(v as i32) / 65536.0);
        let rotation = s.prop(4).map_or(0.0, |v| f64::from(v as i32) / 65536.0);
        let rect = upright(rect, rotation);
        Placed {
            seq: s.seq.unwrap_or(0),
            kind: s.kind,
            rect,
            rotation,
            flip_h: s.flip_h(),
            flip_v: s.flip_v(),
            fill,
            line,
            story: info.and_then(|i| i.story),
            frame_index: info.map_or(0, |i| i.frame_index),
            valign: info.and_then(|i| i.valign).filter(|&v| v <= 2).unwrap_or(0),
            insets: [fp(0x82, 2.88), fp(0x81, 2.88), fp(0x84, 2.88), fp(0x83, 2.88)],
            columns: s.tert(0x8C).or(s.prop(0x8C)).unwrap_or(1),
            gutter: s.tert(0x8D).or(s.prop(0x8D)).map_or(0.0, |v| f64::from(v) / EMU_PER_POINT),
            picture: s.prop(0x104).filter(|&n| n > 0),
            crop: [crop_of(0x100), crop_of(0x102), crop_of(0x101), crop_of(0x103)],
            // Brightness runs across a signed 16 bits, and neutral contrast is 1.0, with what is
            // above it stretched: 0.5 / (1 - setting).
            brightness: s.prop(0x109).filter(|&v| v != 0).map(|v| f64::from(v as i32) / 32768.0 * 100.0),
            contrast: s.prop(0x108).filter(|&v| v != 0x1_0000).map(|v| {
                let k = f64::from(v as i32) / 65536.0;
                let setting = if k > 1.0 { 1.0 - 0.5 / k } else { k / 2.0 };
                (setting - 0.5) * 200.0
            }),
            recolor: s.tert(0x11A).or(s.prop(0x11A)).and_then(|r| color(r, &self.c.palette)),
            adjust: s.prop(0x147).map(|v| v as i32),
            outline: freeform(s),
            art_text: s.complex.get(&0xC0).map(|b| wide_string(b)).unwrap_or_default(),
            art_font: s.complex.get(&0xC5).map(|b| wide_string(b)).unwrap_or_default(),
            table: info.and_then(|i| i.table.clone()).map(|mut table| {
                if let Some(q) = s.seq {
                    self.add_table_parts(q, &mut table);
                }
                table
            }),
            // Layout record 4 holds the wrap mode: 0x103 is no wrap, and no record is wrap.
            wrap: info.is_none_or(|i| i.raw.get(&4).is_none_or(|&v| v != 0x103)),
            fill_ink: s.complex.get(&0x41A1).and_then(|b| ink_name(b)),
            bevel: s.tert(0x29A).is_some_and(|v| v > 1) || s.tert(0x298).is_some_and(|v| v > 0),
            begin_arrow: s.prop(0x1D0).unwrap_or(0),
            end_arrow: s.prop(0x1D1).unwrap_or(0),
            name: s.complex.get(&0x105).map(|b| wide_string(b)).unwrap_or_default(),
            props: s.props.clone(),
            tertiary: s.tertiary.clone(),
            complex: s.complex.clone(),
            layout: info.map(|i| i.raw.clone()).unwrap_or_default(),
        }
    }

    fn add_table_parts(&self, seq: u32, table: &mut TableInfo) {
        let Some(parts) = self.parts.get(&seq) else { return };
        let (rows, cols) = (table.rows, table.cols);
        for s in parts {
            let Some(part) = s.table_part else { continue };
            let colour = s.prop(0x181).and_then(|r| color(r, &self.c.palette));
            if !part.is_line {
                if let Some(c) = colour.filter(|_| fill_on(s)) {
                    let ink = s.complex.get(&0x41A1).and_then(|b| ink_name(b));
                    table.fills.insert(part.cell, (c, ink));
                }
                continue;
            }
            let weight = s.prop(0x1CB).map_or(0.75, |w| f64::from(w) / EMU_PER_POINT);
            let Some(c) = colour else { continue };
            if weight <= 0.0 {
                continue;
            }
            // Where two lines lie on the same edge, the heavier shows.
            let put = |map: &mut BTreeMap<(u32, u32), (f64, Rgb)>, key: (u32, u32)| {
                if map.get(&key).is_none_or(|e| e.0 < weight) {
                    map.insert(key, (weight, c));
                }
            };
            if part.col0 == part.col1 {
                for r in part.row0..part.row1.min(rows) {
                    put(&mut table.down, (r, part.col0));
                }
            } else {
                for col in part.col0..part.col1.min(cols) {
                    put(&mut table.across, (part.row1, col));
                }
            }
        }
    }

    /// Adds `s` to `out` (a group's members, each in its place).
    fn add(&self, s: &EShape, parent: Option<&Frame>, depth: usize, out: &mut Vec<Placed>) {
        if depth > MAX_GROUP_DEPTH {
            return;
        }
        let rect = self.rect_of(s, parent);
        if s.is_group && (!s.members.is_empty() || s.system.is_some()) {
            if s.prop(4).is_some_and(|v| v != 0) || s.flip_h() || s.flip_v() {
                self.notes.borrow_mut().push("A turned or flipped group's objects came across upright, each in its own place.".to_string());
            }
            let system = s.system.map(|a| [f64::from(a[0]), f64::from(a[1]), f64::from(a[2]), f64::from(a[3])]);
            let frame = match (system, rect) {
                (Some(system), Some(onto)) => Some(Frame { system, onto }),
                _ => None,
            };
            for m in &s.members {
                self.add(m, frame.as_ref().or(parent), depth + 1, out);
            }
            return;
        }
        if let Some(r) = rect {
            out.push(self.place(s, r));
        }
    }
}

/// A freeform's points as fractions of its box: a count, room and an element size, then the points.
fn freeform(s: &EShape) -> Vec<(f64, f64)> {
    let Some(b) = s.complex.get(&0x145) else { return Vec::new() };
    let (Some(n), Some(size)) = (crate::bytes::u16_at(b, 0), crate::bytes::u16_at(b, 4)) else { return Vec::new() };
    let (w, h) = (f64::from(s.prop(0x142).unwrap_or(21600).max(1)), f64::from(s.prop(0x143).unwrap_or(21600).max(1)));
    let (left, top) = (f64::from(s.prop(0x140).unwrap_or(0)), f64::from(s.prop(0x141).unwrap_or(0)));
    let span = (w - left).max(1.0);
    let tall = (h - top).max(1.0);
    (0..usize::from(n))
        .map_while(|i| {
            // 0xFFF0 is two 16 bit numbers a point; 8 is two 32 bit numbers.
            let (x, y) = if size == 8 {
                (f64::from(crate::bytes::i32_at(b, 6 + i * 8)?), f64::from(crate::bytes::i32_at(b, 10 + i * 8)?))
            } else {
                (f64::from(crate::bytes::u16_at(b, 6 + i * 4)? as i16), f64::from(crate::bytes::u16_at(b, 8 + i * 4)? as i16))
            };
            Some(((x - left) / span, (y - top) / tall))
        })
        .collect()
}

/// Office stores the box of a shape turned nearer a quarter than not with its sides swapped about its
/// middle: this gives the box before the turn.
fn upright(r: [f64; 4], rotation: f64) -> [f64; 4] {
    let a = rotation.rem_euclid(360.0);
    if !((45.0..135.0).contains(&a) || (225.0..315.0).contains(&a)) {
        return r;
    }
    let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    let (hw, hh) = ((r[3] - r[1]) / 2.0, (r[2] - r[0]) / 2.0);
    [cx - hw, cy - hh, cx + hw, cy + hh]
}

/// A colour-matching ink's name from Publisher's description of it: "P2,#002c005e004f0000,PANTONE 626 C".
pub fn ink_name(b: &[u8]) -> Option<String> {
    let text = wide_string(b);
    let name = text.split(',').nth(2)?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn wide_string(b: &[u8]) -> String {
    let units: Vec<u16> = crate::bytes::utf16_units(b).into_iter().take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// The fill's on-off flag: bit 4, used when bit 20 says so. Fills are on unless turned off.
fn fill_on(s: &EShape) -> bool {
    match s.prop(0x1BF) {
        Some(v) if v & 0x0010_0000 != 0 => v & 0x10 != 0,
        _ => true,
    }
}

/// The line's on-off flag: bit 3, used when bit 19 says so.
fn line_on(s: &EShape) -> bool {
    match s.prop(0x1FF) {
        Some(v) if v & 0x0008_0000 != 0 => v & 0x08 != 0,
        _ => true,
    }
}

/// Finds every shape with a `Contents` number, in stacking order.
fn index<'a>(s: &'a EShape, depth: usize, order: &mut Vec<&'a EShape>) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    if s.seq.is_some() {
        order.push(s);
    }
    for m in &s.members {
        index(m, depth + 1, order);
    }
}

fn build_page(seq: u32, build: &Build<'_>, drawings: &Drawings, c: &Contents, notes: &mut Vec<String>) -> Page {
    let mut page = Page { seq, shapes: Vec::new() };
    let Some(info) = c.pages.get(&seq) else { return page };
    let mut order = Vec::new();
    for top in &drawings.tops {
        index(top, 0, &mut order);
    }
    for s in order {
        if s.seq.is_some_and(|q| info.shapes.contains(&q)) {
            build.add(s, None, 0, &mut page.shapes);
        }
    }
    let found: Vec<u32> = {
        let mut v = Vec::new();
        for top in &drawings.tops {
            collect_seq(top, 0, &mut v);
        }
        v
    };
    let missing = info.shapes.iter().filter(|q| !found.contains(q)).count();
    if missing > 0 {
        notes.push(format!("{missing} objects on a page have no drawing record, so they were left out."));
    }
    page
}

fn collect_seq(s: &EShape, depth: usize, out: &mut Vec<u32>) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    if let Some(q) = s.seq {
        out.push(q);
    }
    for m in &s.members {
        collect_seq(m, depth + 1, out);
    }
}

fn apply_styles(t: &TextStream, notes: &mut Vec<String>) -> BTreeMap<u32, Text> {
    let mut out = BTreeMap::new();
    let _ = notes;
    for (&id, s) in &t.stories {
        let paragraphs = s
            .paragraphs
            .iter()
            .map(|p| {
                let style = t.styles.get(p.props.style.unwrap_or(0) as usize);
                quill::Run { start: p.start, end: p.end, props: style.map_or_else(|| p.props.clone(), |(_, sp)| quill::merge_para(&p.props, sp)) }
            })
            .collect::<Vec<_>>();
        let runs = s
            .runs
            .iter()
            .map(|r| {
                let para = s.paragraphs.iter().find(|p| p.start <= r.start && r.start < p.end);
                let style = t.styles.get(para.and_then(|p| p.props.style).unwrap_or(0) as usize);
                quill::Run { start: r.start, end: r.end, props: style.map_or_else(|| r.props.clone(), |(sc, _)| quill::merge_char(&r.props, sc)) }
            })
            .collect::<Vec<_>>();
        out.insert(id, Text { id, units: s.text.clone(), paragraphs, runs, cell_ends: s.cell_ends.clone() });
    }
    out
}

/// Reads a publication from the bytes of a `.pub` file.
pub fn read(file: &[u8]) -> Result<Publication, PubError> {
    if file.len() > crate::MAX_FILE {
        return Err(PubError::TooLarge);
    }
    let cf = cfb::Compound::open(file)?;
    let missing = |what: &str| PubError::Damaged(format!("its {what} is missing"));
    let contents_bytes = cf.stream(&["Contents"])?.ok_or(PubError::NotPublisher)?;
    let c = contents::read(&contents_bytes)?;
    let quill_bytes = cf.stream(&["Quill", "QuillSub", "CONTENTS"])?.ok_or_else(|| missing("text"))?;
    let t = quill::read(&quill_bytes);
    let escher_bytes = cf.stream(&["Escher", "EscherStm"])?.ok_or_else(|| missing("drawing"))?;
    let drawings = escher::read(&escher_bytes);
    let delay = cf.stream(&["Escher", "EscherDelayStm"])?.unwrap_or_default();

    let mut notes = t.problems.clone();
    let width = f64::from(c.width_emu) / EMU_PER_POINT;
    let height = f64::from(c.height_emu) / EMU_PER_POINT;
    let mut parts: BTreeMap<u32, Vec<&EShape>> = BTreeMap::new();
    for top in &drawings.tops {
        for s in &top.members {
            if let Some(p) = s.table_part {
                parts.entry(p.table).or_default().push(s);
            }
        }
    }
    let build = Build { c: &c, notes: std::cell::RefCell::new(Vec::new()), parts, half_w: width / 2.0, half_h: height / 2.0 };

    // The document lists masters first, then the publication's pages, then Publisher's own working
    // pages, which always have the same numbers.
    let masters: Vec<u32> = c.page_list.iter().copied().filter(|q| c.pages.get(q).is_some_and(|p| p.is_master)).collect();
    let body: Vec<u32> = c.page_list.iter().copied().filter(|q| !WORKING_PAGES.contains(q) && c.pages.get(q).is_some_and(|p| !p.is_master)).collect();
    let built: BTreeMap<u32, Page> = masters.iter().map(|&q| (q, build_page(q, &build, &drawings, &c, &mut notes))).collect();
    // PressRoom's master page is the first that has anything on it. The objects of any other go on
    // the pages that use it, so they still show.
    let main = masters.iter().copied().find(|q| built.get(q).is_some_and(|m| !m.shapes.is_empty())).or(masters.first().copied());
    let pages: Vec<Page> = body
        .iter()
        .map(|&q| {
            let mut page = build_page(q, &build, &drawings, &c, &mut notes);
            let own = c.pages.get(&q).and_then(|i| i.master).filter(|m| Some(*m) != main);
            if let Some(m) = own.and_then(|m| built.get(&m)).filter(|m| !m.shapes.is_empty()) {
                let mut all = m.shapes.clone();
                all.append(&mut page.shapes);
                page.shapes = all;
                notes.push("A page uses a second master page: its objects were put on the page itself.".to_string());
            }
            page
        })
        .collect();
    let master = main.and_then(|m| built.get(&m).cloned());
    notes.extend(build.notes.borrow().iter().cloned());
    let stories = apply_styles(&t, &mut notes);
    let colors = t.colors.iter().map(|&r| color(r, &c.palette).unwrap_or(Rgb(0, 0, 0))).collect();
    let char_styles = t.styles.iter().map(|(sc, _)| sc.clone()).collect();
    Ok(Publication {
        width,
        height,
        pages,
        master,
        stories,
        fonts: t.fonts,
        colors,
        color_inks: t.color_inks,
        char_styles,
        store: drawings.store,
        delay,
        two_page_view: c.two_page_view,
        margin: c.margin.unwrap_or(36.0),
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_come_from_the_scheme_or_the_reference() {
        let palette = [0x0000_0000, 0x0011_2233];
        assert_eq!(color(0x0800_0001, &palette), Some(Rgb(0x33, 0x22, 0x11)));
        assert_eq!(color(0x0078_3C14, &palette), Some(Rgb(0x14, 0x3C, 0x78)));
        assert_eq!(color(0xFFFF_FFFF, &palette), None);
        assert_eq!(color(0x0800_0009, &palette), None);
    }

    #[test]
    fn nothing_breaks_on_bad_bytes() {
        assert!(read(&[]).is_err());
        assert!(read(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]).is_err());
        assert!(read(&vec![0u8; 4096]).is_err());
    }

    #[test]
    fn a_group_puts_its_members_in_place() {
        let f = Frame { system: [0.0, 0.0, 100.0, 100.0], onto: [10.0, 10.0, 60.0, 110.0] };
        assert_eq!(map(&f, [50, 50, 100, 100]), [35.0, 60.0, 60.0, 110.0]);
        let flat = Frame { system: [5.0, 5.0, 5.0, 5.0], onto: [0.0, 0.0, 10.0, 10.0] };
        assert_eq!(map(&flat, [5, 5, 6, 6]), [0.0, 0.0, 1.0, 1.0]);
    }
}

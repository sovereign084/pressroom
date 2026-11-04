//! The text stream (`Quill/QuillSub/CONTENTS`): a table of named sections, one holding all the text
//! in UTF-16, others saying where each story ends, the formatting of runs and paragraphs, the style
//! sheet, the font names and the colours.

use std::collections::BTreeMap;

use crate::blocks::{self, Block, children, find, value};
use crate::bytes;

/// Most sections a table of contents may name (a real file has a few dozen).
const MAX_SECTIONS: usize = 100_000;
/// Formatting pages are 512 bytes: this is their whole size.
const FORMAT_PAGE: usize = 512;

#[derive(Clone, Copy, Debug)]
struct Section {
    id: u16,
    at: usize,
    len: usize,
}

/// Character formatting as written: `None` means inherited.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharProps {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<u32>,
    /// In EMU.
    pub size: Option<u32>,
    /// Index into the colour list.
    pub color: Option<u32>,
    /// The font for each kind of script: index into the font list, by slot.
    pub fonts: BTreeMap<u8, u32>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    pub superscript: Option<u32>,
    /// Character scale in tenths of a percent.
    pub scaling: Option<u32>,
    /// Publisher's tracking in tenths of a percent (1000 is normal).
    pub tracking: Option<u32>,
    /// Extra space between letters, in EMU.
    pub kerning: Option<i32>,
    pub language: Option<u32>,
    /// What kind of field the character stands for (5 is a page number).
    pub field: Option<u32>,
    /// Ids seen that nothing reads yet.
    pub unknown: Vec<u8>,
}

/// Paragraph formatting as written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParaProps {
    pub align: Option<u32>,
    /// Line spacing as written: see [`ParaProps::spacing`].
    pub line_spacing: Option<u32>,
    pub before: Option<u32>,
    pub after: Option<u32>,
    pub first_indent: Option<i32>,
    pub left_indent: Option<i32>,
    pub right_indent: Option<i32>,
    pub tabs: Vec<(i32, u32)>,
    pub drop_lines: Option<u32>,
    pub drop_chars: Option<u32>,
    /// A drop cap's own font and colour: indexes into the font and colour lists.
    pub drop_font: Option<u32>,
    pub drop_color: Option<u32>,
    pub style: Option<u32>,
    pub bullet: Option<u32>,
    pub keep_next: Option<bool>,
    pub keep_lines: Option<bool>,
    pub widow: Option<bool>,
    pub unknown: Vec<u8>,
}

/// A paragraph's line spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spacing {
    /// A multiple of the font's single spacing.
    Lines(f64),
    /// Exact points.
    Points(f64),
}

impl ParaProps {
    pub fn spacing(&self) -> Option<Spacing> {
        let v = self.line_spacing?;
        if v & 1 != 0 {
            Some(Spacing::Points(f64::from(v - 1) / 8.0 / 12700.0))
        } else if v & 2 != 0 {
            Some(Spacing::Lines(f64::from(v - 2) / (12700.0 * 96.0)))
        } else {
            None
        }
    }
}

/// A run of text with one formatting, in UTF-16 units from the start of its story.
#[derive(Clone, Debug, PartialEq)]
pub struct Run<P> {
    pub start: usize,
    pub end: usize,
    pub props: P,
}

#[derive(Clone, Debug, Default)]
pub struct Story {
    pub id: u32,
    pub text: Vec<u16>,
    pub paragraphs: Vec<Run<ParaProps>>,
    pub runs: Vec<Run<CharProps>>,
    /// Where each table cell's text ends, for a table's story.
    pub cell_ends: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct TextStream {
    pub stories: BTreeMap<u32, Story>,
    pub fonts: Vec<String>,
    /// Colour references, as written.
    pub colors: Vec<u32>,
    /// The names of colour-matching inks, by colour.
    pub color_inks: BTreeMap<u32, String>,
    /// The style sheet: (characters, paragraph) of each text style.
    pub styles: Vec<(CharProps, ParaProps)>,
    /// What could not be read, in words.
    pub problems: Vec<String>,
}

fn sections(q: &[u8]) -> BTreeMap<String, Vec<Section>> {
    let mut out: BTreeMap<String, Vec<Section>> = BTreeMap::new();
    let mut at = 0x18usize;
    let mut lists = 0usize;
    let mut total = 0usize;
    while at != 0xFFFF_FFFF {
        lists += 1;
        if lists > 4096 {
            break;
        }
        let (Some(n), Some(next)) = (bytes::u16_at(q, at.saturating_add(2)), bytes::u32_at(q, at.saturating_add(4))) else { break };
        for k in 0..n as usize {
            total += 1;
            if total > MAX_SECTIONS {
                return out;
            }
            let e = at + 8 + 24 * k;
            let Some(name) = q.get(e + 2..e + 6) else { break };
            let (Some(id), Some(off), Some(len)) = (bytes::u16_at(q, e + 6), bytes::u32_at(q, e + 16), bytes::u32_at(q, e + 20)) else { break };
            out.entry(String::from_utf8_lossy(name).into_owned()).or_default().push(Section { id, at: off as usize, len: len as usize });
        }
        at = next as usize;
    }
    out
}

/// The offsets of the formatting pages a bin table lists.
fn page_list(q: &[u8], bte: &Section) -> Vec<usize> {
    let Some(n) = bytes::u32_at(q, bte.at).map(|n| n as usize) else { return Vec::new() };
    if n > bte.len / 4 {
        return Vec::new();
    }
    let base = bte.at + 12 + 4 * (n + 1);
    (0..n).filter_map(|i| bytes::u32_at(q, base + 4 * i)).map(|v| v as usize).collect()
}

/// `(start in the stream's text, end, style's blocks)` of each run on the pages, as absolute offsets.
fn format_runs(q: &[u8], pages: &[usize], text_at: usize) -> Vec<(usize, usize, Vec<Block>)> {
    let mut out = Vec::new();
    let mut prev = text_at;
    for &page in pages {
        let Some(n) = bytes::u16_at(q, page).map(usize::from) else { continue };
        // Four bytes of end and two of offset per run after an eight byte head.
        if 8 + 6 * n > FORMAT_PAGE {
            continue;
        }
        for i in 0..n {
            let (Some(end), Some(off)) = (bytes::u32_at(q, page + 8 + 4 * i), bytes::u16_at(q, page + 8 + 4 * n + 2 * i)) else { break };
            let end = end as usize;
            let list = if off == 0 {
                Vec::new()
            } else {
                let at = page + off as usize;
                let len = bytes::u32_at(q, at).map_or(0, |l| l as usize);
                blocks::blocks(q, at + 4, at.saturating_add(len).min(page + FORMAT_PAGE))
            };
            if end >= prev {
                out.push((prev, end, list));
            }
            prev = end;
        }
    }
    out
}

pub fn char_props(q: &[u8], list: &[Block]) -> CharProps {
    let mut c = CharProps::default();
    for b in list {
        match b.id {
            0x02 => c.bold = Some(b.value != 0 || b.kind == 0x0A),
            // The second copies of bold, italic and size, the language for other scripts and the
            // colour as a fill: the same again, in other words.
            0x37 | 0x38 | 0x39 | 0x3D | 0x3E | 0x58 | 0x22 | 0x35 | 0x36 | 0x4B => {}
            0x03 => c.italic = Some(b.value != 0 || b.kind == 0x0A),
            0x1E => c.underline = Some(b.value & 0xFF),
            0x0C => c.size = Some(b.value),
            0x2E => c.color = Some(b.value),
            0x44 => c.color = value(&children(q, b), 0),
            0x24 => {
                for slot in children(q, b) {
                    if let Some(v) = children(q, &slot).first().map(|i| i.value) {
                        c.fonts.insert(slot.id, v);
                    }
                }
                // Publisher leaves the first slot out when it holds the first font on the list.
                // Filled in here, so a run that changes to that font overrides its style's.
                if !c.fonts.is_empty() {
                    c.fonts.entry(0).or_insert(0);
                }
            }
            0x00 => c.field = Some(b.value),
            // The size from which pairs of letters are kerned.
            0x18 => {}
            0x1B => c.kerning = Some(b.value as i32),
            0x0F => c.superscript = Some(b.value),
            0x13 => c.small_caps = Some(b.value != 0 || b.kind == 0x0A),
            0x14 => c.caps = Some(b.value != 0 || b.kind == 0x0A),
            0x20 => c.scaling = Some(b.value),
            0x1F => c.tracking = Some(b.value),
            0x12 => c.language = Some(b.value),
            id => c.unknown.push(id),
        }
    }
    c
}

impl CharProps {
    /// The font for ordinary letters.
    pub fn font(&self) -> Option<u32> {
        self.fonts.get(&0).copied()
    }
}

pub fn para_props(q: &[u8], list: &[Block]) -> ParaProps {
    let mut p = ParaProps::default();
    for b in list {
        match b.id {
            0x04 => p.align = Some(b.value & 0xFF),
            0x19 => p.style = Some(b.value),
            0x34 => p.line_spacing = Some(b.value),
            0x12 => p.before = Some(b.value),
            0x13 => p.after = Some(b.value),
            0x0C => p.first_indent = Some(b.value as i32),
            0x0D => p.left_indent = Some(b.value as i32),
            0x0E => p.right_indent = Some(b.value as i32),
            0x08 => p.drop_lines = Some(b.value),
            0x2D => p.drop_chars = Some(b.value),
            0x18 => p.keep_next = Some(true),
            0x1D => p.widow = Some(true),
            0x53 => p.drop_font = Some(b.value),
            0x56 => p.drop_color = value(&children(q, b), 0),
            0x06 | 0x30 | 0x3C | 0x5A | 0x2C => {}
            0x32 => p.tabs = tabs(q, b),
            id => p.unknown.push(id),
        }
    }
    p
}

fn tabs(q: &[u8], b: &Block) -> Vec<(i32, u32)> {
    let mut out = Vec::new();
    let inner = children(q, b);
    if let Some(array) = find(&inner, 0x28) {
        for t in children(q, array) {
            let fields = children(q, &t);
            if let Some(at) = value(&fields, 0) {
                out.push((at as i32, value(&fields, 1).unwrap_or(0)));
            }
        }
    }
    out
}

/// Fills in what `over` leaves open from `base`.
pub fn merge_char(over: &CharProps, base: &CharProps) -> CharProps {
    CharProps {
        bold: over.bold.or(base.bold),
        italic: over.italic.or(base.italic),
        underline: over.underline.or(base.underline),
        size: over.size.or(base.size),
        color: over.color.or(base.color),
        fonts: {
            let mut f = base.fonts.clone();
            f.extend(over.fonts.iter().map(|(k, v)| (*k, *v)));
            f
        },
        caps: over.caps.or(base.caps),
        small_caps: over.small_caps.or(base.small_caps),
        superscript: over.superscript.or(base.superscript),
        scaling: over.scaling.or(base.scaling),
        tracking: over.tracking.or(base.tracking),
        kerning: over.kerning.or(base.kerning),
        language: over.language.or(base.language),
        field: over.field.or(base.field),
        unknown: over.unknown.clone(),
    }
}

pub fn merge_para(over: &ParaProps, base: &ParaProps) -> ParaProps {
    ParaProps {
        align: over.align.or(base.align),
        line_spacing: over.line_spacing.or(base.line_spacing),
        before: over.before.or(base.before),
        after: over.after.or(base.after),
        first_indent: over.first_indent.or(base.first_indent),
        left_indent: over.left_indent.or(base.left_indent),
        right_indent: over.right_indent.or(base.right_indent),
        tabs: if over.tabs.is_empty() { base.tabs.clone() } else { over.tabs.clone() },
        drop_lines: over.drop_lines.or(base.drop_lines),
        drop_chars: over.drop_chars.or(base.drop_chars),
        drop_font: over.drop_font.or(base.drop_font),
        drop_color: over.drop_color.or(base.drop_color),
        style: over.style.or(base.style),
        bullet: over.bullet.or(base.bullet),
        keep_next: over.keep_next.or(base.keep_next),
        keep_lines: over.keep_lines.or(base.keep_lines),
        widow: over.widow.or(base.widow),
        unknown: over.unknown.clone(),
    }
}

fn read_fonts(q: &[u8], s: &Section) -> Vec<String> {
    let mut out = Vec::new();
    let Some(n) = bytes::u32_at(q, s.at + 4).map(|n| n as usize) else { return out };
    if n > s.len / 4 {
        return out;
    }
    let table = s.at + 20;
    for i in 0..n {
        let Some(off) = bytes::u32_at(q, table + 4 * i) else { break };
        let at = table + off as usize;
        let Some(len) = bytes::u16_at(q, at).map(usize::from) else {
            out.push(String::new());
            continue;
        };
        let name = q.get(at + 2..(at + 2).saturating_add(len * 2)).map(bytes::utf16_units).unwrap_or_default();
        out.push(String::from_utf16_lossy(&name));
    }
    out
}

fn read_colors(q: &[u8], s: &Section) -> (Vec<u32>, BTreeMap<u32, String>) {
    let mut out = Vec::new();
    let mut inks = BTreeMap::new();
    let Some(n) = bytes::u32_at(q, s.at).map(|n| n as usize) else { return (out, inks) };
    if n > s.len {
        return (out, inks);
    }
    let mut at = s.at + 12;
    let end = s.at.saturating_add(s.len);
    for _ in 0..n {
        let Some(len) = bytes::u32_at(q, at).map(|l| l as usize).filter(|&l| l >= 4) else { break };
        let list = blocks::blocks(q, at + 4, at.saturating_add(len).min(end));
        if let Some(c) = value(&list, 1) {
            // A colour-matching ink has its description somewhere in the entry's containers.
            let index = out.len() as u32;
            for b in &list {
                if b.container {
                    for inner in children(q, b) {
                        if inner.kind == 0xC0 {
                            let units = q.get(inner.body.clone()).map(bytes::utf16_units).unwrap_or_default();
                            let raw: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
                            if let Some(name) = crate::model::ink_name(&raw) {
                                inks.insert(index, name);
                            }
                        }
                    }
                }
            }
            out.push(c);
        }
        at = at.saturating_add(len);
    }
    (out, inks)
}

fn read_styles(q: &[u8], s: &Section) -> Vec<(CharProps, ParaProps)> {
    let Some(n) = bytes::u32_at(q, s.at + 4).map(|n| n as usize) else { return Vec::new() };
    if n > s.len / 4 {
        return Vec::new();
    }
    let table = s.at + 20;
    let entry = |i: usize| -> Vec<Block> {
        let Some(off) = bytes::u32_at(q, table + 4 * i) else { return Vec::new() };
        let at = table + off as usize + 2;
        let len = bytes::u32_at(q, at).map_or(0, |l| l as usize);
        blocks::blocks(q, at + 4, at.saturating_add(len).min(s.at.saturating_add(s.len)))
    };
    (0..n / 2).map(|i| (char_props(q, &entry(2 * i)), para_props(q, &entry(2 * i + 1)))).collect()
}

pub fn read(q: &[u8]) -> TextStream {
    let toc = sections(q);
    let mut out = TextStream::default();
    let section = |name: &str| toc.get(name).and_then(|v| v.first().copied());
    let Some(text) = section("TEXT") else {
        out.problems.push("The file's text couldn't be found.".into());
        return out;
    };
    let units = q.get(text.at..text.at.saturating_add(text.len)).map(bytes::utf16_units).unwrap_or_default();
    if units.len() * 2 != text.len {
        out.problems.push("The file's text is cut short.".into());
    }

    // Story ids and lengths, one after the other in the text.
    let ids: Vec<u32> = section("SYID")
        .map(|s| {
            let n = bytes::u32_at(q, s.at + 4).map_or(0, |n| n as usize).min(s.len / 4);
            (0..n).filter_map(|i| bytes::u32_at(q, s.at + 8 + 4 * i)).collect()
        })
        .unwrap_or_default();
    let lens: Vec<usize> = section("STRS")
        .map(|s| {
            let n = bytes::u32_at(q, s.at).map_or(0, |n| n as usize).min(s.len / 4);
            (0..n).filter_map(|i| bytes::u32_at(q, s.at + 12 + 4 * i)).map(|v| v as usize).collect()
        })
        .unwrap_or_default();
    if ids.len() != lens.len() {
        out.problems.push(format!("The file lists {} stories but gives the length of {}.", ids.len(), lens.len()));
    }

    if let Some(s) = section("FONT") {
        out.fonts = read_fonts(q, &s);
    }
    if let Some(s) = section("PL  ") {
        (out.colors, out.color_inks) = read_colors(q, &s);
    }
    if let Some(sheet) = toc.get("STSH").and_then(|v| v.get(1).copied()) {
        out.styles = read_styles(q, &sheet);
    }

    let char_pages = section("BTEC").map(|s| page_list(q, &s)).unwrap_or_default();
    let para_pages = section("BTEP").map(|s| page_list(q, &s)).unwrap_or_default();
    let char_runs = format_runs(q, &char_pages, text.at);
    let para_runs = format_runs(q, &para_pages, text.at);
    let unit = |abs: usize| abs.saturating_sub(text.at) / 2;

    let cells: BTreeMap<u16, Vec<usize>> = toc
        .get("TCD ")
        .map(|v| {
            v.iter()
                .map(|s| {
                    let n = bytes::u32_at(q, s.at).map_or(0, |m| (m as usize).saturating_add(1)).min(s.len / 4);
                    let ends = (0..n).filter_map(|i| bytes::u32_at(q, s.at + 12 + 4 * i)).map(|e| e as usize).collect();
                    (s.id, ends)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut start = 0usize;
    for (index, (&id, &len)) in ids.iter().zip(&lens).enumerate() {
        let end = start.saturating_add(len).min(units.len());
        let mut story = Story { id, text: units.get(start..end).unwrap_or(&[]).to_vec(), ..Story::default() };
        for (a, b, list) in &para_runs {
            let (a, b) = (unit(*a), unit(*b));
            if a < end && b > start {
                story.paragraphs.push(Run { start: a.max(start) - start, end: b.min(end) - start, props: para_props(q, list) });
            }
        }
        for (a, b, list) in &char_runs {
            let (a, b) = (unit(*a), unit(*b));
            if a < end && b > start {
                story.runs.push(Run { start: a.max(start) - start, end: b.min(end) - start, props: char_props(q, list) });
            }
        }
        if let Some(c) = u16::try_from(index).ok().and_then(|i| cells.get(&i)) {
            story.cell_ends = c.clone();
        }
        out.stories.insert(id, story);
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonsense_is_a_short_answer_not_a_crash() {
        for len in [0usize, 1, 7, 0x18, 0x20, 600] {
            let junk = vec![0xFFu8; len];
            let r = read(&junk);
            assert!(r.stories.is_empty());
            let zeros = vec![0u8; len];
            let _ = read(&zeros);
        }
    }

    /// Records of a font list: `(slot, font)` pairs, each slot a container holding the font's index.
    fn font_list(slots: &[(u8, u32)]) -> Vec<u8> {
        let mut inner = Vec::new();
        for (slot, font) in slots {
            let mut one = vec![0u8, 0x20];
            one.extend_from_slice(&font.to_le_bytes());
            inner.extend_from_slice(&[*slot, 0x80]);
            inner.extend_from_slice(&(one.len() as u32 + 4).to_le_bytes());
            inner.extend_from_slice(&one);
        }
        let mut out = vec![0x24u8, 0x80];
        out.extend_from_slice(&(inner.len() as u32 + 4).to_le_bytes());
        out.extend_from_slice(&inner);
        out
    }

    /// Found on a file where the fifth slot held another font: its text came out in Sylfaen, not Calibri.
    #[test]
    fn the_first_font_on_the_list_is_read_though_its_slot_is_left_out() {
        let q = font_list(&[(1, 0), (2, 0), (5, 1), (6, 2)]);
        let run = char_props(&q, &blocks::blocks(&q, 0, q.len()));
        assert_eq!(run.font(), Some(0));
        // A run changing to it overrides its style's font, too.
        let style = CharProps { fonts: BTreeMap::from([(0, 27), (1, 27), (5, 1)]), ..CharProps::default() };
        assert_eq!(merge_char(&run, &style).font(), Some(0));
    }

    #[test]
    fn line_spacing_reads_as_lines_or_points() {
        let lines = ParaProps { line_spacing: Some(2 + 12700 * 96), ..ParaProps::default() };
        assert_eq!(lines.spacing(), Some(Spacing::Lines(1.0)));
        let points = ParaProps { line_spacing: Some(1 + 8 * 12700 * 14), ..ParaProps::default() };
        assert_eq!(points.spacing(), Some(Spacing::Points(14.0)));
        assert_eq!(ParaProps::default().spacing(), None);
    }

    #[test]
    fn unknown_formatting_is_kept_for_the_report() {
        let d = [0x77, 0x22, 1, 0, 0, 0, 0x0C, 0x22, 0x50, 0x53, 0x02, 0];
        let list = blocks::blocks(&d, 0, d.len());
        let c = char_props(&d, &list);
        assert_eq!(c.unknown, vec![0x77]);
        assert_eq!(c.size, Some(0x25350));
    }
}

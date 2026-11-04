//! The drawing stream (`Escher/EscherStm`, in Office's drawing format, documented as [MS-ODRAW]):
//! each shape's type, place, fill, line and picture, stacked back to front. The picture store points
//! into a second stream, `Escher/EscherDelayStm`.

use std::collections::BTreeMap;

use crate::bytes;

/// Deepest group nesting followed.
const MAX_DEPTH: usize = 64;

const DGG: u16 = 0xF000;
const BSTORE: u16 = 0xF001;
const DG: u16 = 0xF002;
const SPGR: u16 = 0xF003;
const SP: u16 = 0xF004;
const FBSE: u16 = 0xF007;
const FSPGR: u16 = 0xF009;
const FSP: u16 = 0xF00A;
const FOPT: u16 = 0xF00B;
const CHILD_ANCHOR: u16 = 0xF00F;
const CLIENT_ANCHOR: u16 = 0xF010;
const CLIENT_DATA: u16 = 0xF011;
const TERTIARY: u16 = 0xF122;

/// One shape, or a group with its members.
#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub spid: u32,
    /// The shape type (1 rectangle, 3 ellipse, 202 text box, 20 line ...).
    pub kind: u16,
    pub flags: u32,
    /// Properties, by id without the two flag bits.
    pub props: BTreeMap<u16, u32>,
    pub tertiary: BTreeMap<u16, u32>,
    pub complex: BTreeMap<u16, Vec<u8>>,
    /// Left, top, right, bottom in EMU, measured from the middle of the page.
    pub client: Option<[i32; 4]>,
    /// The table it belongs to, for a table cell's fill or a table's border line, with the numbers
    /// that say which (a number left out is zero).
    pub table_part: Option<TablePart>,
    /// The same, in the group's own coordinates.
    pub child: Option<[i32; 4]>,
    /// A group's own coordinates.
    pub system: Option<[i32; 4]>,
    /// The record in `Contents` this shape belongs to.
    pub seq: Option<u32>,
    pub is_group: bool,
    pub members: Vec<Shape>,
}

impl Shape {
    pub fn flip_h(&self) -> bool {
        self.flags & 0x40 != 0
    }
    pub fn flip_v(&self) -> bool {
        self.flags & 0x80 != 0
    }
    pub fn prop(&self, id: u16) -> Option<u32> {
        self.props.get(&id).copied()
    }
    pub fn tert(&self, id: u16) -> Option<u32> {
        self.tertiary.get(&id).copied()
    }
}

/// A table cell's fill or a table's border line: drawing records with no box of their own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TablePart {
    pub table: u32,
    /// The cell, counted along the rows from 0, for a fill.
    pub cell: u32,
    pub is_line: bool,
    /// A line runs from this row and column to that row and column, on the grid's lines.
    pub row0: u32,
    pub col0: u32,
    pub row1: u32,
    pub col1: u32,
}

/// A picture in the store.
#[derive(Clone, Copy, Debug, Default)]
pub struct StoreEntry {
    /// Where its record starts in the delay stream.
    pub offset: u32,
    pub size: u32,
    /// Empty entries hold a place: picture numbers count them.
    pub used: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Drawings {
    pub store: Vec<StoreEntry>,
    /// Every drawing's top shape.
    pub tops: Vec<Shape>,
    /// How many shapes were read in all.
    pub count: usize,
}

struct Rec {
    kind: u16,
    instance: u16,
    body: std::ops::Range<usize>,
}

/// The records of one level. The drawing and drawing-group containers are followed by a four byte
/// trailer in Publisher's files.
fn records(d: &[u8], at: usize, end: usize) -> Vec<Rec> {
    let mut out = Vec::new();
    let end = end.min(d.len());
    let mut at = at;
    while let (Some(head), Some(kind), Some(len)) =
        (bytes::u16_at(d, at), bytes::u16_at(d, at.saturating_add(2)), bytes::u32_at(d, at.saturating_add(4)))
    {
        let Some(start) = at.checked_add(8).filter(|&s| s <= end) else { break };
        let Some(stop) = start.checked_add(len as usize).filter(|&s| s <= end) else { break };
        out.push(Rec { kind, instance: head >> 4, body: start..stop });
        at = stop;
        if matches!(kind, DGG | DG) {
            at = at.saturating_add(4);
        }
    }
    out
}

/// An anchor's fields: a length, then (field id, value) pairs.
fn anchor_fields(d: &[u8], body: &std::ops::Range<usize>) -> BTreeMap<u16, i32> {
    let mut out = BTreeMap::new();
    let mut at = body.start.saturating_add(4);
    while at.saturating_add(6) <= body.end {
        if let (Some(id), Some(v)) = (bytes::u16_at(d, at), bytes::i32_at(d, at + 2)) {
            out.insert(id, v);
        }
        at += 6;
    }
    out
}

fn read_props(d: &[u8], r: &Rec, s: &mut Shape) {
    let count = r.instance as usize;
    let mut at = r.body.start;
    let mut complex_at = r.body.start.saturating_add(count.saturating_mul(6));
    for _ in 0..count {
        let (Some(id), Some(v)) = (bytes::u16_at(d, at), bytes::u32_at(d, at.saturating_add(2))) else { break };
        if at.saturating_add(6) > r.body.end {
            break;
        }
        let pid = id & 0x3FFF;
        s.props.insert(pid, v);
        // A complex value: its bytes follow the table, in the table's order.
        if id & 0x8000 != 0 {
            let stop = complex_at.saturating_add(v as usize).min(r.body.end);
            if let Some(b) = d.get(complex_at..stop) {
                s.complex.insert(pid, b.to_vec());
            }
            complex_at = stop;
        }
        at += 6;
    }
}

fn read_tertiary(d: &[u8], r: &Rec, s: &mut Shape) {
    let mut at = r.body.start;
    let count = r.instance as usize;
    let mut complex_at = r.body.start.saturating_add(count.saturating_mul(6));
    for _ in 0..count {
        let (Some(id), Some(v)) = (bytes::u16_at(d, at), bytes::u32_at(d, at.saturating_add(2))) else { break };
        if at.saturating_add(6) > r.body.end {
            break;
        }
        let pid = id & 0x3FFF;
        s.tertiary.insert(pid, v);
        if id & 0x8000 != 0 {
            let stop = complex_at.saturating_add(v as usize).min(r.body.end);
            if let Some(b) = d.get(complex_at..stop) {
                s.complex.insert(pid | 0x4000, b.to_vec());
            }
            complex_at = stop;
        }
        at += 6;
    }
}

fn read_shape(d: &[u8], body: &std::ops::Range<usize>) -> Shape {
    let mut s = Shape::default();
    for r in records(d, body.start, body.end) {
        match r.kind {
            FSP => {
                s.kind = r.instance;
                s.spid = bytes::u32_at(d, r.body.start).unwrap_or(0);
                s.flags = bytes::u32_at(d, r.body.start.saturating_add(4)).unwrap_or(0);
                s.is_group = s.flags & 1 != 0;
            }
            FSPGR => {
                let at = r.body.start;
                if let (Some(a), Some(b), Some(c), Some(e)) = (
                    bytes::i32_at(d, at),
                    bytes::i32_at(d, at.saturating_add(4)),
                    bytes::i32_at(d, at.saturating_add(8)),
                    bytes::i32_at(d, at.saturating_add(12)),
                ) {
                    s.system = Some([a, b, c, e]);
                }
            }
            CHILD_ANCHOR => {
                let at = r.body.start;
                if let (Some(a), Some(b), Some(c), Some(e)) = (
                    bytes::i32_at(d, at),
                    bytes::i32_at(d, at.saturating_add(4)),
                    bytes::i32_at(d, at.saturating_add(8)),
                    bytes::i32_at(d, at.saturating_add(12)),
                ) {
                    s.child = Some([a, b, c, e]);
                }
            }
            CLIENT_ANCHOR => {
                let f = anchor_fields(d, &r.body);
                let field = |id: u16| f.get(&id).copied().unwrap_or(0);
                if let Some(&table) = f.get(&0x6802) {
                    let uf = |id: u16| u32::try_from(field(id)).unwrap_or(0);
                    let is_line = [0x2001, 0x2004, 0x2005, 0x2006, 0x2007].iter().any(|id| f.contains_key(id));
                    s.table_part = u32::try_from(table).ok().map(|table| TablePart {
                        table,
                        cell: uf(0x2003),
                        is_line,
                        row0: uf(0x2004),
                        col0: uf(0x2005),
                        row1: uf(0x2006),
                        col1: uf(0x2007),
                    });
                } else if !f.is_empty() {
                    // A value that is zero is left out: a box can start at the middle of the page.
                    s.client = Some([field(0x2001), field(0x2002), field(0x2003), field(0x2004)]);
                }
            }
            CLIENT_DATA => {
                let mut at = r.body.start.saturating_add(4);
                while at.saturating_add(6) <= r.body.end {
                    if bytes::u16_at(d, at) == Some(0x6801) {
                        s.seq = bytes::u32_at(d, at + 2);
                    }
                    at += 6;
                }
            }
            FOPT => read_props(d, &r, &mut s),
            TERTIARY => read_tertiary(d, &r, &mut s),
            _ => {}
        }
    }
    s
}

fn read_group(d: &[u8], body: &std::ops::Range<usize>, depth: usize, count: &mut usize) -> Shape {
    let mut group = Shape::default();
    let mut first = true;
    for r in records(d, body.start, body.end) {
        match r.kind {
            SP => {
                *count += 1;
                let s = read_shape(d, &r.body);
                if first {
                    // The group's own record comes first.
                    group = s;
                    group.is_group = true;
                    first = false;
                } else {
                    group.members.push(s);
                }
            }
            SPGR if depth < MAX_DEPTH => group.members.push(read_group(d, &r.body, depth + 1, count)),
            _ => {}
        }
    }
    group
}

pub fn read(d: &[u8]) -> Drawings {
    let mut out = Drawings::default();
    for r in records(d, 0, d.len()) {
        match r.kind {
            DGG => {
                for b in records(d, r.body.start, r.body.end) {
                    if b.kind == BSTORE {
                        for e in records(d, b.body.start, b.body.end) {
                            if e.kind == FBSE {
                                // Size at 20, the record's place in the delay stream at 28.
                                let size = bytes::u32_at(d, e.body.start.saturating_add(20)).unwrap_or(0);
                                let offset = bytes::u32_at(d, e.body.start.saturating_add(28)).unwrap_or(u32::MAX);
                                out.store.push(StoreEntry { offset, size, used: size > 0 && offset != u32::MAX });
                            }
                        }
                    }
                }
            }
            DG => {
                for g in records(d, r.body.start, r.body.end) {
                    if g.kind == SPGR {
                        let mut count = 0;
                        out.tops.push(read_group(d, &g.body, 0, &mut count));
                        out.count += count;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// What kind of picture a store record holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureKind {
    Png,
    Jpeg,
    Tiff,
    Dib,
    Emf,
    Wmf,
    Pict,
    Other,
}

/// A picture's bytes, ready to use: metafiles inflated, a bitmap given its file header.
#[derive(Clone, Debug)]
pub struct Picture {
    pub kind: PictureKind,
    pub data: Vec<u8>,
    /// A metafile's box in its own units and its size in EMU, as the record gives them: what a crop
    /// is measured against.
    pub meta: Option<([i32; 4], (i32, i32))>,
}

/// Reads the picture record at `offset` of the delay stream. `None` when it can't be read.
pub fn picture(delay: &[u8], offset: u32) -> Option<Picture> {
    let at = offset as usize;
    let head = bytes::u16_at(delay, at)?;
    let kind = bytes::u16_at(delay, at.checked_add(2)?)?;
    let len = bytes::u32_at(delay, at.checked_add(4)?)? as usize;
    let start = at.checked_add(8)?;
    let body = delay.get(start..start.checked_add(len)?)?;
    let instance = head >> 4;
    // One or two 16-byte ids come first; the second instance of each pair has two.
    let two = |one: u16| usize::from(instance != one);
    let (pk, skip) = match kind {
        0xF01E => (PictureKind::Png, 16 + 16 * two(0x6E0) + 1),
        0xF01D => (PictureKind::Jpeg, 16 + 16 * two(if instance & !1 == 0x6E2 { 0x6E2 } else { 0x46A }) + 1),
        0xF02A => (PictureKind::Jpeg, 16 + 16 * two(if instance & !1 == 0x6E2 { 0x6E3 } else { 0x46B }) + 1),
        0xF029 => (PictureKind::Tiff, 16 + 16 * two(0x6E4) + 1),
        0xF01F => (PictureKind::Dib, 16 + 16 * two(0x7A8) + 1),
        // A 34 byte metafile header follows the ids: compression at 32.
        0xF01A => (PictureKind::Emf, 16 + 16 * two(0x3D4) + 34),
        0xF01B => (PictureKind::Wmf, 16 + 16 * two(0x216) + 34),
        0xF01C => (PictureKind::Pict, 16 + 16 * two(0x542) + 34),
        _ => (PictureKind::Other, 0),
    };
    let data = body.get(skip..)?;
    let meta = if matches!(pk, PictureKind::Emf | PictureKind::Wmf | PictureKind::Pict) {
        // After the ids: the uncompressed size, the box, then the size.
        let head = skip.checked_sub(34)?;
        let field = |n: usize| bytes::i32_at(body, head + 4 + n * 4);
        Some(([field(0)?, field(1)?, field(2)?, field(3)?], (field(4)?, field(5)?)))
    } else {
        None
    };
    let data = match pk {
        PictureKind::Emf | PictureKind::Wmf | PictureKind::Pict => {
            let compression = *body.get(skip.checked_sub(2)?)?;
            if compression == 0 { miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, 256 * 1024 * 1024).ok()? } else { data.to_vec() }
        }
        PictureKind::Dib => bmp_file(data)?,
        _ => data.to_vec(),
    };
    Some(Picture { kind: pk, data, meta })
}

/// A bitmap in the store has no file header: this puts one on.
fn bmp_file(dib: &[u8]) -> Option<Vec<u8>> {
    let header = bytes::u32_at(dib, 0)?;
    let bits = bytes::u16_at(dib, 14)?;
    let used = bytes::u32_at(dib, 32)?;
    let colours = if used != 0 {
        used
    } else if bits <= 8 {
        1u32 << bits
    } else {
        0
    };
    let total = u32::try_from(dib.len().checked_add(14)?).ok()?;
    let pixels = 14u32.checked_add(header)?.checked_add(colours.checked_mul(4)?)?;
    let mut out = Vec::with_capacity(dib.len() + 14);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&pixels.to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: u16, instance: u16, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(instance << 4).to_le_bytes());
        v.extend_from_slice(&kind.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn a_shape_comes_back_with_its_place_and_properties() {
        let mut anchor = vec![28u8, 0, 0, 0];
        for (id, v) in [(0x2001u16, -100i32), (0x2002, -50), (0x2003, 100), (0x2004, 50)] {
            anchor.extend_from_slice(&id.to_le_bytes());
            anchor.extend_from_slice(&v.to_le_bytes());
        }
        let mut client = vec![10u8, 0, 0, 0];
        client.extend_from_slice(&0x6801u16.to_le_bytes());
        client.extend_from_slice(&302u32.to_le_bytes());
        let mut opt = Vec::new();
        opt.extend_from_slice(&0x0181u16.to_le_bytes());
        opt.extend_from_slice(&0x00112233u32.to_le_bytes());
        let mut sp = rec(FSP, 202, &[1, 4, 0, 0, 0, 0xA, 0, 0]);
        sp.extend(rec(FOPT, 1, &opt));
        sp.extend(rec(CLIENT_ANCHOR, 26, &anchor));
        sp.extend(rec(CLIENT_DATA, 26, &client));
        let mut group = rec(SP, 0, &rec(FSP, 0, &[0, 4, 0, 0, 5, 0, 0, 0]));
        group.extend(rec(SP, 0, &sp));
        let mut dg = rec(DG, 0, &rec(SPGR, 0, &group));
        dg.extend([0, 0, 0, 0]);
        let d = read(&dg);
        let top = d.tops.first().unwrap();
        let s = top.members.first().unwrap();
        assert_eq!(s.kind, 202);
        assert_eq!(s.client, Some([-100, -50, 100, 50]));
        assert_eq!(s.seq, Some(302));
        assert_eq!(s.prop(0x181), Some(0x00112233));
    }

    #[test]
    fn nested_groups_past_the_limit_stop_without_a_crash() {
        let mut body = rec(SP, 0, &rec(FSP, 0, &[0, 0, 0, 0, 1, 0, 0, 0]));
        for _ in 0..200 {
            let mut outer = rec(SP, 0, &rec(FSP, 0, &[0, 0, 0, 0, 1, 0, 0, 0]));
            outer.extend(rec(SPGR, 0, &body));
            body = outer;
        }
        let dg = rec(DG, 0, &rec(SPGR, 0, &body));
        let d = read(&dg);
        assert_eq!(d.tops.len(), 1);
    }

    #[test]
    fn bad_records_end_the_list_without_a_crash() {
        assert!(read(&[0xFF; 7]).tops.is_empty());
        assert!(read(&[0, 0, 0x02, 0xF0, 0xFF, 0xFF, 0xFF, 0xFF]).tops.is_empty());
        assert!(picture(&[0, 0], 0).is_none());
        assert!(picture(&[0xE0, 0x6E, 0x1E, 0xF0, 200, 0, 0, 0, 1, 2], 0).is_none());
        assert!(picture(&[1, 2, 3], u32::MAX).is_none());
    }

    #[test]
    fn a_png_comes_out_of_its_record() {
        let mut body = vec![0u8; 17];
        body.extend_from_slice(b"\x89PNG");
        let r = rec(0xF01E, 0x6E0, &body);
        let p = picture(&r, 0).unwrap();
        assert_eq!(p.kind, PictureKind::Png);
        assert_eq!(p.data, b"\x89PNG");
    }
}

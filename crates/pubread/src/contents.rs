//! The layout stream (`Contents`): the document's size, its pages and which shapes are on each, what
//! a shape's text is and how tables are divided. A directory near its end lists numbered chunks.

use std::collections::BTreeMap;

use crate::blocks::{self, Block, children, find, value};
use crate::{PubError, bytes};

/// Chunk kinds.
const SHAPE: u32 = 0x01;
const TABLE: u32 = 0x10;
const ALT_SHAPE: u32 = 0x20;
const GROUP: u32 = 0x30;
const LOGO: u32 = 0x31;
const PAGE: u32 = 0x43;
const DOCUMENT: u32 = 0x44;
const PALETTE: u32 = 0x5C;
const CELLS: u32 = 0x63;

/// One numbered chunk.
#[derive(Clone, Copy, Debug)]
pub struct Chunk {
    pub kind: u32,
    pub at: usize,
    pub parent: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct PageInfo {
    pub shapes: Vec<u32>,
    pub master: Option<u32>,
    /// The name a master page has ("A").
    pub name: String,
    pub is_master: bool,
}

/// What `Contents` says about a shape (its look is in the drawing stream).
#[derive(Clone, Debug, Default)]
pub struct ShapeInfo {
    pub kind: u32,
    pub story: Option<u32>,
    /// A linked box's place in its story: 0 first, 1 next, and so on.
    pub frame_index: u32,
    pub valign: Option<u32>,
    /// All the values, by id, for what isn't understood yet.
    pub raw: BTreeMap<u8, u32>,
    pub table: Option<TableInfo>,
}

#[derive(Clone, Debug, Default)]
pub struct TableInfo {
    /// Rows grow with their text.
    pub grows: bool,
    pub rows: u32,
    pub cols: u32,
    /// Column widths then row heights, in EMU.
    pub sizes: Vec<u32>,
    pub cells: Vec<CellSpan>,
    /// Each cell's fill, counted along the rows, with its ink's name if it has one.
    pub fills: BTreeMap<u32, (crate::model::Rgb, Option<String>)>,
    /// Lines along the rows, by (grid line, column): weight in points and colour.
    pub across: BTreeMap<(u32, u32), (f64, crate::model::Rgb)>,
    /// Lines along the columns, by (row, grid line).
    pub down: BTreeMap<(u32, u32), (f64, crate::model::Rgb)>,
}

/// A cell's first and last row and column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellSpan {
    pub row0: u32,
    pub row1: u32,
    pub col0: u32,
    pub col1: u32,
    /// 0 top, 1 middle, 2 bottom.
    pub valign: u32,
    /// Top, left, bottom and right in EMU, where the cell says.
    pub margins: [Option<u32>; 4],
}

#[derive(Clone, Debug, Default)]
pub struct Contents {
    pub chunks: BTreeMap<u32, Chunk>,
    pub width_emu: u32,
    pub height_emu: u32,
    /// Every page the document lists, master and working pages too.
    pub page_list: Vec<u32>,
    /// How many of them are the publication's pages.
    pub page_count: Option<u32>,
    pub pages: BTreeMap<u32, PageInfo>,
    pub shapes: BTreeMap<u32, ShapeInfo>,
    /// The scheme colours.
    pub palette: Vec<u32>,
    pub two_page_view: bool,
    /// The layout guides' margin in points.
    pub margin: Option<f64>,
}

fn top_blocks(d: &[u8], at: usize) -> Vec<Block> {
    let len = bytes::u32_at(d, at).map_or(0, |l| l as usize);
    blocks::blocks(d, at.saturating_add(4), at.saturating_add(len))
}

fn directory(d: &[u8]) -> Result<BTreeMap<u32, Chunk>, PubError> {
    let damaged = || PubError::Damaged("its page directory can't be read".into());
    let trailer = bytes::u32_at(d, 0x1A).ok_or_else(damaged)? as usize;
    let len = bytes::u32_at(d, trailer).ok_or_else(damaged)? as usize;
    let top = blocks::blocks(d, trailer.saturating_add(4), trailer.saturating_add(len));
    let dir = top.iter().find(|b| b.kind == 0x90).ok_or_else(damaged)?;
    let mut out = BTreeMap::new();
    for (seq, entry) in children(d, dir).iter().enumerate() {
        if !entry.container {
            continue;
        }
        let fields = children(d, entry);
        if let (Some(kind), Some(at), Ok(seq)) = (value(&fields, 2), value(&fields, 4), u32::try_from(seq)) {
            out.insert(seq, Chunk { kind, at: at as usize, parent: value(&fields, 5) });
        }
    }
    Ok(out)
}

pub fn read(d: &[u8]) -> Result<Contents, PubError> {
    match d.get(..4) {
        Some([0xE8, 0xAC, 0x2C, 0x00]) => {}
        Some([0xE8, 0xAC, 0x22, 0x00]) => return Err(PubError::TooOld),
        _ => return Err(PubError::NotPublisher),
    }
    let chunks = directory(d)?;
    let mut c = Contents { chunks: chunks.clone(), ..Contents::default() };

    let (doc_seq, doc) = chunks.iter().find(|(_, k)| k.kind == DOCUMENT).ok_or_else(|| PubError::Damaged("its document record is missing".into()))?;
    let _ = doc_seq;
    let list = top_blocks(d, doc.at);
    if let Some(size) = find(&list, 0x12) {
        let f = children(d, size);
        c.width_emu = value(&f, 1).unwrap_or(0);
        c.height_emu = value(&f, 2).unwrap_or(0);
    }
    if c.width_emu == 0 || c.height_emu == 0 {
        return Err(PubError::Damaged("it doesn't say how large its pages are".into()));
    }
    if let Some(pages) = find(&list, 0x02) {
        c.page_list = children(d, pages).iter().map(|b| b.value).collect();
    }
    c.page_count = value(&list, 0x23);
    c.two_page_view = blocks::has(&list, 0x06);

    let mut cells: BTreeMap<u32, Vec<CellSpan>> = BTreeMap::new();
    for (&seq, k) in &chunks {
        if k.kind == CELLS {
            cells.insert(seq, read_cells(d, k.at));
        }
    }
    for (&seq, k) in &chunks {
        match k.kind {
            PAGE => {
                c.pages.insert(seq, read_page(d, k.at));
            }
            SHAPE | ALT_SHAPE | GROUP | LOGO | TABLE => {
                let mut s = read_shape(d, k.at);
                s.kind = k.kind;
                if k.kind == TABLE {
                    s.table = read_table(d, k.at, &cells);
                }
                c.shapes.insert(seq, s);
            }
            PALETTE => {
                for b in top_blocks(d, k.at) {
                    if b.kind == 0xA0 {
                        for e in children(d, &b) {
                            let v = if e.container { value(&children(d, &e), 1) } else { None };
                            c.palette.push(v.unwrap_or(0));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(c)
}

fn read_page(d: &[u8], at: usize) -> PageInfo {
    let list = top_blocks(d, at);
    let mut p = PageInfo::default();
    if let Some(s) = find(&list, 0x02) {
        p.shapes = children(d, s).iter().map(|b| b.value).collect();
    }
    p.master = value(&list, 0x0D);
    if let Some(n) = find(&list, 0x0E) {
        let units = d.get(n.body.clone()).map(bytes::utf16_units).unwrap_or_default();
        let name: Vec<u16> = units.into_iter().take_while(|&u| u != 0).collect();
        p.name = String::from_utf16_lossy(&name);
        p.is_master = !p.name.is_empty();
    }
    p
}

fn read_shape(d: &[u8], at: usize) -> ShapeInfo {
    let list = top_blocks(d, at);
    let mut s =
        ShapeInfo { story: value(&list, 0x27), frame_index: value(&list, 0x28).unwrap_or(0), valign: value(&list, 0x35), ..ShapeInfo::default() };
    for b in &list {
        if !b.container {
            s.raw.insert(b.id, b.value);
        }
    }
    s
}

fn read_cells(d: &[u8], at: usize) -> Vec<CellSpan> {
    let list = top_blocks(d, at);
    let Some(array) = find(&list, 0x02) else { return Vec::new() };
    children(d, array)
        .iter()
        .map(|c| {
            let f = children(d, c);
            CellSpan {
                row0: value(&f, 1).unwrap_or(0),
                row1: value(&f, 2).unwrap_or(0),
                col0: value(&f, 3).unwrap_or(0),
                col1: value(&f, 4).unwrap_or(0),
                valign: value(&f, 7).unwrap_or(0),
                margins: [value(&f, 0x0C), value(&f, 0x0A), value(&f, 0x0D), value(&f, 0x0B)],
            }
        })
        .collect()
}

fn read_table(d: &[u8], at: usize, cells: &BTreeMap<u32, Vec<CellSpan>>) -> Option<TableInfo> {
    let list = top_blocks(d, at);
    let rows = value(&list, 0x66)?;
    let cols = value(&list, 0x67)?;
    let mut t = TableInfo { rows, cols, ..TableInfo::default() };
    if let Some(array) = find(&list, 0x6D) {
        for e in children(d, array) {
            if let Some(size) = value(&children(d, &e), 0x02) {
                t.sizes.push(size);
            }
        }
    }
    if let Some(seq) = value(&list, 0x6B) {
        t.cells = cells.get(&seq).cloned().unwrap_or_default();
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_files_and_older_publisher_files_say_so() {
        assert!(matches!(read(b"abcd"), Err(PubError::NotPublisher)));
        assert!(matches!(read(&[0xE8, 0xAC, 0x22, 0x00]), Err(PubError::TooOld)));
        assert!(matches!(read(&[0xE8, 0xAC, 0x2C, 0x00]), Err(PubError::Damaged(_))));
    }

    #[test]
    fn a_trailer_pointing_anywhere_is_an_error_not_a_crash() {
        for at in [0u32, 4, 1000, u32::MAX] {
            let mut d = vec![0xE8, 0xAC, 0x2C, 0x00];
            d.resize(64, 0);
            d[0x1A..0x1E].copy_from_slice(&at.to_le_bytes());
            assert!(read(&d).is_err());
        }
    }
}

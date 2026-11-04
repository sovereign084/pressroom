//! Publisher keeps some drawings as Windows metafiles (`.emf`), which PressRoom can't place. This
//! turns one into an SVG of the same polygons, lines and text, so it stays sharp at any size. The
//! record layouts are Microsoft's, documented as [MS-EMF].

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::bytes;

/// Most records read, and most marks written.
const MAX_RECORDS: usize = 2_000_000;
const MAX_POINTS: usize = 2_000_000;

#[derive(Clone, Debug)]
enum Object {
    Pen { width: i32, color: Option<u32> },
    Brush { color: Option<u32> },
    Font { height: i32, weight: i32, italic: bool, family: String },
}

/// The result: the SVG, and how many records that draw something were not understood.
#[derive(Clone, Debug, Default)]
pub struct Converted {
    pub svg: String,
    pub unread: usize,
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c >= ' ' => out.push(c),
            _ => {}
        }
    }
    out
}

/// `COLORREF` to SVG: red in the low byte.
fn css(c: u32) -> String {
    format!("#{:02x}{:02x}{:02x}", c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF)
}

fn points16(d: &[u8], at: usize, n: usize) -> Vec<(i32, i32)> {
    (0..n.min(MAX_POINTS))
        .map_while(|i| {
            let x = bytes::u16_at(d, at + i * 4)? as i16;
            let y = bytes::u16_at(d, at + i * 4 + 2)? as i16;
            Some((i32::from(x), i32::from(y)))
        })
        .collect()
}

fn points32(d: &[u8], at: usize, n: usize) -> Vec<(i32, i32)> {
    (0..n.min(MAX_POINTS)).map_while(|i| Some((bytes::i32_at(d, at + i * 8)?, bytes::i32_at(d, at + i * 8 + 4)?))).collect()
}

fn path_of(points: &[(i32, i32)], close: bool) -> String {
    let mut s = String::new();
    for (i, (x, y)) in points.iter().enumerate() {
        let _ = write!(s, "{}{x} {y} ", if i == 0 { "M" } else { "L" });
    }
    if close {
        s.push('Z');
    }
    s
}

struct State {
    objects: BTreeMap<u32, Object>,
    pen: Option<(i32, Option<u32>)>,
    brush: Option<u32>,
    font: (i32, i32, bool, String),
    text_color: u32,
    align: u32,
    /// Window origin and the scale to device units, when the file sets them.
    window: (i32, i32),
    window_ext: Option<(i32, i32)>,
}

/// The stock objects Windows keeps: selected by a number with the top bit set.
fn stock(index: u32, st: &mut State) {
    match index & 0x7FFF_FFFF {
        0 => st.brush = Some(0x00FF_FFFF),
        1 => st.brush = Some(0x00C0_C0C0),
        2 => st.brush = Some(0x0080_8080),
        3 => st.brush = Some(0x0040_4040),
        4 => st.brush = Some(0),
        5 => st.brush = None,
        6 => st.pen = Some((0, Some(0x00FF_FFFF))),
        7 => st.pen = Some((0, Some(0))),
        8 => st.pen = None,
        _ => {}
    }
}

fn paint(st: &State, close: bool) -> String {
    let fill = if close { st.brush.map_or_else(|| "none".to_string(), css) } else { "none".to_string() };
    match st.pen {
        Some((width, Some(c))) => format!("fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{}\"", css(c), width.max(1)),
        _ => format!("fill=\"{fill}\" stroke=\"none\""),
    }
}

/// Converts a metafile. `None` when it isn't one. `meta` is the box and size (in EMU) the picture
/// record gives: a crop is measured against them, so they win over the file's own tighter bounds.
pub fn to_svg(d: &[u8], meta: Option<([i32; 4], (i32, i32))>) -> Option<Converted> {
    if bytes::u32_at(d, 0) != Some(1) || d.get(40..44) != Some(b" EMF") {
        return None;
    }
    let bounds = [bytes::i32_at(d, 8)?, bytes::i32_at(d, 12)?, bytes::i32_at(d, 16)?, bytes::i32_at(d, 20)?];
    let frame = [bytes::i32_at(d, 24)?, bytes::i32_at(d, 28)?, bytes::i32_at(d, 32)?, bytes::i32_at(d, 36)?];
    let (w, h) = (i64::from(bounds[2]) - i64::from(bounds[0]), i64::from(bounds[3]) - i64::from(bounds[1]));
    if w <= 0 || h <= 0 {
        return None;
    }
    // The frame is in hundredths of a millimetre.
    let to_pt = 72.0 / 2540.0;
    let (pw, ph) = (f64::from(frame[2].saturating_sub(frame[0])) * to_pt, f64::from(frame[3].saturating_sub(frame[1])) * to_pt);
    let mut body = String::new();
    let mut st = State {
        objects: BTreeMap::new(),
        pen: Some((0, Some(0))),
        brush: Some(0x00FF_FFFF),
        font: (12, 400, false, "Arial".into()),
        text_color: 0,
        align: 0,
        window: (0, 0),
        window_ext: None,
    };
    let mut unread = 0usize;
    let mut at = 0usize;
    for _ in 0..MAX_RECORDS {
        let (Some(kind), Some(len)) = (bytes::u32_at(d, at), bytes::u32_at(d, at.saturating_add(4)).map(|n| n as usize)) else { break };
        if len < 8 || at.checked_add(len).is_none_or(|e| e > d.len()) {
            break;
        }
        let rec = at + 8;
        match kind {
            1 | 14 | 27 | 40 | 25 | 18 | 17 | 19 | 20 | 21 | 58 | 33 | 34 | 70 | 13 | 75 | 115 | 98 => {}
            // Window origin and extent.
            10 => st.window = (bytes::i32_at(d, rec).unwrap_or(0), bytes::i32_at(d, rec + 4).unwrap_or(0)),
            9 => st.window_ext = Some((bytes::i32_at(d, rec).unwrap_or(0), bytes::i32_at(d, rec + 4).unwrap_or(0))),
            22 => st.align = bytes::u32_at(d, rec).unwrap_or(0),
            24 => st.text_color = bytes::u32_at(d, rec).unwrap_or(0),
            37 => {
                let index = bytes::u32_at(d, rec).unwrap_or(0);
                if index & 0x8000_0000 != 0 {
                    stock(index, &mut st);
                } else {
                    match st.objects.get(&index).cloned() {
                        Some(Object::Pen { width, color }) => st.pen = Some((width, color)),
                        Some(Object::Brush { color }) => st.brush = color,
                        Some(Object::Font { height, weight, italic, family }) => st.font = (height, weight, italic, family),
                        None => {}
                    }
                }
            }
            38 => {
                // Handle, pen style (5 is no line), width as a point, colour.
                let index = bytes::u32_at(d, rec).unwrap_or(0);
                let style = bytes::u32_at(d, rec + 4).unwrap_or(0) & 0xF;
                let width = bytes::i32_at(d, rec + 8).unwrap_or(0);
                let color = bytes::u32_at(d, rec + 16).unwrap_or(0);
                st.objects.insert(index, Object::Pen { width, color: (style != 5).then_some(color) });
            }
            39 => {
                // Handle, brush style (1 is hollow), colour.
                let index = bytes::u32_at(d, rec).unwrap_or(0);
                let style = bytes::u32_at(d, rec + 4).unwrap_or(0);
                let color = bytes::u32_at(d, rec + 8).unwrap_or(0);
                st.objects.insert(index, Object::Brush { color: (style != 1).then_some(color) });
            }
            82 => {
                let index = bytes::u32_at(d, rec).unwrap_or(0);
                let height = bytes::i32_at(d, rec + 4).unwrap_or(12);
                let weight = bytes::i32_at(d, rec + 20).unwrap_or(400);
                let italic = d.get(rec + 24).copied().unwrap_or(0) != 0;
                let name = d.get(rec + 32..rec + 32 + 64).map(bytes::utf16_units).unwrap_or_default();
                let name: Vec<u16> = name.into_iter().take_while(|&u| u != 0).collect();
                st.objects.insert(index, Object::Font { height, weight, italic, family: String::from_utf16_lossy(&name) });
            }
            // Polygon, polyline: bounds then a count then points.
            3 | 4 | 86 | 87 => {
                let n = bytes::u32_at(d, rec + 16).unwrap_or(0) as usize;
                let points = if kind >= 86 { points16(d, rec + 20, n) } else { points32(d, rec + 20, n) };
                let close = matches!(kind, 3 | 86);
                let _ = writeln!(body, "<path d=\"{}\" {}/>", path_of(&points, close), paint(&st, close));
            }
            // Several polygons or polylines at once.
            7 | 8 | 90 | 91 => {
                let parts = bytes::u32_at(d, rec + 16).unwrap_or(0) as usize;
                let total = bytes::u32_at(d, rec + 20).unwrap_or(0) as usize;
                if parts > MAX_POINTS || total > MAX_POINTS {
                    unread += 1;
                } else {
                    let counts: Vec<usize> = (0..parts).filter_map(|i| bytes::u32_at(d, rec + 24 + i * 4)).map(|c| c as usize).collect();
                    let start = rec + 24 + parts * 4;
                    let all = if kind >= 90 { points16(d, start, total) } else { points32(d, start, total) };
                    let close = matches!(kind, 8 | 91);
                    let mut path = String::new();
                    let mut from = 0usize;
                    for c in counts {
                        if let Some(p) = all.get(from..from.saturating_add(c)) {
                            path.push_str(&path_of(p, close));
                        }
                        from = from.saturating_add(c);
                    }
                    let _ = writeln!(body, "<path d=\"{path}\" {} fill-rule=\"evenodd\"/>", paint(&st, close));
                }
            }
            // Rectangle and ellipse: a box.
            42 | 43 => {
                if let (Some(a), Some(b), Some(c), Some(e)) =
                    (bytes::i32_at(d, rec), bytes::i32_at(d, rec + 4), bytes::i32_at(d, rec + 8), bytes::i32_at(d, rec + 12))
                {
                    if kind == 43 {
                        let _ = writeln!(
                            body,
                            "<rect x=\"{a}\" y=\"{b}\" width=\"{}\" height=\"{}\" {}/>",
                            c.saturating_sub(a),
                            e.saturating_sub(b),
                            paint(&st, true)
                        );
                    } else {
                        let (cx, cy) = ((f64::from(a) + f64::from(c)) / 2.0, (f64::from(b) + f64::from(e)) / 2.0);
                        let _ = writeln!(
                            body,
                            "<ellipse cx=\"{cx}\" cy=\"{cy}\" rx=\"{}\" ry=\"{}\" {}/>",
                            (f64::from(c) - f64::from(a)) / 2.0,
                            (f64::from(e) - f64::from(b)) / 2.0,
                            paint(&st, true)
                        );
                    }
                }
            }
            84 => {
                // Bounds, mode, scales, then the text's place, length and where its characters are.
                let x = bytes::i32_at(d, rec + 28).unwrap_or(0);
                let y = bytes::i32_at(d, rec + 32).unwrap_or(0);
                let n = bytes::u32_at(d, rec + 36).unwrap_or(0) as usize;
                let off = bytes::u32_at(d, rec + 40).unwrap_or(0) as usize;
                let text = d
                    .get(at.saturating_add(off)..at.saturating_add(off).saturating_add(n.saturating_mul(2)).min(at + len))
                    .map(bytes::utf16_units)
                    .unwrap_or_default();
                let text = String::from_utf16_lossy(&text);
                if !text.trim().is_empty() {
                    let (height, weight, italic, family) = &st.font;
                    // A positive height is the font's cell, a negative one its size.
                    let size = if *height < 0 { f64::from(-*height) } else { f64::from(*height) * 0.8 };
                    // The top of the text unless the file says its baseline (24) or bottom (8).
                    let baseline = match st.align & 0x18 {
                        24 => f64::from(y),
                        8 => f64::from(y) - size * 0.2,
                        _ => f64::from(y) + size * 0.9,
                    };
                    let anchor = match st.align & 0x06 {
                        6 => "middle",
                        2 => "end",
                        _ => "start",
                    };
                    let _ = writeln!(
                        body,
                        "<text x=\"{x}\" y=\"{baseline}\" font-family=\"{}\" font-size=\"{size}\" font-weight=\"{weight}\" font-style=\"{}\" text-anchor=\"{anchor}\" fill=\"{}\">{}</text>",
                        escape(family),
                        if *italic { "italic" } else { "normal" },
                        css(st.text_color),
                        escape(&text)
                    );
                }
            }
            // Anything else that draws (bitmaps, curves, clipped paths) isn't read yet.
            2 | 5 | 6 | 54 | 76 | 77 | 80 | 81 | 85 | 88 | 89 | 92 | 93 | 59..=64 => unread += 1,
            _ => {}
        }
        at += len;
        if kind == 14 {
            break;
        }
    }
    let (mut ox, mut oy, mut vw, mut vh) = match st.window_ext {
        Some((ew, eh)) if ew > 0 && eh > 0 => (st.window.0, st.window.1, i64::from(ew), i64::from(eh)),
        _ => (bounds[0], bounds[1], w, h),
    };
    let (mut pw, mut ph) = (pw, ph);
    if let Some((b, (ew, eh))) = meta {
        let (mw, mh) = (i64::from(b[2]) - i64::from(b[0]), i64::from(b[3]) - i64::from(b[1]));
        if mw > 0 && mh > 0 && ew > 0 && eh > 0 {
            (ox, oy, vw, vh) = (b[0], b[1], mw, mh);
            (pw, ph) = (f64::from(ew) / 12700.0, f64::from(eh) / 12700.0);
        }
    }
    let svg =
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{pw}pt\" height=\"{ph}pt\" viewBox=\"{ox} {oy} {vw} {vh}\">\n{body}</svg>\n");
    Some(Converted { svg, unread })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: u32, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&kind.to_le_bytes());
        v.extend_from_slice(&((body.len() + 8) as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn header() -> Vec<u8> {
        let mut h = vec![0u8; 80];
        for (i, v) in [0i32, 0, 100, 50, 0, 0, 2540, 1270].iter().enumerate() {
            h[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        h[32..36].copy_from_slice(b" EMF");
        rec(1, &h)
    }

    #[test]
    fn a_polygon_comes_out_with_its_brush_and_size() {
        let mut d = header();
        let mut brush = 1u32.to_le_bytes().to_vec();
        brush.extend_from_slice(&0u32.to_le_bytes());
        brush.extend_from_slice(&0x0000_00FFu32.to_le_bytes());
        brush.extend_from_slice(&0u32.to_le_bytes());
        d.extend(rec(39, &brush));
        d.extend(rec(37, &1u32.to_le_bytes()));
        let mut poly = vec![0u8; 16];
        poly.extend_from_slice(&3u32.to_le_bytes());
        for v in [0i16, 0, 10, 0, 10, 10] {
            poly.extend_from_slice(&v.to_le_bytes());
        }
        d.extend(rec(86, &poly));
        d.extend(rec(14, &[]));
        let c = to_svg(&d, None).unwrap();
        assert!(c.svg.contains("M0 0 L10 0 L10 10 Z"), "{}", c.svg);
        assert!(c.svg.contains("fill=\"#ff0000\""), "{}", c.svg);
        assert!(c.svg.contains("width=\"72pt\""), "{}", c.svg);
        assert_eq!(c.unread, 0);
    }

    #[test]
    fn text_is_escaped_and_bad_files_are_refused() {
        assert!(to_svg(b"nothing", None).is_none());
        let mut d = header();
        let mut t = vec![0u8; 28];
        t.extend_from_slice(&1i32.to_le_bytes());
        t.extend_from_slice(&2i32.to_le_bytes());
        t.extend_from_slice(&2u32.to_le_bytes());
        t.extend_from_slice(&80u32.to_le_bytes());
        t.resize(72, 0);
        t.extend_from_slice(&[b'<', 0, b'&', 0]);
        d.extend(rec(84, &t));
        let c = to_svg(&d, None).unwrap();
        assert!(c.svg.contains("&lt;&amp;"), "{}", c.svg);
    }

    #[test]
    fn a_count_larger_than_the_file_does_not_allocate_or_crash() {
        let mut d = header();
        let mut poly = vec![0u8; 16];
        poly.extend_from_slice(&u32::MAX.to_le_bytes());
        d.extend(rec(86, &poly));
        let mut many = vec![0u8; 16];
        many.extend_from_slice(&u32::MAX.to_le_bytes());
        many.extend_from_slice(&u32::MAX.to_le_bytes());
        d.extend(rec(91, &many));
        let c = to_svg(&d, None).unwrap();
        assert_eq!(c.unread, 1);
    }
}

//! PressRoom's picture tools: Corrections and Recolor (`picture.adjust`), Reset Picture
//! (`picture.reset`) and Compress Pictures (`picture.compress`).
//!
//! A corrected picture is a new asset made from the placed one, which stays in the document
//! ([`designcraft_doc::Adjusted`]), so corrections can be changed or taken off without losing
//! anything. Compressing makes the pictures themselves smaller: fewer pixels where the page doesn't
//! need them, the cropped-away parts left out, photos as JPEG.

use std::sync::Arc;

use designcraft_doc::{Adjusted, Asset, AssetId, Content, Document, Graphic, ItemId};
use designcraft_geom::{Affine, Rect};
use designcraft_images::adjust::{self, Recolor};
use serde_json::{Value, json};

use super::{CommandSpec, bad, bool_or, cmd, f64_or, has_selection, str_param, targets};
use crate::{Result, Session};

/// Pictures larger than this many pixels are left alone (a corrected copy would need gigabytes).
const MAX_PIXELS: u64 = 120_000_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "picture.swap",
            "Swap Pictures",
            [],
            None,
            "{ids?} — the two picture boxes (the selection by default) exchange pictures, each keeping its place and size",
            has_selection,
            swap
        ),
        cmd!(
            "picture.adjust",
            "Picture Corrections",
            ["Object"],
            None,
            "{brightness? (-100..100 %), contrast? (-100..100 %), recolor?: none|grayscale|sepia|washout|blackWhite|dark|light, tint? (#rrggbb, for dark and light), ids?} → {changed}. Settings not given stay as they are.",
            has_selection,
            adjust_pictures
        ),
        cmd!(
            "picture.reset",
            "Reset Picture",
            ["Object"],
            None,
            "{ids?} → {changed}: takes corrections and recolouring off",
            has_selection,
            reset_pictures
        ),
        cmd!(
            "picture.compress",
            "Compress Pictures…",
            ["Object"],
            None,
            "{ppi? (target resolution, default 220), all? (every picture, else the selected ones), deleteCropped? (default true)} → {pictures, before, after} (bytes)",
            super::has_doc,
            compress_pictures
        ),
    ]
}

/// The corrections a picture shows: its own, or none when it's an original.
pub fn adjusted_of(doc: &Document, asset: AssetId) -> Option<&Adjusted> {
    doc.assets.get(&asset)?.adjusted.as_ref()
}

/// Whether `a` is a picture the tools can work on: pixels, not a PDF, drawing or EPS.
fn is_raster(a: &Asset) -> bool {
    !a.data.is_empty()
        && a.mime.starts_with("image/")
        && !designcraft_images::is_svg(&a.data)
        && !designcraft_images::is_eps(&a.data)
        && !designcraft_images::is_pdf(&a.data)
}

fn decode(a: &Asset) -> Option<adjust::RgbaImage> {
    if !is_raster(a) {
        return None;
    }
    let (w, h) = designcraft_images::pixel_size(&a.data)?;
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    designcraft_images::decode_rgba(&a.data)
}

fn recolor_of(adj: &Adjusted) -> Recolor {
    Recolor::from_name(&adj.recolor, adj.tint.unwrap_or([0, 0, 0])).unwrap_or_default()
}

fn neutral(adj: &Adjusted) -> bool {
    adj.brightness == 0 && adj.contrast == 0 && recolor_of(adj) == Recolor::None
}

/// The picture frames among `ids`: (item, its graphic).
/// Exchanges two picture boxes' pictures. Each box stays where it is.
fn swap(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "picture.swap";
    let ids = targets(s, p)?;
    let [a, b] = ids.as_slice() else { return Err(bad(ID, "select two pictures to swap")) };
    let (a, b) = (*a, *b);
    s.edit(|d, _| {
        let content = |d: &Document, id: ItemId| d.item(id).map(|it| it.content.clone()).filter(|c| matches!(c, Content::Graphic(_)));
        let (Some(ca), Some(cb)) = (content(d, a), content(d, b)) else { return Err(bad(ID, "select two pictures to swap")) };
        if let Some(it) = d.item_mut(a) {
            it.content = cb;
        }
        if let Some(it) = d.item_mut(b) {
            it.content = ca;
        }
        Ok(json!({"swapped": [a.0, b.0]}))
    })
}

fn pictures(d: &Document, ids: &[ItemId]) -> Vec<(ItemId, Graphic)> {
    ids.iter().filter_map(|id| d.item(*id).and_then(|it| it.graphic().cloned()).map(|g| (*id, g))).collect()
}

fn set_graphic(d: &mut Document, id: ItemId, g: Graphic) {
    if let Some(it) = d.item_mut(id) {
        it.content = Content::Graphic(g);
    }
}

/// `source` with `adj`'s corrections, as a new asset; `None` when it can't be read.
fn corrected(d: &mut Document, source: AssetId, adj: &Adjusted) -> Option<AssetId> {
    let src = d.assets.get(&source)?.clone();
    let mut img = decode(&src)?;
    adjust::adjust(&mut img, adj.brightness, adj.contrast, recolor_of(adj));
    let (bytes, mime) = adjust::encode(&img, src.mime == "image/jpeg")?;
    let fresh = AssetId(d.alloc());
    let name = src.name.clone();
    Some(d.add_asset(Asset {
        id: fresh,
        name,
        mime: mime.into(),
        data: Arc::new(bytes),
        pixels: Some(img.dimensions()),
        adjusted: Some(Adjusted { source, ..adj.clone() }),
        ..Default::default()
    }))
}

fn tint_param(p: &Value) -> std::result::Result<Option<[u8; 3]>, String> {
    let Some(s) = str_param(p, "tint") else { return Ok(None) };
    let hex = s.trim_start_matches('#');
    let byte = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
    match (hex.len(), byte(0), byte(2), byte(4)) {
        (6, Some(r), Some(g), Some(b)) => Ok(Some([r, g, b])),
        _ => Err(format!("tint `{s}` isn't a colour like #1f5f7a")),
    }
}

fn percent(p: &Value, key: &str) -> std::result::Result<Option<i32>, String> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_f64() {
            Some(n) if n.is_finite() && (-100.0..=100.0).contains(&n) => Ok(Some(n.round() as i32)),
            _ => Err(format!("{key} must be a number from -100 to 100")),
        },
    }
}

/// Runs `f` on a copy of the document and keeps the copy (one undo step) only when it changed
/// something: `f` gives the command's result and how many pictures changed.
fn edit_if_changed(s: &mut Session, f: impl FnOnce(&mut Document) -> Result<(Value, usize)>) -> Result<Value> {
    let mut copy = (*s.doc()?.doc).clone();
    let (out, changed) = f(&mut copy)?;
    if changed > 0 {
        s.edit(|d, _| {
            *d = copy;
            Ok(())
        })?;
    }
    Ok(out)
}

fn adjust_pictures(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "picture.adjust";
    let brightness = percent(p, "brightness").map_err(|e| bad(ID, e))?;
    let contrast = percent(p, "contrast").map_err(|e| bad(ID, e))?;
    let tint = tint_param(p).map_err(|e| bad(ID, e))?;
    let recolor = match str_param(p, "recolor") {
        Some(r) => {
            let c = Recolor::from_name(r, tint.unwrap_or([0, 0, 0])).ok_or_else(|| bad(ID, format!("unknown recolor `{r}`")))?;
            if matches!(c, Recolor::Dark(_) | Recolor::Light(_)) && tint.is_none() {
                return Err(bad(ID, "dark and light recolours need a tint"));
            }
            Some(c)
        }
        None => None,
    };
    let ids = targets(s, p)?;
    edit_if_changed(s, |d| {
        let found = pictures(d, &ids);
        if found.is_empty() {
            return Err(bad(ID, "select a picture"));
        }
        let mut changed = 0;
        for (id, mut g) in found {
            let current = adjusted_of(d, g.asset).cloned();
            let source = current.as_ref().map_or(g.asset, |a| a.source);
            let mut next = current.unwrap_or_default();
            next.source = source;
            if let Some(b) = brightness {
                next.brightness = b;
            }
            if let Some(c) = contrast {
                next.contrast = c;
            }
            if let Some(r) = recolor {
                next.recolor = if r == Recolor::None { String::new() } else { r.name().into() };
                next.tint = r.tint();
            }
            let asset = if neutral(&next) {
                if !d.assets.contains_key(&source) {
                    continue;
                }
                source
            } else {
                match corrected(d, source, &next) {
                    Some(a) => a,
                    None => continue,
                }
            };
            if asset != g.asset {
                g.asset = asset;
                set_graphic(d, id, g);
                changed += 1;
            }
        }
        Ok((json!({"changed": changed}), changed))
    })
}

fn reset_pictures(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = targets(s, p)?;
    edit_if_changed(s, |d| {
        let mut changed = 0;
        for (id, mut g) in pictures(d, &ids) {
            if let Some(source) = adjusted_of(d, g.asset).map(|a| a.source).filter(|s| d.assets.contains_key(s)) {
                g.asset = source;
                set_graphic(d, id, g);
                changed += 1;
            }
        }
        Ok((json!({"changed": changed}), changed))
    })
}

/// Every picture frame on the document's spreads and parents, in groups too.
fn all_pictures(d: &Document) -> Vec<ItemId> {
    fn walk(it: &designcraft_doc::Item, out: &mut Vec<ItemId>, depth: usize) {
        if depth > 32 {
            return;
        }
        if it.graphic().is_some() {
            out.push(it.id);
        }
        for c in it.children() {
            walk(c, out, depth + 1);
        }
    }
    let mut out = Vec::new();
    for sp in d.spreads.iter().chain(&d.parents) {
        for it in &sp.items {
            walk(it, &mut out, 0);
        }
    }
    out
}

/// How much a picture is enlarged on the page, across and down (points of page per point of picture).
fn page_scale(d: &Document, id: ItemId, g: &Graphic) -> Option<(f64, f64)> {
    let loc = d.find(id)?;
    let m = d.parent_xf(&loc) * d.item(id)?.xf * g.xf;
    let [a, b, c, dd, _, _] = m.as_coeffs();
    let (sx, sy) = (a.hypot(b), c.hypot(dd));
    (sx.is_finite() && sy.is_finite() && sx > 1e-9 && sy > 1e-9).then_some((sx, sy))
}

/// The part of the picture its frame shows, in picture points; `None` when it's all of it.
fn shown_part(d: &Document, id: ItemId, g: &Graphic) -> Option<Rect> {
    if g.clip.is_some() || !g.wrap.is_default() || g.xf.determinant().abs() < 1e-12 {
        return None;
    }
    let inner = d.item(id)?.inner_bounds();
    let whole = Rect::new(0.0, 0.0, g.size.0, g.size.1);
    let shown = g.xf.inverse().transform_rect_bbox(inner).intersect(whole);
    let area = |r: Rect| r.width().max(0.0) * r.height().max(0.0);
    (area(shown) > 0.0 && area(shown) < area(whole) * 0.98).then_some(shown)
}

/// One picture made smaller: its new graphic and the bytes before and after.
fn compress_one(d: &mut Document, id: ItemId, g: &Graphic, ppi: f64, delete_cropped: bool) -> Option<(Graphic, usize, usize)> {
    let placed = d.assets.get(&g.asset)?.clone();
    let adj = placed.adjusted.clone();
    let base = match &adj {
        Some(a) => d.assets.get(&a.source)?.clone(),
        None => placed.clone(),
    };
    let before = placed.data.len() + adj.as_ref().map_or(0, |_| base.data.len());
    let img = decode(&base)?;
    let (bw, bh) = img.dimensions();
    if g.size.0 <= 0.0 || g.size.1 <= 0.0 || bw == 0 || bh == 0 {
        return None;
    }
    let (px_x, px_y) = (f64::from(bw) / g.size.0, f64::from(bh) / g.size.1);
    // Leave out what the frame crops away.
    let (img, part) = match shown_part(d, id, g).filter(|_| delete_cropped) {
        Some(r) => {
            let (x0, y0) = ((r.x0 * px_x).floor().max(0.0) as u32, (r.y0 * px_y).floor().max(0.0) as u32);
            let (x1, y1) = ((r.x1 * px_x).ceil().min(f64::from(bw)) as u32, (r.y1 * px_y).ceil().min(f64::from(bh)) as u32);
            let cut = adjust::crop(&img, x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0));
            let (cw, ch) = cut.dimensions();
            // The part in picture points, on whole pixels.
            let part = Rect::new(f64::from(x0) / px_x, f64::from(y0) / px_y, f64::from(x0 + cw) / px_x, f64::from(y0 + ch) / px_y);
            (cut, Some(part))
        }
        None => (img, None),
    };
    // Fewer pixels where the page doesn't need them.
    let (sx, sy) = page_scale(d, id, g)?;
    let (ppi_x, ppi_y) = (px_x * 72.0 / sx, px_y * 72.0 / sy);
    let keep = (ppi / ppi_x).max(ppi / ppi_y);
    let (w, h) = img.dimensions();
    let img = if keep < 0.95 { adjust::resize(&img, (f64::from(w) * keep).round() as u32, (f64::from(h) * keep).round() as u32) } else { img };
    let resampled = img.dimensions() != (w, h);
    // A photo saved as a PNG or bitmap becomes a JPEG even at its size.
    let photo_as_png = base.mime != "image/jpeg" && adjust::is_opaque(&img) && img.width() * img.height() > 250_000;
    if part.is_none() && !resampled && !photo_as_png {
        return None;
    }
    let (bytes, mime) = adjust::encode(&img, true)?;
    let mut out = g.clone();
    let mut after = bytes.len();
    let new_base = Asset {
        id: AssetId(0),
        name: base.name.clone(),
        mime: mime.into(),
        data: Arc::new(bytes),
        pixels: Some(img.dimensions()),
        ..Default::default()
    };
    let corrected_img = adj.as_ref().map(|a| {
        let mut c = img.clone();
        adjust::adjust(&mut c, a.brightness, a.contrast, recolor_of(a));
        c
    });
    let corrected_bytes = match corrected_img.as_ref() {
        Some(c) => Some(adjust::encode(c, true)?),
        None => None,
    };
    after += corrected_bytes.as_ref().map_or(0, |(b, _)| b.len());
    if after >= before {
        return None;
    }
    let fresh = AssetId(d.alloc());
    let base_id = d.add_asset(Asset { id: fresh, ..new_base });
    out.asset = match (adj, corrected_bytes) {
        (Some(a), Some((bytes, mime))) => {
            let fresh = AssetId(d.alloc());
            let pixels = corrected_img.map(|c| c.dimensions());
            d.add_asset(Asset {
                id: fresh,
                name: base.name.clone(),
                mime: mime.into(),
                data: Arc::new(bytes),
                pixels,
                adjusted: Some(Adjusted { source: base_id, ..a }),
                ..Default::default()
            })
        }
        _ => base_id,
    };
    if let Some(r) = part {
        out.size = (r.width(), r.height());
        out.xf = g.xf * Affine::translate((r.x0, r.y0));
    }
    Some((out, before, after))
}

fn compress_pictures(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "picture.compress";
    let ppi = f64_or(p, "ppi", 220.0);
    if !(ppi.is_finite() && (36.0..=2400.0).contains(&ppi)) {
        return Err(bad(ID, "ppi must be from 36 to 2400"));
    }
    let all = bool_or(p, "all", false);
    let delete_cropped = bool_or(p, "deleteCropped", true);
    let ids = if all { all_pictures(&s.doc()?.doc) } else { targets(s, p)? };
    edit_if_changed(s, |d| {
        let (mut n, mut before, mut after) = (0, 0usize, 0usize);
        for (id, g) in pictures(d, &ids) {
            if let Some((ng, b, a)) = compress_one(d, id, &g, ppi, delete_cropped) {
                set_graphic(d, id, ng);
                n += 1;
                before += b;
                after += a;
            }
        }
        Ok((json!({"pictures": n, "before": before, "after": after}), n))
    })
}

#[cfg(test)]
mod swap_tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn swap_needs_two_pictures() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let a = s.execute("frame.create", &json!({"rect": [10, 10, 50, 50], "content": "graphic"})).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("frame.create", &json!({"rect": [60, 10, 90, 50], "content": "graphic"})).unwrap()["id"].as_u64().unwrap();
        // Empty boxes hold no picture to swap, and one box isn't two.
        assert!(s.execute("picture.swap", &json!({"ids": [a, b]})).is_err());
        assert!(s.execute("picture.swap", &json!({"ids": [a]})).is_err());
    }
}

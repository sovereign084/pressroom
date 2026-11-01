//! The small marks Publisher draws on the page, which aren't printed: the overflow mark on a text
//! box whose text doesn't fit, the arrows on a linked box that go to the next and previous box, and
//! the picture icon in an empty picture box. The canvas draws them; a click on one acts here.

use designcraft_doc::{Content, Document, Item, ItemId};
use egui::{Color32, Rect, Stroke, pos2, vec2};
use serde_json::json;

use crate::DesignApp;
use crate::canvas::Xf;
use designcraft_tools::CanvasLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Overflow(ItemId),
    Next(ItemId),
    Previous(ItemId),
    Placeholder(ItemId),
}

/// This frame's marks and where they are on screen, for the click after the canvas has drawn.
static MARKS: std::sync::Mutex<Vec<(Rect, Mark)>> = std::sync::Mutex::new(Vec::new());

const SIZE: f32 = 15.0;
/// The picture icon in an empty picture box.
const ICON: f32 = 34.0;

/// Whether an item is a picture box still waiting for its picture.
fn is_placeholder(it: &Item) -> bool {
    matches!(it.content, Content::Unassigned) && it.object_style == designcraft_doc::BASIC_GRAPHICS_FRAME
}

fn button(painter: &egui::Painter, c: egui::Pos2, hot: bool) -> Rect {
    let r = Rect::from_center_size(c, vec2(SIZE, SIZE));
    painter.rect_filled(r, 2.0, if hot { Color32::from_rgb(230, 240, 250) } else { Color32::WHITE });
    painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_rgb(77, 115, 153)), egui::StrokeKind::Inside);
    r
}

/// Draws one spread's marks. Publisher shows them whatever Boundaries is set to.
pub(crate) fn draw(
    app: &DesignApp,
    painter: &egui::Painter,
    xf: &Xf,
    doc: &Document,
    layout: &CanvasLayout,
    items: &[std::sync::Arc<Item>],
    first: bool,
) {
    let mut marks = MARKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if first {
        marks.clear();
    }
    let pointer = painter.ctx().pointer_hover_pos();
    let hot = |r: Rect| pointer.is_some_and(|p| r.contains(p));
    let selected: Vec<ItemId> = app.session.active().map_or(vec![], |st| {
        let mut v = st.selection.items.clone();
        v.extend(st.selection.text.and_then(|t| t.frame));
        v
    });
    let mut walk: Vec<&Item> = items.iter().map(AsRef::as_ref).collect();
    let mut depth = 0;
    while let Some(it) = walk.pop() {
        if it.hidden {
            continue;
        }
        if let Content::Group { items } = &it.content {
            depth += 1;
            if depth < 2000 {
                walk.extend(items.iter().map(AsRef::as_ref));
            }
            continue;
        }
        let port = |out: bool| designcraft_tools::thread::port_center(doc, layout, it.id, out, xf.zoom).map(|c| xf.to_screen(c));
        if is_placeholder(it)
            && let Some(loc) = doc.find(it.id)
        {
            let m = layout.xf(loc.spread) * doc.parent_xf(&loc) * it.xf;
            let b = it.bounds();
            let centre = xf.to_screen(m * designcraft_geom::Point::new((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0));
            let r = Rect::from_center_size(centre, vec2(ICON, ICON));
            super::art::paint(painter, r, "picture", true);
            marks.push((r, Mark::Placeholder(it.id)));
            continue;
        }
        let Content::Text(tf) = &it.content else { continue };
        let Some(story) = doc.story(tf.story) else { continue };
        let k = story.frames.iter().position(|f| *f == it.id);
        let last = k.is_some_and(|k| k + 1 == story.frames.len());
        if last
            && app.session.cache.get(doc, tf.story, None).is_overset()
            && let Some(c) = port(true)
        {
            let r = button(painter, c, hot(Rect::from_center_size(c, vec2(SIZE, SIZE))));
            let red = Color32::from_rgb(200, 30, 30);
            for dx in [-3.5, 0.0, 3.5] {
                painter.circle_filled(pos2(r.center().x + dx, r.center().y), 1.2, red);
            }
            marks.push((r, Mark::Overflow(it.id)));
        }
        if !selected.contains(&it.id) {
            continue;
        }
        let ink = Color32::from_rgb(40, 70, 100);
        let arrow = |r: Rect, right: bool| {
            let (a, b) = if right {
                (r.center() - vec2(4.0, 0.0), r.center() + vec2(4.0, 0.0))
            } else {
                (r.center() + vec2(4.0, 0.0), r.center() - vec2(4.0, 0.0))
            };
            let d = if right { -3.0 } else { 3.0 };
            let s = Stroke::new(1.3, ink);
            painter.line_segment([a, b], s);
            painter.line_segment([b, b + vec2(d, -3.0)], s);
            painter.line_segment([b, b + vec2(d, 3.0)], s);
        };
        if k.is_some_and(|k| k + 1 < story.frames.len())
            && let Some(c) = port(true)
        {
            let r = button(painter, c, hot(Rect::from_center_size(c, vec2(SIZE, SIZE))));
            arrow(r, true);
            marks.push((r, Mark::Next(it.id)));
        }
        if k.is_some_and(|k| k > 0)
            && let Some(c) = port(false)
        {
            let r = button(painter, c, hot(Rect::from_center_size(c, vec2(SIZE, SIZE))));
            arrow(r, false);
            marks.push((r, Mark::Previous(it.id)));
        }
    }
}

/// Acts on a click on a mark, after the canvas has had the press (and selected what was under it).
pub(super) fn clicks(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(p) = ctx.input(|i| if i.pointer.primary_clicked() { i.pointer.interact_pos() } else { None }) else { return };
    if app.ui.dialog.is_some() {
        return;
    }
    let hit = MARKS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().find(|(r, _)| r.contains(p)).map(|(_, m)| *m);
    let Some(mark) = hit else { return };
    let select = |app: &mut DesignApp, id: ItemId| {
        let _ = app.run("selection.set", json!({"ids": [id.0]}));
    };
    match mark {
        Mark::Overflow(id) => {
            select(app, id);
            super::start_link(app);
        }
        Mark::Next(id) => {
            select(app, id);
            super::follow_link(app, true);
        }
        Mark::Previous(id) => {
            select(app, id);
            super::follow_link(app, false);
        }
        Mark::Placeholder(id) => {
            select(app, id);
            super::insert_picture(app);
        }
    }
}

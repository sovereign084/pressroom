//! Edit Points (PressRoom, as Publisher's): the selected shape's own points to drag. Ctrl and a
//! click on its outline adds a point there; Ctrl, Shift and a click on a point takes it away.
//! Esc, or a click away from the shape, goes back to selecting. The changes are the path
//! commands (`path.moveAnchors`, `path.addAnchor`, `path.deleteAnchor`), one step each.

use designcraft_doc::{Item, ItemId};
use designcraft_geom::{Affine, BezPath, Point};
use serde_json::json;

use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

/// How near the pointer must be to a point or the outline, in screen pixels.
const REACH_PX: f64 = 5.0;

#[derive(Default)]
pub struct ShapePointsTool {
    /// The point being dragged: (shape, subpath, anchor, where the press was, where it is now),
    /// canvas coordinates.
    drag: Option<(ItemId, usize, usize, Point, Point)>,
}

/// The shape whose points are shown: the one selected, its inner space → canvas, and the
/// spread → canvas transform.
fn target<'a>(cx: &'a ToolContext) -> Option<(ItemId, &'a Item, Affine, Affine)> {
    let [id] = cx.selection.items.as_slice() else { return None };
    let it = cx.doc.item(*id)?;
    let loc = cx.doc.find(*id)?;
    let spread = cx.layout.xf(loc.spread);
    Some((*id, it, cx.item_canvas_xf(*id)? * it.xf, spread))
}

/// Every anchor of the shape: (subpath, anchor, canvas point).
fn anchors(it: &Item, m: Affine) -> Vec<(usize, usize, Point)> {
    it.path.subpaths.iter().enumerate().flat_map(|(si, sp)| sp.anchors.iter().enumerate().map(move |(ai, a)| (si, ai, m * a.p))).collect()
}

fn anchor_at(cx: &ToolContext, it: &Item, m: Affine, p: Point) -> Option<(usize, usize)> {
    let reach = cx.tol(REACH_PX);
    anchors(it, m)
        .into_iter()
        .filter(|a| (a.2 - p).hypot() <= reach)
        .min_by(|a, b| (a.2 - p).hypot().total_cmp(&(b.2 - p).hypot()))
        .map(|a| (a.0, a.1))
}

/// Whether `p` is on the shape's outline (within reach), canvas coordinates.
fn on_outline(cx: &ToolContext, it: &Item, m: Affine, p: Point) -> bool {
    let reach = cx.tol(REACH_PX);
    let mut near = false;
    let mut last: Option<Point> = None;
    let mut first: Option<Point> = None;
    designcraft_geom::kurbo::flatten(m * it.path.to_bezpath(), 0.25, |el| {
        let seg = |a: Point, b: Point| {
            let d = b - a;
            let len2 = d.hypot2();
            let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
            (p - (a + d * t)).hypot() <= reach
        };
        match el {
            designcraft_geom::PathEl::MoveTo(q) => {
                last = Some(q);
                first = Some(q);
            }
            designcraft_geom::PathEl::LineTo(q) => {
                near |= last.is_some_and(|a| seg(a, q));
                last = Some(q);
            }
            designcraft_geom::PathEl::ClosePath => {
                near |= last.zip(first).is_some_and(|(a, b)| seg(a, b));
            }
            _ => {}
        }
    });
    near
}

impl Tool for ShapePointsTool {
    fn id(&self) -> &'static str {
        "shapePoints"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let Some((id, it, m, spread)) = target(cx) else {
            self.drag = None;
            return vec![Action::SwitchTool("selection".into())];
        };
        match ev.kind {
            PointerKind::Down => {
                if let Some((si, ai)) = anchor_at(cx, it, m, ev.pos) {
                    if ev.mods.cmd && ev.mods.shift {
                        let count: usize = it.path.subpaths.iter().map(|s| s.anchors.len()).sum();
                        if count > 2 {
                            return vec![Action::Exec("path.deleteAnchor".into(), json!({"id": id.0, "subpath": si, "anchor": ai}))];
                        }
                        return vec![Action::Handled];
                    }
                    self.drag = Some((id, si, ai, ev.pos, ev.pos));
                    return vec![Action::Handled];
                }
                if ev.mods.cmd && on_outline(cx, it, m, ev.pos) {
                    let at = spread.inverse() * ev.pos;
                    return vec![Action::Exec("path.addAnchor".into(), json!({"id": id.0, "at": [at.x, at.y], "tolerance": cx.tol(REACH_PX)}))];
                }
                vec![Action::SwitchTool("selection".into())]
            }
            PointerKind::Drag => match &mut self.drag {
                Some(d) => {
                    d.4 = ev.pos;
                    vec![Action::Handled]
                }
                None => vec![],
            },
            PointerKind::Up => match self.drag.take() {
                Some((id, si, ai, from, to)) if (to - from).hypot() > 0.0 => {
                    let inv = spread.inverse();
                    let d = (inv * to) - (inv * from);
                    vec![Action::Exec("path.moveAnchors".into(), json!({"id": id.0, "anchors": [[si, ai]], "dx": d.x, "dy": d.y}))]
                }
                _ => vec![],
            },
            _ => vec![],
        }
    }

    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: Mods) -> Vec<Action> {
        if key == ToolKey::Escape {
            self.drag = None;
            return vec![Action::SwitchTool("selection".into())];
        }
        vec![]
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let Some((id, it, m, _)) = target(cx) else { return vec![] };
        let h = cx.tol(3.5);
        let mut out = Vec::new();
        for (si, ai, mut c) in anchors(it, m) {
            if let Some((d, dsi, dai, from, to)) = self.drag
                && d == id
                && (dsi, dai) == (si, ai)
            {
                c += to - from;
            }
            let mut sq = BezPath::new();
            sq.move_to((c.x - h, c.y - h));
            sq.line_to((c.x + h, c.y - h));
            sq.line_to((c.x + h, c.y + h));
            sq.line_to((c.x - h, c.y + h));
            sq.close_path();
            out.push(Overlay::Path { path: sq, color: [0, 0, 0], dashed: false });
        }
        out
    }

    fn cursor(&self, cx: &ToolContext, p: Point, mods: Mods) -> Cursor {
        let Some((_, it, m, _)) = target(cx) else { return Cursor::Arrow };
        if anchor_at(cx, it, m, p).is_some() {
            return if mods.cmd && mods.shift { Cursor::PenDelete } else { Cursor::Move };
        }
        if mods.cmd && on_outline(cx, it, m, p) {
            return Cursor::PenAdd;
        }
        Cursor::Arrow
    }

    fn busy(&self) -> bool {
        self.drag.is_some()
    }
}

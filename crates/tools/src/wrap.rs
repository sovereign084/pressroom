//! Edit Wrap Points (PressRoom, as Publisher's): the selected object's text wrap boundary drawn as
//! points to drag. Ctrl and a click on the outline adds a point there; Ctrl, Shift and a click on
//! a point takes it away. Esc, or a click away from the outline, goes back to selecting. Each
//! change is one `object.textWrap` with the new boundary, which also makes the wrap Tight.

use designcraft_doc::{Item, ItemId, WrapMode};
use designcraft_geom::{Affine, BezPath, Point};
use serde_json::json;

use crate::{Action, Cursor, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

/// Most points a boundary is shown with: a detailed outline is thinned to this many.
const MOST_POINTS: usize = 48;
/// How near the pointer must be to a point or the outline, in screen pixels.
const REACH_PX: f64 = 5.0;

#[derive(Default)]
pub struct WrapPointsTool {
    /// The point being dragged and the boundary as it is meanwhile (the object's inner space).
    drag: Option<(ItemId, usize, Vec<Point>)>,
}

/// The object whose wrap points are shown: the one selected, and its inner space → canvas map.
fn target<'a>(cx: &'a ToolContext) -> Option<(ItemId, &'a Item, Affine)> {
    let [id] = cx.selection.items.as_slice() else { return None };
    let it = cx.doc.item(*id)?;
    Some((*id, it, cx.item_canvas_xf(*id)? * it.xf))
}

/// The wrap boundary's points in the object's inner space: its own boundary when it has one, else
/// its outline, a box's grown by the wrap's distances from the text.
pub fn wrap_points(it: &Item) -> Vec<Point> {
    let bp = match &it.wrap.path {
        Some(p) => p.to_bezpath(),
        None => it.path.to_bezpath(),
    };
    let mut pts: Vec<Point> = Vec::new();
    designcraft_geom::kurbo::flatten(bp, 0.5, |el| match el {
        designcraft_geom::PathEl::MoveTo(p) if pts.is_empty() => pts.push(p),
        designcraft_geom::PathEl::LineTo(p) if !pts.is_empty() => pts.push(p),
        _ => {}
    });
    if pts.len() > 1 && pts.first().zip(pts.last()).is_some_and(|(a, b)| (*a - *b).hypot() < 1e-6) {
        pts.pop();
    }
    if pts.len() > MOST_POINTS {
        let step = pts.len().div_ceil(MOST_POINTS);
        pts = pts.into_iter().step_by(step).collect();
    }
    if it.wrap.path.is_none() && pts.len() == 4 {
        // A box: its corners moved out by the distances (top, left, bottom, right).
        let o = it.wrap.offsets;
        let (x0, x1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.y), b.max(p.y)));
        let mid = Point::new((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for p in &mut pts {
            p.x += if p.x < mid.x { -o[1] } else { o[3] };
            p.y += if p.y < mid.y { -o[0] } else { o[2] };
        }
    }
    pts
}

/// The canvas distance from `p` to the segment `a`–`b`, and the nearest point on it.
fn to_segment(p: Point, a: Point, b: Point) -> (f64, Point) {
    let d = b - a;
    let len2 = d.hypot2();
    let t = if len2 > 0.0 { ((p - a).dot(d) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let q = a + d * t;
    ((p - q).hypot(), q)
}

fn commit(it: &Item, id: ItemId, pts: &[Point]) -> Vec<Action> {
    let mut p = json!({"ids": [id.0], "points": pts.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>()});
    // As in Publisher, a boundary of its own makes the wrap follow it closely.
    if it.wrap.mode != WrapMode::Contour {
        p["mode"] = json!("contour");
    }
    vec![Action::Exec("object.textWrap".into(), p)]
}

impl WrapPointsTool {
    /// The point of the boundary under `pos`, if any.
    fn point_at(cx: &ToolContext, m: Affine, pts: &[Point], pos: Point) -> Option<usize> {
        let reach = cx.tol(REACH_PX);
        pts.iter().position(|p| (m * *p - pos).hypot() <= reach)
    }

    /// The edge of the boundary under `pos`: the index the new point goes at, and where on the
    /// canvas.
    fn edge_at(cx: &ToolContext, m: Affine, pts: &[Point], pos: Point) -> Option<(usize, Point)> {
        let reach = cx.tol(REACH_PX);
        let n = pts.len();
        (0..n)
            .filter_map(|i| {
                let (d, q) = to_segment(pos, m * pts[i], m * pts[(i + 1) % n]);
                (d <= reach).then_some((d, i + 1, q))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, i, q)| (i, q))
    }
}

impl Tool for WrapPointsTool {
    fn id(&self) -> &'static str {
        "wrapPoints"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let Some((id, it, m)) = target(cx) else {
            self.drag = None;
            return vec![Action::SwitchTool("selection".into())];
        };
        match ev.kind {
            PointerKind::Down => {
                let mut pts = wrap_points(it);
                if pts.len() < 3 {
                    return vec![Action::SwitchTool("selection".into())];
                }
                if let Some(i) = Self::point_at(cx, m, &pts, ev.pos) {
                    if ev.mods.cmd && ev.mods.shift {
                        if pts.len() > 3 {
                            pts.remove(i);
                            return commit(it, id, &pts);
                        }
                        return vec![Action::Handled];
                    }
                    self.drag = Some((id, i, pts));
                    return vec![Action::Handled];
                }
                if ev.mods.cmd
                    && let Some((i, q)) = Self::edge_at(cx, m, &pts, ev.pos)
                {
                    pts.insert(i, m.inverse() * q);
                    self.drag = Some((id, i, pts));
                    return vec![Action::Handled];
                }
                // A click away from the outline ends editing.
                vec![Action::SwitchTool("selection".into())]
            }
            PointerKind::Drag => match &mut self.drag {
                Some((_, i, pts)) => {
                    if let Some(p) = pts.get_mut(*i) {
                        *p = m.inverse() * ev.pos;
                    }
                    vec![Action::Handled]
                }
                None => vec![],
            },
            PointerKind::Up => match self.drag.take() {
                Some((id, _, pts)) => commit(it, id, &pts),
                None => vec![],
            },
            _ => vec![],
        }
    }

    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: crate::Mods) -> Vec<Action> {
        if key == ToolKey::Escape {
            self.drag = None;
            return vec![Action::SwitchTool("selection".into())];
        }
        vec![]
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let Some((id, it, m)) = target(cx) else { return vec![] };
        let pts = match &self.drag {
            Some((d, _, pts)) if *d == id => pts.clone(),
            _ => wrap_points(it),
        };
        if pts.len() < 3 {
            return vec![];
        }
        let mut outline = BezPath::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 {
                outline.move_to(m * *p);
            } else {
                outline.line_to(m * *p);
            }
        }
        outline.close_path();
        let mut out = vec![Overlay::Path { path: outline, color: [60, 60, 60], dashed: true }];
        let h = cx.tol(3.5);
        for p in &pts {
            let c = m * *p;
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

    fn cursor(&self, cx: &ToolContext, p: Point, mods: crate::Mods) -> Cursor {
        let Some((_, it, m)) = target(cx) else { return Cursor::Arrow };
        let pts = wrap_points(it);
        if Self::point_at(cx, m, &pts, p).is_some() {
            return if mods.cmd && mods.shift { Cursor::PenDelete } else { Cursor::Move };
        }
        if mods.cmd && Self::edge_at(cx, m, &pts, p).is_some() {
            return Cursor::PenAdd;
        }
        Cursor::Arrow
    }

    fn busy(&self) -> bool {
        self.drag.is_some()
    }
}

//! Connectors, as Publisher's: lines whose ends stay on the connection sites of two objects (the
//! middle of each side of the object's box). After every edit [`sync`] routes each connector
//! from the sites as they are now, so moving or resizing an object takes its connectors along.

use designcraft_doc::{Connector, ConnectorEnd, ConnectorKind, Document, ItemId, Shape};
use designcraft_geom::{Affine, BezPath, PathData, Point, Vec2};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, has_selection, str_param};
use crate::{Result, Session};

/// How near an object a connector's end must be drawn to attach to it, in points.
const REACH: f64 = 12.0;
/// How far an elbow connector runs straight out of a site before it turns.
const STUB: f64 = 9.0;
/// The way out of each site: top, right, bottom, left.
const OUT: [Vec2; 4] = [Vec2::new(0.0, -1.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0)];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "object.connect",
            "Connect",
            [],
            None,
            "{id (a line), kind?: straight|elbow|curved, from?: {item, site} | null, to?: {item, site} | null, auto?: bool (each end attaches to the nearest side of the object it was drawn on)} — the line becomes a connector: its ends stay on the objects' connection sites (site 0 top, 1 right, 2 bottom, 3 left) as they move",
            has_doc,
            connect
        ),
        cmd!(
            "object.rerouteConnectors",
            "Reroute Connectors",
            ["Object"],
            None,
            "{ids?} — the connectors among the selected objects, and those joined to them, move to the nearest sides of their objects",
            has_selection,
            reroute
        ),
    ]
}

/// The middle of each side of an object's box on its spread: top, right, bottom, left.
fn sites(d: &Document, id: ItemId) -> Option<[Point; 4]> {
    let loc = d.find(id)?;
    let it = d.item(id)?;
    let b = d.parent_xf(&loc).transform_rect_bbox(it.bounds());
    let c = b.center();
    Some([Point::new(c.x, b.y0), Point::new(b.x1, c.y), Point::new(c.x, b.y1), Point::new(b.x0, c.y)])
}

/// A line's two ends on its spread, and its parent's transform to the spread.
fn line_ends(d: &Document, id: ItemId) -> Option<(Point, Point, Affine)> {
    let loc = d.find(id)?;
    let it = d.item(id)?;
    let parent = d.parent_xf(&loc);
    let m = parent * it.xf;
    let sp = it.path.subpaths.first()?;
    Some((m * sp.anchors.first()?.p, m * sp.anchors.last()?.p, parent))
}

/// The site nearest `p` of the object (other than the line itself and other lines) whose box,
/// grown by [`REACH`], holds it.
fn nearest_end(d: &Document, line: ItemId, p: Point) -> Option<ConnectorEnd> {
    let loc = d.find(line)?;
    let spread = d.spread(loc.spread)?;
    spread
        .items
        .iter()
        .filter(|it| it.id != line && !it.hidden && it.shape != Shape::GraphicLine && it.bounds().inflate(REACH, REACH).contains(p))
        .filter_map(|it| {
            let s = sites(d, it.id)?;
            let (site, dist) = s.iter().enumerate().map(|(i, q)| (i, (*q - p).hypot())).min_by(|a, b| a.1.total_cmp(&b.1))?;
            Some((ConnectorEnd { item: it.id, site: site as u8 }, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// The way out of an end: its site's, or toward the other end when it isn't attached.
fn way_out(attached: Option<Vec2>, from: Point, to: Point) -> Vec2 {
    attached.unwrap_or_else(|| {
        let d = to - from;
        if d.x.abs() >= d.y.abs() { Vec2::new(d.x.signum(), 0.0) } else { Vec2::new(0.0, d.y.signum()) }
    })
}

/// The connector's path from `a` (leaving along `da`) to `b` (arriving against `db`).
fn route(kind: ConnectorKind, a: Point, da: Vec2, b: Point, db: Vec2) -> BezPath {
    let mut bp = BezPath::new();
    bp.move_to(a);
    match kind {
        ConnectorKind::Straight => bp.line_to(b),
        ConnectorKind::Curved => {
            let k = ((b - a).hypot() / 2.0).max(18.0);
            bp.curve_to(a + da * k, b + db * k, b);
        }
        ConnectorKind::Elbow => {
            let (pa, pb) = (a + da * STUB, b + db * STUB);
            let mut pts = vec![pa];
            match (da.x != 0.0, db.x != 0.0) {
                (true, true) => {
                    let mx = (pa.x + pb.x) / 2.0;
                    pts.extend([Point::new(mx, pa.y), Point::new(mx, pb.y)]);
                }
                (false, false) => {
                    let my = (pa.y + pb.y) / 2.0;
                    pts.extend([Point::new(pa.x, my), Point::new(pb.x, my)]);
                }
                (true, false) => pts.push(Point::new(pb.x, pa.y)),
                (false, true) => pts.push(Point::new(pa.x, pb.y)),
            }
            pts.extend([pb, b]);
            let mut last = a;
            for q in pts {
                if (q - last).hypot() > 1e-6 {
                    bp.line_to(q);
                    last = q;
                }
            }
        }
    }
    bp
}

/// Routes every connector from its objects as they are now; one whose line is gone is dropped,
/// and an end whose object is gone stays where it is, no longer attached.
pub(crate) fn sync(d: &mut Document) {
    if d.connectors.is_empty() {
        return;
    }
    let connectors = std::mem::take(&mut d.connectors);
    let mut kept = Vec::with_capacity(connectors.len());
    for c in connectors {
        let Some((a0, b0, parent)) = line_ends(d, c.line) else { continue };
        let at = |e: Option<ConnectorEnd>| e.and_then(|e| Some((*sites(d, e.item)?.get(usize::from(e.site))?, OUT[usize::from(e.site) % 4], e)));
        let (from, to) = (at(c.from), at(c.to));
        let (a, b) = (from.map_or(a0, |f| f.0), to.map_or(b0, |t| t.0));
        let da = way_out(from.map(|f| f.1), a, b);
        let db = way_out(to.map(|t| t.1), b, a);
        let local = parent.inverse() * route(c.kind, a, da, b, db);
        let path = PathData::from_bezpath(&local);
        let shape = if c.kind == ConnectorKind::Straight { Shape::GraphicLine } else { Shape::Path };
        if let Some(it) = d.item_mut(c.line)
            && (it.path != path || it.xf != Affine::IDENTITY || it.shape != shape)
        {
            it.path = path;
            it.xf = Affine::IDENTITY;
            it.shape = shape;
        }
        kept.push(Connector { from: from.map(|f| f.2), to: to.map(|t| t.2), ..c });
    }
    d.connectors = kept;
}

fn end_param(d: &Document, p: &Value, key: &str) -> Result<Option<Option<ConnectorEnd>>> {
    const ID: &str = "object.connect";
    match p.get(key) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => {
            let e: ConnectorEnd = serde_json::from_value(v.clone()).map_err(|_| bad(ID, format!("{key}: {{item, site}}")))?;
            if e.site > 3 || d.item(e.item).is_none() {
                return Err(bad(ID, format!("{key}: an object's id and a site from 0 to 3")));
            }
            Ok(Some(Some(e)))
        }
    }
}

fn connect(s: &mut Session, p: &Value) -> Result<Value> {
    const ID: &str = "object.connect";
    let id = super::id_param(p, "id").ok_or_else(|| bad(ID, "missing id"))?;
    let kind = match str_param(p, "kind") {
        None => None,
        Some("straight") => Some(ConnectorKind::Straight),
        Some("elbow") => Some(ConnectorKind::Elbow),
        Some("curved") => Some(ConnectorKind::Curved),
        Some(o) => return Err(bad(ID, format!("unknown kind `{o}` (straight, elbow, curved)"))),
    };
    let auto = p.get("auto").and_then(Value::as_bool).unwrap_or(false);
    let (from, to) = {
        let d = &s.doc()?.doc;
        (end_param(d, p, "from")?, end_param(d, p, "to")?)
    };
    s.edit(|d, _| {
        let (a, b, _) = line_ends(d, id).ok_or_else(|| bad(ID, "not a line"))?;
        let mut c = d.connectors.iter().find(|c| c.line == id).copied().unwrap_or(Connector {
            line: id,
            kind: ConnectorKind::Straight,
            from: None,
            to: None,
        });
        if let Some(k) = kind {
            c.kind = k;
        }
        if auto {
            c.from = nearest_end(d, id, a);
            c.to = nearest_end(d, id, b);
        }
        if let Some(e) = from {
            c.from = e;
        }
        if let Some(e) = to {
            c.to = e;
        }
        d.connectors.retain(|x| x.line != id);
        // A straight line drawn away from any object is only a line.
        if !(auto && c.kind == ConnectorKind::Straight && c.from.is_none() && c.to.is_none()) {
            d.connectors.push(c);
        }
        Ok(json!({"line": id.0, "from": c.from, "to": c.to}))
    })
}

fn reroute(s: &mut Session, p: &Value) -> Result<Value> {
    let ids: Vec<ItemId> = match p.get("ids").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_u64).map(ItemId).collect(),
        None => s.doc()?.selection.items.clone(),
    };
    s.edit(|d, _| {
        let mut moved = 0usize;
        let mut cs = std::mem::take(&mut d.connectors);
        for c in &mut cs {
            let touches = |e: Option<ConnectorEnd>| e.is_some_and(|e| ids.contains(&e.item));
            if !(ids.contains(&c.line) || touches(c.from) || touches(c.to)) {
                continue;
            }
            match (c.from, c.to) {
                (Some(f), Some(t)) => {
                    let (Some(sa), Some(sb)) = (sites(d, f.item), sites(d, t.item)) else { continue };
                    let best = (0..4u8).flat_map(|i| (0..4u8).map(move |j| (i, j))).min_by(|x, y| {
                        let dx = (sa[usize::from(x.0)] - sb[usize::from(x.1)]).hypot();
                        let dy = (sa[usize::from(y.0)] - sb[usize::from(y.1)]).hypot();
                        dx.total_cmp(&dy)
                    });
                    if let Some((i, j)) = best {
                        c.from = Some(ConnectorEnd { site: i, ..f });
                        c.to = Some(ConnectorEnd { site: j, ..t });
                        moved += 1;
                    }
                }
                (Some(e), None) | (None, Some(e)) => {
                    let Some((a, b, _)) = line_ends(d, c.line) else { continue };
                    let other = if c.from.is_some() { b } else { a };
                    let Some(s) = sites(d, e.item) else { continue };
                    let site = (0..4u8)
                        .min_by(|x, y| (s[usize::from(*x)] - other).hypot().total_cmp(&(s[usize::from(*y)] - other).hypot()))
                        .unwrap_or(e.site);
                    let e = Some(ConnectorEnd { site, ..e });
                    if c.from.is_some() {
                        c.from = e;
                    } else {
                        c.to = e;
                    }
                    moved += 1;
                }
                (None, None) => {}
            }
        }
        d.connectors = cs;
        Ok(json!({"rerouted": moved}))
    })
}

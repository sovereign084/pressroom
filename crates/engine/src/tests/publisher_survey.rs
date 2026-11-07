//! Publisher's features from the 2026-11-07 survey: divided cells, Fill Down and Fill Right, Grow
//! to Fit Text, Through and wrap points, label stock, registration marks and connectors.

use designcraft_doc::{CellDiagonal, ItemId, RowHeightMode, StoryId, WrapMode};
use designcraft_geom::Point;
use serde_json::json;

use super::Session;

fn new_doc() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"pages": 1})).unwrap();
    s
}

/// A text frame holding a `rows` × `cols` table: (story, table).
fn table(s: &mut Session, rows: u32, cols: u32) -> (u64, u64) {
    let f = s.execute("frame.create", &json!({"rect": [36, 36, 336, 500], "content": "text", "text": ""})).unwrap();
    let sid = f["story"].as_u64().unwrap();
    let tid = s.execute("table.insert", &json!({"rows": rows, "cols": cols})).unwrap()["table"].as_u64().unwrap();
    (sid, tid)
}

fn cell(s: &Session, sid: u64, tid: u64, r: usize, c: usize) -> designcraft_doc::Cell {
    s.doc().unwrap().doc.story(StoryId(sid)).unwrap().tables[&tid].cell(r, c).unwrap().clone()
}

fn frag(s: &Session, sid: u64, tid: u64) -> designcraft_compose::TableFrag {
    let st = s.doc().unwrap();
    let cs = s.cache.get(&st.doc, StoryId(sid), None);
    cs.frames.iter().flat_map(|f| &f.tables).find(|t| t.table == tid).unwrap().clone()
}

#[test]
fn a_divided_cell_draws_its_diagonal_and_sets_a_half_for_each_paragraph() {
    let mut s = new_doc();
    let (sid, tid) = table(&mut s, 2, 2);
    s.execute("table.setRowHeight", &json!({"story": sid, "table": tid, "row": 0, "height": 72})).unwrap();
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "text": "Time", "diagonal": "down"})).unwrap();
    let c = cell(&s, sid, tid, 0, 0);
    assert_eq!(c.diagonal, CellDiagonal::Down);
    assert_eq!(c.text.text, "Time\n", "dividing gives the lower half a paragraph");
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "text": "Time\nDay"})).unwrap();
    let f = frag(&s, sid, tid);
    let placed = f.cells.iter().find(|p| p.row == 0 && p.col == 0).unwrap();
    let r = placed.rect;
    let near = |a: Point, b: Point| (a - b).hypot() < 0.01;
    assert!(f.strokes.iter().any(|seg| near(seg.a, Point::new(r.x0, r.y0)) && near(seg.b, Point::new(r.x1, r.y1))), "the diagonal, corner to corner");
    // The first paragraph at the top on the right, the last at the bottom on the left.
    let lines = &placed.text.frames[0].lines;
    assert_eq!(lines.len(), 2);
    assert!(lines[1].baseline - lines[0].baseline > 30.0, "the halves are apart: {} {}", lines[0].baseline, lines[1].baseline);
    let x0 = |i: usize| lines[i].glyphs.first().map_or(0.0, |g| g.x);
    assert!(x0(0) > x0(1) + 20.0, "upper half right, lower half left");
    // Divide Up runs the other way; No Division takes the line away.
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "diagonal": "up"})).unwrap();
    let f = frag(&s, sid, tid);
    assert!(f.strokes.iter().any(|seg| near(seg.a, Point::new(r.x0, r.y1)) && near(seg.b, Point::new(r.x1, r.y0))));
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "diagonal": "none"})).unwrap();
    let f = frag(&s, sid, tid);
    assert!(!f.strokes.iter().any(|seg| (seg.a.x - seg.b.x).abs() > 0.01 && (seg.a.y - seg.b.y).abs() > 0.01), "no slanted stroke left");
    assert!(s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "diagonal": "sideways"})).is_err());
    // Undo puts the division back.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(cell(&s, sid, tid, 0, 0).diagonal, CellDiagonal::Up);
}

#[test]
fn fill_down_and_fill_right_copy_the_first_cell() {
    let mut s = new_doc();
    let (sid, tid) = table(&mut s, 3, 3);
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 1, "text": "Choir", "fill": "[Black]", "tint": 0.2})).unwrap();
    s.execute("table.fillCells", &json!({"story": sid, "table": tid, "rows": [0, 2], "cols": [1, 1]})).unwrap();
    for r in 1..3 {
        let c = cell(&s, sid, tid, r, 1);
        assert_eq!((c.text.text.as_str(), c.fill.as_str(), c.fill_tint), ("Choir", "[Black]", 0.2), "row {r}");
    }
    assert_eq!(cell(&s, sid, tid, 1, 0).text.text, "", "other columns untouched");
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 2, "col": 0, "text": "Sunday"})).unwrap();
    s.execute("table.fillCells", &json!({"story": sid, "table": tid, "rows": [2, 2], "cols": [0, 2], "direction": "right"})).unwrap();
    assert_eq!(cell(&s, sid, tid, 2, 2).text.text, "Sunday");
    assert!(s.execute("table.fillCells", &json!({"story": sid, "table": tid, "direction": "up"})).is_err());
    s.doc().unwrap().doc.check().unwrap();
}

#[test]
fn grow_to_fit_text_off_keeps_each_row_as_tall_as_it_shows() {
    let mut s = new_doc();
    let (sid, tid) = table(&mut s, 2, 2);
    s.execute("table.setCell", &json!({"story": sid, "table": tid, "row": 0, "col": 0, "text": "One\nTwo\nThree\nFour"})).unwrap();
    let shown = frag(&s, sid, tid).cells.iter().find(|p| p.row == 0 && p.col == 0).unwrap().rect.height();
    let r = s.execute("table.growToFit", &json!({"story": sid, "table": tid})).unwrap();
    assert_eq!(r["on"], false, "every row grows to start with, so the first click turns it off");
    let t = s.doc().unwrap().doc.story(StoryId(sid)).unwrap().tables[&tid].clone();
    assert!(t.rows.iter().all(|r| r.mode == RowHeightMode::Exactly));
    assert!(t.rows[0].height >= shown - 0.01, "{} ≥ {shown}", t.rows[0].height);
    s.execute("table.growToFit", &json!({"story": sid, "table": tid, "on": true})).unwrap();
    let t = s.doc().unwrap().doc.story(StoryId(sid)).unwrap().tables[&tid].clone();
    assert!(t.rows.iter().all(|r| r.mode == RowHeightMode::AtLeast));
}

#[test]
fn through_and_wrap_points_are_set_on_the_wrap() {
    let mut s = new_doc();
    let id = s.execute("frame.create", &json!({"rect": [100, 100, 200, 200], "content": "unassigned"})).unwrap()["id"].as_u64().unwrap();
    s.execute("object.textWrap", &json!({"mode": "contour", "insideEdges": true})).unwrap();
    let w = s.doc().unwrap().doc.item(ItemId(id)).unwrap().wrap.clone();
    assert!(w.inside_edges && w.mode == WrapMode::Contour);
    s.execute("object.textWrap", &json!({"points": [[-5, -5], [105, -5], [50, 120]]})).unwrap();
    let w = s.doc().unwrap().doc.item(ItemId(id)).unwrap().wrap.clone();
    let path = w.path.expect("a boundary of its own");
    assert_eq!(path.subpaths[0].anchors.len(), 3);
    assert!(path.subpaths[0].closed);
    for bad in [json!({"points": [[0, 0], [1, 1]]}), json!({"points": "here"}), json!({"points": [[0, 0], [1, 1], [f64::MAX, 0]]})] {
        assert!(s.execute("object.textWrap", &bad).is_err(), "{bad}");
    }
}

#[test]
fn label_stock_prints_from_its_own_margins() {
    let mut s = new_doc();
    // Address labels, 30 to a letter sheet: 2 5/8 x 1 in, 3/16 in in, 1/2 in down, 1/8 in apart.
    s.execute("layout.documentSetup", &json!({"width": 189, "height": 72, "labelSheet": {"side": 13.5, "top": 36, "hGap": 9, "vGap": 0}})).unwrap();
    let r = s.execute("file.printPlan", &json!({"layout": "copiesPerSheet"})).unwrap();
    let placed = r["sides"][0]["placed"].as_array().unwrap();
    assert_eq!(placed.len(), 30);
    assert_eq!((placed[0]["x"].as_f64(), placed[0]["y"].as_f64()), (Some(13.5), Some(36.0)));
    assert_eq!(placed[1]["x"].as_f64(), Some(13.5 + 189.0 + 9.0));
    assert_eq!(s.execute("layout.documentSetup", &json!({})).unwrap()["labelSheet"]["top"], 36.0);
    s.execute("layout.documentSetup", &json!({"labelSheet": null})).unwrap();
    assert!(s.doc().unwrap().doc.settings.label_sheet.is_none());
    assert!(s.execute("layout.documentSetup", &json!({"labelSheet": {"side": -1}})).is_err());
}

#[test]
fn registration_marks_are_drawn_when_asked_for() {
    let mut s = new_doc();
    s.execute("frame.create", &json!({"rect": [36, 36, 300, 200], "content": "text", "text": "Notes"})).unwrap();
    let size =
        |s: &mut Session, marks: serde_json::Value| s.execute("file.exportPdf", &json!({"marks": marks})).unwrap()["base64"].as_str().unwrap().len();
    let crop = size(&mut s, json!({"crop": true}));
    let both = size(&mut s, json!({"crop": true, "registration": true}));
    assert!(both > crop, "{both} > {crop}");
}

#[test]
fn a_connector_follows_the_objects_its_ends_hold() {
    let mut s = new_doc();
    let a = s.execute("frame.create", &json!({"rect": [50, 50, 150, 150], "content": "unassigned"})).unwrap()["id"].as_u64().unwrap();
    let b = s.execute("frame.create", &json!({"rect": [300, 50, 400, 150], "content": "unassigned"})).unwrap()["id"].as_u64().unwrap();
    let line = s.execute("line.create", &json!({"a": [152, 100], "b": [298, 100]})).unwrap()["id"].as_u64().unwrap();
    let r = s.execute("object.connect", &json!({"id": line, "kind": "elbow", "auto": true})).unwrap();
    assert_eq!((r["from"]["item"].as_u64(), r["from"]["site"].as_u64()), (Some(a), Some(1)), "{r}");
    assert_eq!((r["to"]["item"].as_u64(), r["to"]["site"].as_u64()), (Some(b), Some(3)), "{r}");
    let ends = |s: &Session| {
        let d = &s.doc().unwrap().doc;
        let it = d.item(ItemId(line)).unwrap();
        let sp = &it.path.subpaths[0];
        (it.xf * sp.anchors[0].p, it.xf * sp.anchors[sp.anchors.len() - 1].p, sp.anchors.len())
    };
    let (p0, p1, _) = ends(&s);
    assert!((p0 - Point::new(150.0, 100.0)).hypot() < 0.01 && (p1 - Point::new(300.0, 100.0)).hypot() < 0.01, "{p0:?} {p1:?}");
    // Moving the second object takes the end along, and the elbow bends to reach it.
    s.execute("selection.set", &json!({"ids": [b]})).unwrap();
    s.execute("transform.move", &json!({"dx": 0, "dy": 150})).unwrap();
    let (_, p1, n) = ends(&s);
    assert!((p1 - Point::new(300.0, 250.0)).hypot() < 0.01, "{p1:?}");
    assert!(n > 2, "an elbow has bends");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!((ends(&s).1 - Point::new(300.0, 100.0)).hypot() < 0.01, "undo takes both back");
    // Reroute picks the nearest sides; a deleted object lets its end go.
    s.execute("selection.set", &json!({"ids": [b]})).unwrap();
    s.execute("transform.move", &json!({"dx": -250, "dy": 200})).unwrap();
    s.execute("object.rerouteConnectors", &json!({"ids": [line]})).unwrap();
    let c = s.doc().unwrap().doc.connectors[0];
    assert_eq!((c.from.unwrap().site, c.to.unwrap().site), (2, 0), "bottom of the first to the top of the second");
    s.execute("selection.set", &json!({"ids": [b]})).unwrap();
    s.execute("edit.clear", &json!({})).unwrap();
    let c = s.doc().unwrap().doc.connectors[0];
    assert!(c.to.is_none() && c.from.is_some());
    // The line deleted, its connector goes too.
    s.execute("selection.set", &json!({"ids": [line]})).unwrap();
    s.execute("edit.clear", &json!({})).unwrap();
    assert!(s.doc().unwrap().doc.connectors.is_empty());
    // A plain line drawn away from everything is only a line.
    let lone = s.execute("line.create", &json!({"a": [400, 600], "b": [500, 600]})).unwrap()["id"].as_u64().unwrap();
    s.execute("object.connect", &json!({"id": lone, "auto": true})).unwrap();
    assert!(s.doc().unwrap().doc.connectors.is_empty());
}

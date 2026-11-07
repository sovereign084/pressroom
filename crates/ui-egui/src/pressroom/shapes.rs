//! The Shapes gallery: lines and arrows, rectangles, basic shapes, block arrows, stars and banners,
//! and callouts, all PressRoom's own drawings (`designcraft_geom::shapes::preset`). A shape is drawn
//! by dragging on the page, with its outline showing as it grows, or dropped at a default size by a
//! click.

use designcraft_geom::kurbo;
use egui::{Pos2, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::DesignApp;

struct Section {
    title: &'static str,
    shapes: &'static [(&'static str, &'static str)],
}

const LINES: &[(&str, &str)] = &[
    ("line", "Line"),
    ("arrow", "Arrow"),
    ("doubleArrow", "Double Arrow"),
    ("elbowConnector", "Connector: Elbow"),
    ("elbowArrowConnector", "Connector: Elbow Arrow"),
    ("curvedConnector", "Connector: Curved"),
    ("curvedArrowConnector", "Connector: Curved Arrow"),
];

/// A line from the gallery: the connector it makes once drawn (every line holds on to the objects
/// its ends are drawn on), and its arrowheads.
pub(super) fn connector(name: &str) -> Option<(&'static str, Option<&'static str>)> {
    match name {
        "line" => Some(("straight", None)),
        "arrow" => Some(("straight", Some("arrow"))),
        "doubleArrow" => Some(("straight", Some("doubleArrow"))),
        "elbowConnector" => Some(("elbow", None)),
        "elbowArrowConnector" => Some(("elbow", Some("arrow"))),
        "curvedConnector" => Some(("curved", None)),
        "curvedArrowConnector" => Some(("curved", Some("arrow"))),
        _ => None,
    }
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Rectangles",
        shapes: &[("rectangle", "Rectangle"), ("roundedRect", "Rounded Rectangle"), ("snippedRect", "Snipped Rectangle"), ("pill", "Rounded Bar")],
    },
    Section {
        title: "Basic Shapes",
        shapes: &[
            ("ellipse", "Oval"),
            ("triangle", "Triangle"),
            ("rightTriangle", "Right Triangle"),
            ("parallelogram", "Parallelogram"),
            ("trapezoid", "Trapezoid"),
            ("diamond", "Diamond"),
            ("pentagon", "Pentagon"),
            ("hexagon", "Hexagon"),
            ("octagon", "Octagon"),
            ("cross", "Cross"),
            ("heart", "Heart"),
            ("lightning", "Lightning Bolt"),
            ("sun", "Sun"),
            ("moon", "Moon"),
            ("cloud", "Cloud"),
        ],
    },
    Section {
        title: "Block Arrows",
        shapes: &[
            ("arrowRight", "Right Arrow"),
            ("arrowLeft", "Left Arrow"),
            ("arrowUp", "Up Arrow"),
            ("arrowDown", "Down Arrow"),
            ("arrowLeftRight", "Left-Right Arrow"),
            ("arrowUpDown", "Up-Down Arrow"),
            ("chevron", "Chevron"),
            ("homePlate", "Arrow: Pentagon"),
            ("notchedArrow", "Notched Right Arrow"),
        ],
    },
    Section {
        title: "Stars and Banners",
        shapes: &[
            ("star4", "4-Point Star"),
            ("star5", "5-Point Star"),
            ("star6", "6-Point Star"),
            ("star8", "8-Point Star"),
            ("star12", "12-Point Star"),
            ("star16", "16-Point Star"),
            ("star24", "24-Point Star"),
            ("ribbon", "Ribbon"),
            ("wave", "Wave"),
        ],
    },
    Section {
        title: "Callouts",
        shapes: &[("speech", "Speech Bubble"), ("roundSpeech", "Rounded Speech Bubble"), ("ovalSpeech", "Oval Speech Bubble")],
    },
    Section {
        title: "Flowchart",
        shapes: &[("flowDocument", "Document"), ("parallelogram", "Data"), ("diamond", "Decision"), ("pill", "Terminator")],
    },
];

/// The drawing tool and preset for a gallery name.
fn tool_for(name: &str) -> (&'static str, Option<&str>) {
    if let Some((_, heads)) = connector(name) {
        return ("line", heads);
    }
    match name {
        "rectangle" => ("rectangle", None),
        "ellipse" => ("ellipse", None),
        _ => ("rectangle", Some(name)),
    }
}

/// Starts drawing the shape on the page.
fn start(app: &mut DesignApp, name: &str) {
    let (tool, preset) = tool_for(name);
    super::start_draw(app, tool);
    let _ = app.run("tool.shapePreset", json!({"name": preset}));
    if let Some((kind, _)) = connector(name) {
        super::await_connector(app, kind);
        if kind != "straight" {
            app.status("Drag from one object to another. The connector stays joined to them when they move.");
        }
    }
}

/// One square of the gallery: the shape's own outline. Returns whether it was clicked.
fn tile(ui: &mut Ui, name: &str, label: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, super::PR::ROUND, super::pal().hover);
    }
    let inner = r.shrink(5.0);
    let ink = Stroke::new(1.1, super::pal().text);
    let rect = designcraft_geom::Rect::new(f64::from(inner.min.x), f64::from(inner.min.y), f64::from(inner.max.x), f64::from(inner.max.y));
    let path = match name {
        "elbowConnector" | "elbowArrowConnector" | "curvedConnector" | "curvedArrowConnector" => {
            let (a, b) = (inner.left_top() + vec2(0.0, 2.0), inner.right_bottom() - vec2(0.0, 2.0));
            if name.starts_with("elbow") {
                let mx = (a.x + b.x) / 2.0;
                ui.painter().line_segment([a, pos2(mx, a.y)], ink);
                ui.painter().line_segment([pos2(mx, a.y), pos2(mx, b.y)], ink);
                ui.painter().line_segment([pos2(mx, b.y), b], ink);
            } else {
                let pts: Vec<Pos2> = (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0;
                        let u = 1.0 - t;
                        let (c1, c2) = (pos2(b.x, a.y), pos2(a.x, b.y));
                        pos2(
                            u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x,
                            u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y,
                        )
                    })
                    .collect();
                ui.painter().add(egui::Shape::line(pts, ink));
            }
            if name.contains("Arrow") {
                ui.painter().line_segment([b, b + vec2(-4.0, -2.5)], ink);
                ui.painter().line_segment([b, b + vec2(-4.0, 2.5)], ink);
            }
            None
        }
        "line" | "arrow" | "doubleArrow" => {
            ui.painter().line_segment([inner.left_bottom(), inner.right_top()], ink);
            if name != "line" {
                let tip = inner.right_top();
                ui.painter().line_segment([tip, tip + vec2(-5.0, 0.5)], ink);
                ui.painter().line_segment([tip, tip + vec2(-0.5, 5.0)], ink);
            }
            if name == "doubleArrow" {
                let tail = inner.left_bottom();
                ui.painter().line_segment([tail, tail + vec2(5.0, -0.5)], ink);
                ui.painter().line_segment([tail, tail + vec2(0.5, -5.0)], ink);
            }
            None
        }
        "rectangle" => Some(designcraft_geom::shapes::rectangle(rect)),
        "ellipse" => Some(designcraft_geom::shapes::ellipse(rect)),
        n => designcraft_geom::shapes::preset(n, rect),
    };
    if let Some(path) = path {
        let mut line: Vec<Pos2> = Vec::new();
        kurbo::flatten(path.to_bezpath(), 0.25, |el| {
            use designcraft_geom::PathEl::{ClosePath, LineTo, MoveTo};
            match el {
                MoveTo(p) | LineTo(p) => line.push(pos2(p.x as f32, p.y as f32)),
                ClosePath => {
                    if let Some(first) = line.first().copied() {
                        line.push(first);
                    }
                }
                _ => {}
            }
        });
        ui.painter().add(egui::Shape::line(line, ink));
    }
    resp.on_hover_text(label).clicked()
}

/// What the gallery's clicks do: draw a new shape, or change the selected one's.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Use {
    Draw,
    Change,
}

pub(super) fn gallery(app: &mut DesignApp, ui: &mut Ui, what: Use) {
    ui.set_max_width(290.0);
    let mut chosen = None;
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        if what == Use::Draw {
            ui.label(egui::RichText::new("Lines").strong());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                for (name, label) in LINES {
                    if tile(ui, name, label) {
                        chosen = Some(*name);
                    }
                }
                if tile(ui, "ellipse", "Freeform: Scribble") {
                    chosen = Some("scribble");
                }
            });
            ui.add_space(4.0);
        }
        for section in SECTIONS {
            ui.label(egui::RichText::new(section.title).strong());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                for (name, label) in section.shapes {
                    if tile(ui, name, label) {
                        chosen = Some(*name);
                    }
                }
            });
            ui.add_space(4.0);
        }
    });
    if what == Use::Change {
        ui.separator();
        let one = app.session.active().is_some_and(|d| d.selection.items.len() == 1);
        if ui.add_enabled(one, egui::Button::new("Edit Points")).clicked() {
            ui.close();
            super::edit_points(app);
            return;
        }
    }
    let Some(name) = chosen else { return };
    match what {
        Use::Draw if name == "scribble" => super::start_draw(app, "pencil"),
        Use::Draw => start(app, name),
        Use::Change => {
            let to: Value = json!(name);
            if let Err(e) = app.run("object.convertShape", json!({"to": to})) {
                app.status(format!("Couldn't change the shape: {e}"));
            }
        }
    }
    ui.close();
}

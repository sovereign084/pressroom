//! Publisher's colour menus: No Fill, a grid of the scheme's colours and the publication's own, and
//! More Colors. A page-layout program's list names its swatches by their ink recipes
//! (`C=100 M=0 Y=0 K=0`) and offers `[Registration]`; these show colours and call white White.

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::DesignApp;
use crate::dialogs::Dialog;

pub(crate) const MORE: &str = "pr.moreColors";

/// What a chosen colour goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Font,
    Fill,
    Line,
    Background,
    Cell,
    TextOutline,
}

impl Target {
    fn key(self) -> &'static str {
        match self {
            Target::Font => "font",
            Target::Fill => "fill",
            Target::Line => "line",
            Target::Background => "background",
            Target::Cell => "cell",
            Target::TextOutline => "textOutline",
        }
    }
    fn from_key(k: &str) -> Option<Self> {
        [Target::Font, Target::Fill, Target::Line, Target::Background, Target::Cell, Target::TextOutline].into_iter().find(|t| t.key() == k)
    }
}

/// Applies a colour by swatch name, `[None]` being none.
pub(super) fn apply(app: &mut DesignApp, target: Target, swatch: &str) {
    let result = match target {
        Target::Font => {
            let typing = crate::panels::text_attrs(app).is_some();
            if typing {
                app.run("type.char", json!({"attrs": {"fill": swatch}}))
            } else {
                super::set_next_text(&json!({"fill": swatch}));
                Ok(Value::Null)
            }
        }
        Target::Fill => app.run("object.fill", json!({"swatch": swatch})),
        Target::Line => app.run("object.stroke", json!({"swatch": swatch})),
        Target::Background => {
            super::set_background(app, (swatch != "[None]").then(|| swatch.to_string()));
            Ok(Value::Null)
        }
        Target::Cell => app.run("table.setCell", json!({"fill": swatch})),
        Target::TextOutline => {
            let weight = if swatch == "[None]" { 0.0 } else { 0.5 };
            app.run("type.char", json!({"attrs": {"stroke": swatch, "strokeWeight": weight, "strokeBehindFill": true}}))
        }
    };
    if let Err(e) = result {
        app.status(format!("Couldn't change the colour: {e}"));
    }
}

/// The name a swatch goes by in the menus.
fn shown_name(name: &str) -> String {
    match name {
        "[Paper]" => "White".to_string(),
        "[Black]" => "Black".to_string(),
        "[None]" => "No Color".to_string(),
        n => n.strip_prefix("Scheme ").map_or_else(|| n.to_string(), str::to_string),
    }
}

/// One square of the grid. Returns whether it was clicked.
fn chip(ui: &mut Ui, doc: &designcraft_doc::Document, name: &str, current: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    let (color, gradient) = crate::widgets::swatch_colors(doc, name, 1.0);
    let inner = Rect::from_center_size(r.center(), vec2(18.0, 18.0));
    crate::widgets::paint_chip(ui.painter(), inner, color, gradient);
    let edge = if resp.hovered() || current { Stroke::new(1.5, super::pal().select) } else { Stroke::new(1.0, Color32::from_black_alpha(60)) };
    ui.painter().rect_stroke(inner, 1.0, edge, egui::StrokeKind::Outside);
    resp.on_hover_text(shown_name(name)).clicked()
}

fn grid(ui: &mut Ui, doc: &designcraft_doc::Document, salt: &str, names: &[String], current: &str) -> Option<String> {
    let mut picked = None;
    egui::Grid::new(salt).spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            if chip(ui, doc, n, n == current) {
                picked = Some(n.clone());
            }
            if i % 10 == 9 {
                ui.end_row();
            }
        }
    });
    picked
}

/// The colour rows of a menu. Returns the swatch chosen.
pub(super) fn rows(app: &mut DesignApp, ui: &mut Ui, target: Target, no_label: Option<&str>, current: &str) -> Option<String> {
    let doc = app.session.active().map(|d| d.doc.clone())?;
    ui.set_min_width(250.0);
    let mut picked = None;
    if let Some(label) = no_label
        && ui.button(label).clicked()
    {
        picked = Some("[None]".to_string());
    }
    let is_scheme = |n: &str| n.starts_with("Scheme ");
    let mut scheme: Vec<String> = vec!["[Black]".to_string(), "[Paper]".to_string()];
    scheme.extend(doc.swatches.iter().map(|s| s.name.clone()).filter(|n| is_scheme(n)));
    let own: Vec<String> = doc
        .swatches
        .iter()
        .map(|s| s.name.clone())
        .filter(|n| !is_scheme(n) && !matches!(n.as_str(), "[None]" | "[Paper]" | "[Black]" | "[Registration]"))
        .collect();
    ui.label(egui::RichText::new("Scheme colors").color(super::pal().group_label));
    picked = picked.or(grid(ui, &doc, "pr_scheme_colors", &scheme, current));
    if !own.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Colors in this publication").color(super::pal().group_label));
        egui::ScrollArea::vertical().id_salt("pr_own_colors").max_height(120.0).show(ui, |ui| {
            picked = picked.take().or(grid(ui, &doc, "pr_own_colors_grid", &own, current));
        });
    }
    ui.separator();
    if ui.button("More Colors…").clicked() {
        open_more(app, target, current);
        ui.close();
    }
    if picked.is_some() {
        ui.close();
    }
    picked
}

/// A menu's whole body for `target`: chooses and applies.
pub(super) fn menu(app: &mut DesignApp, ui: &mut Ui, target: Target, no_label: Option<&str>, current: &str) {
    if let Some(name) = rows(app, ui, target, no_label, current) {
        apply(app, target, &name);
    }
}

/// A colour chip and arrow that opens the menu inside a dialog, for choosing without applying.
pub(super) fn picker(app: &mut DesignApp, ui: &mut Ui, salt: &str, target: Target, no_label: Option<&str>, current: &str) -> Option<String> {
    let doc = app.session.active().map(|d| d.doc.clone())?;
    let (color, gradient) = crate::widgets::swatch_colors(&doc, current, 1.0);
    let mut picked = None;
    ui.push_id(salt, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(64.0, 20.0), Sense::click());
        let chip = Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(40.0, 16.0));
        crate::widgets::paint_chip(ui.painter(), chip, color, gradient);
        ui.painter().rect_stroke(chip, 1.0, Stroke::new(1.0, super::pal().group_label), egui::StrokeKind::Outside);
        ui.painter().text(
            chip.right_center() + vec2(10.0, 0.0),
            egui::Align2::CENTER_CENTER,
            "⏷",
            egui::FontId::proportional(11.0),
            super::pal().text,
        );
        egui::Popup::menu(&resp).show(|ui| {
            picked = rows(app, ui, target, no_label, current);
        });
    });
    picked
}

fn open_more(app: &mut DesignApp, target: Target, current: &str) {
    let rgb = app.session.active().and_then(|d| crate::widgets::swatch_colors(&d.doc, current, 1.0).0).unwrap_or(Color32::BLACK);
    // A dialog already open (Format…) must not be lost: More Colors then isn't offered over it.
    if app.ui.dialog.is_some() {
        return;
    }
    app.ui.dialog = Some(Dialog::new(MORE, json!({"target": target.key(), "r": rgb.r(), "g": rgb.g(), "b": rgb.b()})));
}

pub(crate) fn more_body(ui: &mut Ui, d: &mut Dialog) {
    let channel = |d: &Dialog, k: &str| d.n(k).unwrap_or(0.0).clamp(0.0, 255.0) as u8;
    let mut c = Color32::from_rgb(channel(d, "r"), channel(d, "g"), channel(d, "b"));
    ui.label("Pick a color:");
    ui.add_space(4.0);
    if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
        d.fields.insert("r".into(), json!(c.r()));
        d.fields.insert("g".into(), json!(c.g()));
        d.fields.insert("b".into(), json!(c.b()));
    }
}

pub(crate) fn more_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let target = Target::from_key(&d.s("target")).ok_or("Choose a color first.")?;
    let channel = |k: &str| d.n(k).unwrap_or(0.0).clamp(0.0, 255.0).round() as u8;
    let (r, g, b) = (channel("r"), channel("g"), channel("b"));
    let name = format!("Custom #{r:02X}{g:02X}{b:02X}");
    if app.session.active().is_some_and(|st| st.doc.swatch(&name).is_none()) {
        app.run("swatch.create", json!({"name": name, "color": [r, g, b]}))?;
    }
    apply(app, target, &name);
    Ok(Value::Null)
}

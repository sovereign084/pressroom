//! Publisher's dialogs, in place of a page-layout program's: Font, Character Spacing, Paragraph,
//! Insert Hyperlink, Hyphenation, Rename, Move and Insert Page, Create Table, Layout Guides, Page
//! Setup, Format Object, and Date and Time. `crate::dialogs` shows them through one line per dialog
//! step (title, width, body, apply); everything else about them is here. Each sends only what
//! changed, so a mixed selection isn't flattened.

use designcraft_geom::units::{Unit, format_measure_prec, parse_measure};
use egui::Ui;
use serde_json::{Map, Value, json};

use super::colors::{self, Target};
use crate::DesignApp;
use crate::dialogs::Dialog;

pub(crate) const PREFIX: &str = "pr.";

const FONT: &str = "pr.font";
const SPACING: &str = "pr.spacing";
const PARAGRAPH: &str = "pr.paragraph";
const HYPERLINK: &str = "pr.hyperlink";
const HYPHENATION: &str = "pr.hyphenation";
const RENAME: &str = "pr.rename";
const MOVE: &str = "pr.movePage";
const INSERT_PAGE: &str = "pr.insertPage";
const TABLE: &str = "pr.table";
const GUIDES: &str = "pr.guides";
const SETUP: &str = "pr.pageSetup";
const FORMAT: &str = "pr.format";
const DATE: &str = "pr.date";
const NUMBER: &str = "pr.pageNumberFormat";
const APPLY_MASTER: &str = "pr.applyMaster";
const NEW_MASTER: &str = "pr.newMaster";
const OPTIONS: &str = "pr.options";

pub(crate) fn title(id: &str) -> &'static str {
    match id {
        FONT => "Font",
        SPACING => "Character Spacing",
        PARAGRAPH => "Paragraph",
        HYPERLINK => "Insert Hyperlink",
        HYPHENATION => "Hyphenation",
        RENAME => "Rename Page",
        MOVE => "Move Page",
        INSERT_PAGE => "Insert Page",
        TABLE => "Create Table",
        GUIDES => "Layout Guides",
        SETUP => "Page Setup",
        FORMAT => "Format",
        DATE => "Date and Time",
        NUMBER => "Page Number Format",
        APPLY_MASTER => "Apply Master Page",
        NEW_MASTER => "New Master Page",
        OPTIONS => "PressRoom Options",
        super::spelling::DIALOG => "Check Spelling",
        colors::MORE => "More Colors",
        _ => "",
    }
}

pub(crate) fn width(id: &str) -> f32 {
    match id {
        FONT | PARAGRAPH | FORMAT => 500.0,
        super::spelling::DIALOG => 440.0,
        colors::MORE => 340.0,
        _ => 400.0,
    }
}

/// The dialogs with only a Close: they act as their buttons are clicked.
pub(crate) fn close_only(id: &str) -> bool {
    id == super::spelling::DIALOG
}

pub(crate) fn body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    match d.id.as_str() {
        FONT => font_body(app, ui, d),
        SPACING => spacing_body(ui, d),
        PARAGRAPH => paragraph_body(app, ui, d),
        HYPERLINK => hyperlink_body(app, ui, d),
        HYPHENATION => hyphenation_body(app, ui, d),
        RENAME => rename_body(ui, d),
        MOVE => move_body(app, ui, d),
        INSERT_PAGE => insert_page_body(app, ui, d),
        TABLE => table_body(ui, d),
        GUIDES => guides_body(app, ui, d),
        SETUP => setup_body(app, ui, d),
        FORMAT => format_body(app, ui, d),
        DATE => date_body(ui, d),
        NUMBER => number_body(ui, d),
        APPLY_MASTER => apply_master_body(app, ui, d),
        NEW_MASTER => new_master_body(ui, d),
        OPTIONS => options_body(app, ui, d),
        super::spelling::DIALOG => super::spelling::body(app, ui, d),
        colors::MORE => colors::more_body(ui, d),
        _ => {}
    }
}

pub(crate) fn apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    match d.id.as_str() {
        FONT => font_apply(app, d),
        SPACING => spacing_apply(app, d),
        PARAGRAPH => paragraph_apply(app, d),
        HYPERLINK => hyperlink_apply(app, d),
        HYPHENATION => hyphenation_apply(app, d),
        RENAME => app.run("layout.pages.setTitle", json!({"page": d.n("page").unwrap_or(0.0) as u64, "title": d.s("title")})),
        MOVE => move_apply(app, d),
        INSERT_PAGE => insert_page_apply(app, d),
        TABLE => table_apply(app, d),
        GUIDES => guides_apply(app, d),
        SETUP => setup_apply(app, d),
        FORMAT => format_apply(app, d),
        DATE => date_apply(app, d),
        NUMBER => number_apply(app, d),
        APPLY_MASTER => apply_master_apply(app, d),
        NEW_MASTER => app.run("layout.parents.new", json!({"name": d.s("name")})),
        colors::MORE => colors::more_apply(app, d),
        OPTIONS => options_apply(app, d),
        _ => Ok(Value::Null),
    }
}

// ───────────────────────────── shared pieces ─────────────────────────────

fn open(app: &mut DesignApp, id: &str, fields: Value) {
    app.ui.dialog = Some(Dialog::new(id, fields));
}

/// A row of label and field in a dialog's grid.
fn labelled(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.label(label);
    ui.horizontal(add);
    ui.end_row();
}

fn text(ui: &mut Ui, d: &mut Dialog, key: &str, width: f32) {
    let mut s = d.s(key);
    if ui.add(egui::TextEdit::singleline(&mut s).desired_width(width)).changed() {
        d.fields.insert(key.into(), Value::String(s));
    }
}

fn check(ui: &mut Ui, d: &mut Dialog, key: &str, label: &str) {
    let mut b = d.b(key);
    if ui.checkbox(&mut b, label).changed() {
        d.fields.insert(key.into(), Value::Bool(b));
    }
}

fn combo(ui: &mut Ui, d: &mut Dialog, key: &str, choices: &[(&str, &str)], width: f32) {
    let cur = d.s(key);
    let shown = choices.iter().find(|(v, _)| *v == cur).map_or(cur.as_str(), |(_, l)| l).to_string();
    craft_ui::ComboBox::from_id_salt(("pr_combo", key)).width(width).selected_text(shown).show_ui(ui, |ui| {
        for (v, l) in choices {
            if ui.selectable_label(cur == *v, *l).clicked() {
                d.fields.insert(key.into(), json!(v));
            }
        }
    });
}

fn tabs(ui: &mut Ui, d: &mut Dialog, names: &[(&str, &str)]) {
    let cur = d.s("tab");
    ui.horizontal(|ui| {
        for (key, label) in names {
            if ui.selectable_label(cur == *key, *label).clicked() {
                d.fields.insert("tab".into(), json!(key));
            }
        }
    });
    ui.separator();
    ui.add_space(4.0);
}

/// The publication's ruler unit, which measures in these dialogs follow.
fn unit(app: &DesignApp) -> Unit {
    app.session.active().map_or(Unit::Inches, |st| st.doc.settings.horizontal_units)
}

fn measure(pt: f64, unit: Unit) -> Value {
    json!(format_measure_prec(pt, unit, 2))
}

/// The points in a measure field, or a message saying which field to fix.
fn read(d: &Dialog, key: &str, label: &str, unit: Unit) -> Result<f64, String> {
    parse_measure(d.s(key).trim(), unit).map_err(|_| format!("{label}: type a measurement such as 1.5\"."))
}

/// Whether a field differs from what the dialog opened with.
fn changed(d: &Dialog, key: &str) -> bool {
    d.fields.get(key) != d.fields.get("init").and_then(|i| i.get(key))
}

/// A copy of the fields as they are, kept to compare with on OK.
fn with_init(mut fields: Value) -> Value {
    if let Some(o) = fields.as_object_mut() {
        let snapshot: Map<String, Value> = o.clone();
        o.insert("init".into(), Value::Object(snapshot));
    }
    fields
}

/// The caret's selection widened to whole paragraphs, for what Publisher sets by the paragraph: the
/// steps to run `steps` over them and put the selection back.
pub(super) fn over_paragraphs(app: &DesignApp, steps: Vec<Value>) -> Vec<Value> {
    let Some(st) = app.session.active() else { return steps };
    let Some(t) = st.selection.text.filter(|t| t.cell.is_none()) else { return steps };
    let Some(story) = st.doc.story(t.story) else { return steps };
    let r = t.range();
    let txt = &story.text;
    let a = txt.get(..r.start).and_then(|h| h.rfind('\n')).map_or(0, |i| i + 1);
    let b = txt.get(r.end..).and_then(|h| h.find('\n')).map_or(txt.len(), |i| r.end + i);
    let mut out = vec![json!({"command": "text.select", "params": {"story": t.story.0, "anchor": a, "focus": b}})];
    out.extend(steps);
    out.push(json!({"command": "text.select", "params": {"story": t.story.0, "anchor": t.anchor, "focus": t.focus}}));
    out
}

// ───────────────────────────── Font ─────────────────────────────

pub(crate) fn open_font(app: &mut DesignApp) {
    let chars = crate::panels::text_attrs(app).map(|a| a["chars"].clone()).unwrap_or(Value::Null);
    let family = chars["fontFamily"].as_str().unwrap_or("").to_string();
    let style = chars["fontStyle"].as_str().unwrap_or("").to_string();
    let position = chars["position"].as_str().unwrap_or("normal").to_string();
    let caps = chars["capitalization"].as_str().unwrap_or("normal").to_string();
    let weight = |b: bool, i: bool| match (b, i) {
        (true, true) => "boldItalic",
        (true, false) => "bold",
        (false, true) => "italic",
        _ => "regular",
    };
    let fields = json!({
        "font": if family.is_empty() { String::new() } else { super::fontnames::windows_name(&family, &style) },
        "style": weight(super::fontnames::is_bold(&style), super::fontnames::is_italic(&style)),
        "size": chars["size"].as_f64().map_or(String::new(), |v| v.to_string()),
        "fill": chars["fill"].as_str().unwrap_or("[Black]"),
        "underline": chars["underline"].as_bool().unwrap_or(false) || chars["underline"].is_object(),
        "strike": chars["strikethrough"].as_bool().unwrap_or(false),
        "superscript": position == "superscript",
        "subscript": position == "subscript",
        "allCaps": caps == "allCaps",
        "smallCaps": caps == "smallCaps",
    });
    open(app, FONT, with_init(fields));
}

fn font_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    egui::Grid::new("pr_font").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label("Font:");
        let shown = d.s("font");
        if let Some(name) = super::fontnames::picker(app, ui, &shown, 260.0, true) {
            d.fields.insert("font".into(), json!(name));
        }
        ui.end_row();
        labelled(ui, "Font style:", |ui| {
            combo(ui, d, "style", &[("regular", "Regular"), ("italic", "Italic"), ("bold", "Bold"), ("boldItalic", "Bold Italic")], 150.0);
        });
        labelled(ui, "Size:", |ui| text(ui, d, "size", 60.0));
        ui.label("Font color:");
        let cur = d.s("fill");
        if let Some(c) = colors::picker(app, ui, "pr_font_color", Target::Font, None, &cur) {
            d.fields.insert("fill".into(), json!(c));
        }
        ui.end_row();
    });
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Effects").strong());
    ui.add_space(4.0);
    egui::Grid::new("pr_font_fx").num_columns(2).spacing([40.0, 6.0]).show(ui, |ui| {
        check(ui, d, "underline", "Underline");
        check(ui, d, "strike", "Strikethrough");
        ui.end_row();
        check(ui, d, "superscript", "Superscript");
        check(ui, d, "subscript", "Subscript");
        ui.end_row();
        check(ui, d, "allCaps", "All caps");
        check(ui, d, "smallCaps", "Small caps");
        ui.end_row();
    });
    // One each way: turning one on turns its pair off.
    for (a, b) in [("superscript", "subscript"), ("allCaps", "smallCaps")] {
        if d.b(a) && d.b(b) {
            let was_a = d.fields.get("init").and_then(|i| i.get(a)).and_then(Value::as_bool).unwrap_or(false);
            d.fields.insert(if was_a { a } else { b }.into(), json!(false));
        }
    }
}

fn font_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let mut attrs = Map::new();
    if changed(d, "font") || changed(d, "style") {
        let style = d.s("style");
        let (bold, italic) = (style.starts_with("bold") || style == "bold", style.ends_with("talic"));
        let (family, face) = super::fontnames::resolve(app, &d.s("font"), bold, italic);
        attrs.insert("fontFamily".into(), json!(family));
        attrs.insert("fontStyle".into(), json!(face));
    }
    if changed(d, "size") {
        let size = d.n("size").filter(|v| v.is_finite() && *v >= 1.0 && *v <= 1296.0).ok_or("Size: type a number from 1 to 1296.")?;
        attrs.insert("size".into(), json!(size));
    }
    if changed(d, "fill") {
        attrs.insert("fill".into(), json!(d.s("fill")));
    }
    if changed(d, "underline") {
        attrs.insert("underline".into(), json!(d.b("underline")));
    }
    if changed(d, "strike") {
        attrs.insert("strikethrough".into(), json!(d.b("strike")));
    }
    if changed(d, "superscript") || changed(d, "subscript") {
        let p = if d.b("superscript") {
            "superscript"
        } else if d.b("subscript") {
            "subscript"
        } else {
            "normal"
        };
        attrs.insert("position".into(), json!(p));
    }
    if changed(d, "allCaps") || changed(d, "smallCaps") {
        let c = if d.b("allCaps") {
            "allCaps"
        } else if d.b("smallCaps") {
            "smallCaps"
        } else {
            "normal"
        };
        attrs.insert("capitalization".into(), json!(c));
    }
    if attrs.is_empty() {
        return Ok(Value::Null);
    }
    if crate::panels::text_attrs(app).is_some() {
        app.run("type.char", json!({"attrs": attrs}))
    } else {
        super::set_next_text(&Value::Object(attrs));
        Ok(Value::Null)
    }
}

// ───────────────────────────── Character Spacing ─────────────────────────────

pub(crate) fn open_spacing(app: &mut DesignApp) {
    let chars = crate::panels::text_attrs(app).map(|a| a["chars"].clone()).unwrap_or(Value::Null);
    let fields = json!({
        "scale": chars["hScale"].as_f64().unwrap_or(1.0) * 100.0,
        "tracking": chars["tracking"].as_f64().unwrap_or(0.0),
        "shift": chars["baselineShift"].as_f64().unwrap_or(0.0),
    });
    open(app, SPACING, with_init(fields));
}

fn spacing_body(ui: &mut Ui, d: &mut Dialog) {
    egui::Grid::new("pr_spacing").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        labelled(ui, "Scaling (% of normal width):", |ui| text(ui, d, "scale", 60.0));
        labelled(ui, "Tracking (1000ths of a size):", |ui| text(ui, d, "tracking", 60.0));
        labelled(ui, "Raise text by (pt):", |ui| text(ui, d, "shift", 60.0));
    });
}

fn spacing_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let mut attrs = Map::new();
    if changed(d, "scale") {
        let v = d.n("scale").filter(|v| *v >= 1.0 && *v <= 1000.0).ok_or("Scaling: type a percentage from 1 to 1000.")?;
        attrs.insert("hScale".into(), json!(v / 100.0));
    }
    if changed(d, "tracking") {
        attrs.insert("tracking".into(), json!(d.n("tracking").filter(|v| v.is_finite()).ok_or("Tracking: type a number.")?));
    }
    if changed(d, "shift") {
        attrs.insert("baselineShift".into(), json!(d.n("shift").filter(|v| v.is_finite()).ok_or("Raise text by: type a number.")?));
    }
    if attrs.is_empty() {
        return Ok(Value::Null);
    }
    app.run("type.char", json!({"attrs": attrs}))
}

// ───────────────────────────── Paragraph ─────────────────────────────

pub(crate) fn open_paragraph(app: &mut DesignApp) {
    let Some(a) = crate::panels::text_attrs(app) else {
        app.status("Click in some text first.");
        return;
    };
    let (para, chars) = (&a["para"], &a["chars"]);
    let u = unit(app);
    let first = para["firstLineIndent"].as_f64().unwrap_or(0.0);
    let size = chars["size"].as_f64().unwrap_or(12.0).max(1.0);
    let between = match chars["leading"]["kind"].as_str() {
        Some("points") => format!("{:.2}sp", chars["leading"]["value"].as_f64().unwrap_or(size * 1.2) / (size * 1.2)),
        _ => format!("{:.2}sp", para["autoLeading"].as_f64().unwrap_or(1.2) / 1.2),
    };
    let fields = json!({
        "tab": "indents",
        "align": para["align"].as_str().unwrap_or("left"),
        "left": measure(para["leftIndent"].as_f64().unwrap_or(0.0), u),
        "right": measure(para["rightIndent"].as_f64().unwrap_or(0.0), u),
        "firstKind": if first > 0.0 { "first" } else if first < 0.0 { "hanging" } else { "normal" },
        "firstBy": measure(first.abs(), u),
        "before": measure(para["spaceBefore"].as_f64().unwrap_or(0.0), u),
        "after": measure(para["spaceAfter"].as_f64().unwrap_or(0.0), u),
        "between": between,
        "widow": para["keepFirst"].as_u64().unwrap_or(2) >= 2 && para["keepLast"].as_u64().unwrap_or(2) >= 2,
        "keepNext": para["keepWithNext"].as_u64().unwrap_or(0) > 0,
        "keepLines": para["keepLinesTogether"].as_bool().unwrap_or(false) || para["keepAllLines"].as_bool().unwrap_or(false),
        "nextBox": para["startParagraph"].as_str() == Some("nextFrame"),
        "tabAt": "",
        "tabAlign": "left",
        "tabLeader": "",
    });
    open(app, PARAGRAPH, with_init(fields));
}

fn paragraph_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    tabs(ui, d, &[("indents", "Indents and Spacing"), ("breaks", "Line and Paragraph Breaks"), ("tabs", "Tabs")]);
    match d.s("tab").as_str() {
        "breaks" => {
            check(ui, d, "widow", "Widow/Orphan control");
            check(ui, d, "keepNext", "Keep with next");
            check(ui, d, "keepLines", "Keep lines together");
            check(ui, d, "nextBox", "Start in next text box");
        }
        "tabs" => tabs_section(app, ui, d),
        _ => {
            egui::Grid::new("pr_para").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                labelled(ui, "Alignment:", |ui| {
                    combo(
                        ui,
                        d,
                        "align",
                        &[
                            ("left", "Left"),
                            ("center", "Center"),
                            ("right", "Right"),
                            ("leftJustified", "Justified"),
                            ("fullyJustified", "Distributed"),
                        ],
                        130.0,
                    );
                });
                labelled(ui, "Before text:", |ui| text(ui, d, "left", 70.0));
                labelled(ui, "First line:", |ui| {
                    combo(ui, d, "firstKind", &[("normal", "(none)"), ("first", "First line"), ("hanging", "Hanging")], 100.0);
                    text(ui, d, "firstBy", 70.0);
                });
                labelled(ui, "After text:", |ui| text(ui, d, "right", 70.0));
                labelled(ui, "Before paragraphs:", |ui| text(ui, d, "before", 70.0));
                labelled(ui, "After paragraphs:", |ui| text(ui, d, "after", 70.0));
                labelled(ui, "Between lines:", |ui| text(ui, d, "between", 70.0));
            });
        }
    }
}

fn tabs_section(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let u = unit(app);
    let got = app.session.execute("type.tabs.get", &json!({})).unwrap_or(Value::Null);
    let stops = got["tabs"].as_array().cloned().unwrap_or_default();
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        ui.set_min_height(100.0);
        if stops.is_empty() {
            ui.label(egui::RichText::new("No tab stops set").color(super::pal().group_label));
        }
        for (i, s) in stops.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format_measure_prec(s["position"].as_f64().unwrap_or(0.0), u, 2));
                ui.label(s["align"].as_str().unwrap_or("left"));
                if ui.small_button("Clear").clicked() {
                    let _ = app.run("type.tabs.remove", json!({"index": i}));
                }
            });
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Tab stop position:");
        text(ui, d, "tabAt", 70.0);
        combo(ui, d, "tabAlign", &[("left", "Left"), ("center", "Center"), ("right", "Right"), ("char", "Decimal")], 80.0);
    });
    ui.horizontal(|ui| {
        ui.label("Leader:");
        combo(ui, d, "tabLeader", &[("", "None"), (".", "Dot"), ("-", "Dash"), ("_", "Line")], 80.0);
        if ui.button("Set").clicked()
            && let Ok(at) = read(d, "tabAt", "Tab stop position", u)
        {
            let mut p = json!({"position": at, "align": d.s("tabAlign")});
            if !d.s("tabLeader").is_empty() {
                p["leader"] = json!(d.s("tabLeader"));
            }
            let _ = app.run("type.tabs.add", p);
            d.fields.insert("tabAt".into(), json!(""));
        }
        if ui.button("Clear All").clicked() {
            let _ = app.run("type.tabs.clear", json!({}));
        }
    });
}

/// `1.5sp`, `1.5` (lines) or `14pt` (points) as line spacing for this size: the leading in points.
fn parse_between(s: &str, size: f64) -> Option<f64> {
    let t = s.trim().to_lowercase();
    if let Some(pt) = t.strip_suffix("pt") {
        return pt.trim().parse::<f64>().ok().filter(|v| *v > 0.0 && v.is_finite());
    }
    let lines = t.strip_suffix("sp").unwrap_or(&t).trim().parse::<f64>().ok().filter(|v| *v > 0.0 && v.is_finite())?;
    Some(size * 1.2 * lines)
}

fn paragraph_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let u = unit(app);
    let mut para = Map::new();
    if changed(d, "align") {
        para.insert("align".into(), json!(d.s("align")));
    }
    for (key, attr, label) in [
        ("left", "leftIndent", "Before text"),
        ("right", "rightIndent", "After text"),
        ("before", "spaceBefore", "Before paragraphs"),
        ("after", "spaceAfter", "After paragraphs"),
    ] {
        if changed(d, key) {
            para.insert(attr.into(), json!(read(d, key, label, u)?));
        }
    }
    if changed(d, "firstKind") || changed(d, "firstBy") {
        let by = read(d, "firstBy", "First line", u)?;
        let v = match d.s("firstKind").as_str() {
            "first" => by,
            "hanging" => -by,
            _ => 0.0,
        };
        para.insert("firstLineIndent".into(), json!(v));
    }
    if changed(d, "widow") {
        let n = if d.b("widow") { 2 } else { 1 };
        para.insert("keepFirst".into(), json!(n));
        para.insert("keepLast".into(), json!(n));
    }
    if changed(d, "keepNext") {
        para.insert("keepWithNext".into(), json!(u32::from(d.b("keepNext"))));
    }
    if changed(d, "keepLines") {
        para.insert("keepLinesTogether".into(), json!(d.b("keepLines")));
        para.insert("keepAllLines".into(), json!(d.b("keepLines")));
    }
    if changed(d, "nextBox") {
        para.insert("startParagraph".into(), json!(if d.b("nextBox") { "nextFrame" } else { "anywhere" }));
    }
    let mut steps = Vec::new();
    if !para.is_empty() {
        steps.push(json!({"command": "type.para", "params": {"attrs": para}}));
    }
    if changed(d, "between") {
        let size = crate::panels::text_attrs(app).and_then(|a| a["chars"]["size"].as_f64()).unwrap_or(12.0).max(1.0);
        let lead = parse_between(&d.s("between"), size).ok_or("Between lines: type lines such as 1.5sp, or points such as 14pt.")?;
        steps
            .extend(over_paragraphs(app, vec![json!({"command": "type.char", "params": {"attrs": {"leading": {"kind": "points", "value": lead}}}})]));
    }
    if steps.is_empty() {
        return Ok(Value::Null);
    }
    app.run("script.run", json!({"text": Value::Array(steps).to_string()}))
}

// ───────────────────────────── Insert Hyperlink ─────────────────────────────

pub(crate) fn open_hyperlink(app: &mut DesignApp) {
    let Some(st) = app.session.active() else { return };
    let usable = st.selection.text.is_some_and(|t| !t.is_caret()) || !st.selection.items.is_empty();
    if !usable {
        app.status("Select the text or object to link first.");
        return;
    }
    let existing = hyperlink_of_selection(app);
    let (kind, address) = existing.as_ref().map_or(("web", String::new()), |h| {
        let dest = &h["dest"];
        if let Some(u) = dest.get("url").and_then(Value::as_str) {
            ("web", u.to_string())
        } else if let Some(e) = dest.get("email").and_then(Value::as_str) {
            ("email", e.to_string())
        } else {
            ("page", dest.get("page").map_or(String::new(), Value::to_string))
        }
    });
    open(app, HYPERLINK, json!({"kind": kind, "address": address, "existing": existing.as_ref().map(|h| h["id"].clone())}));
}

/// The hyperlink already on the selected text or object.
fn hyperlink_of_selection(app: &mut DesignApp) -> Option<Value> {
    let st = app.session.active()?;
    let text = st.selection.text.filter(|t| !t.is_caret()).map(|t| (t.story.0, t.range()));
    let item = st.selection.items.first().map(|i| i.0);
    let list = app.session.execute("hyperlink.list", &json!({})).ok()?;
    list.as_array()?
        .iter()
        .find(|h| {
            let src = &h["source"];
            match (&text, item) {
                (Some((story, r)), _) => {
                    src["story"].as_u64() == Some(*story)
                        && src["start"].as_u64().is_some_and(|a| (a as usize) < r.end)
                        && src["end"].as_u64().is_some_and(|b| (b as usize) > r.start)
                }
                (None, Some(id)) => src["id"].as_u64() == Some(id),
                _ => false,
            }
        })
        .cloned()
}

fn hyperlink_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    ui.label("Link to:");
    ui.horizontal(|ui| {
        for (k, label) in [("web", "Web page"), ("email", "E-mail address"), ("page", "Page in this publication")] {
            if ui.radio(d.s("kind") == k, label).clicked() {
                d.fields.insert("kind".into(), json!(k));
            }
        }
    });
    ui.add_space(8.0);
    let label = match d.s("kind").as_str() {
        "email" => "E-mail address:",
        "page" => "Page number:",
        _ => "Address:",
    };
    ui.label(label);
    text(ui, d, "address", 370.0);
    if let Some(id) = d.fields.get("existing").and_then(Value::as_u64) {
        ui.add_space(8.0);
        if ui.button("Remove Link").clicked() {
            let _ = app.run("hyperlink.delete", json!({"id": id}));
            d.fields.remove("existing");
            d.fields.insert("address".into(), json!(""));
        }
    }
}

fn hyperlink_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let address = d.s("address").trim().to_string();
    if address.is_empty() {
        return Ok(Value::Null);
    }
    let mut p = match d.s("kind").as_str() {
        "email" => json!({"email": address}),
        "page" => json!({"page": address.parse::<u64>().map_err(|_| "Page number: type a page number.")?}),
        _ => {
            let url = if address.contains("://") { address } else { format!("https://{address}") };
            json!({"url": url})
        }
    };
    if let Some(id) = d.fields.get("existing").and_then(Value::as_u64) {
        p["id"] = json!(id);
        return app.run("hyperlink.edit", p);
    }
    app.run("hyperlink.create", p)
}

// ───────────────────────────── Hyphenation ─────────────────────────────

pub(crate) fn open_hyphenation(app: &mut DesignApp) {
    let Some(a) = crate::panels::text_attrs(app) else {
        app.status("Click in some text first.");
        return;
    };
    let u = unit(app);
    let fields = json!({
        "auto": a["para"]["hyphenate"].as_bool().unwrap_or(true),
        "zone": measure(a["para"]["hyphZone"].as_f64().unwrap_or(36.0), u),
    });
    open(app, HYPHENATION, with_init(fields));
}

fn hyphenation_body(_app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    check(ui, d, "auto", "Automatically hyphenate this story");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Hyphenation zone:");
        text(ui, d, "zone", 70.0);
    });
}

fn hyphenation_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    if !changed(d, "auto") && !changed(d, "zone") {
        return Ok(Value::Null);
    }
    let zone = read(d, "zone", "Hyphenation zone", unit(app))?;
    let attrs = json!({"hyphenate": d.b("auto"), "hyphZone": zone});
    // The whole story, as Publisher sets it, and the selection back afterwards.
    let Some(t) = app.session.active().and_then(|st| st.selection.text) else {
        return app.run("type.para", json!({"attrs": attrs}));
    };
    let len = app.session.active().and_then(|st| st.doc.story(t.story).map(|s| s.text.len())).unwrap_or(0);
    let steps = json!([
        {"command": "text.select", "params": {"story": t.story.0, "anchor": 0, "focus": len}},
        {"command": "type.para", "params": {"attrs": attrs}},
        {"command": "text.select", "params": {"story": t.story.0, "anchor": t.anchor, "focus": t.focus}},
    ]);
    app.run("script.run", json!({"text": steps.to_string()}))
}

// ───────────────────────────── Pages ─────────────────────────────

pub(crate) fn open_rename(app: &mut DesignApp, page: usize) {
    let title = app.session.active().and_then(|st| st.doc.page(page).and_then(|p| p.title.clone())).unwrap_or_default();
    open(app, RENAME, json!({"page": page, "title": title}));
}

fn rename_body(ui: &mut Ui, d: &mut Dialog) {
    ui.label(format!("Name for page {}:", d.n("page").unwrap_or(0.0) as u64 + 1));
    text(ui, d, "title", 370.0);
}

pub(crate) fn open_move(app: &mut DesignApp, page: usize) {
    open(app, MOVE, json!({"from": page, "where": "before", "to": 0}));
}

fn move_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let n = app.session.active().map_or(0, |st| st.doc.page_count());
    let from = d.n("from").unwrap_or(0.0) as usize;
    ui.label(format!("Move page {}:", from + 1));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        combo(ui, d, "where", &[("before", "Before page"), ("after", "After page")], 120.0);
        let to = d.n("to").unwrap_or(0.0) as usize;
        craft_ui::ComboBox::from_id_salt("pr_move_to").width(70.0).selected_text((to + 1).to_string()).show_ui(ui, |ui| {
            for i in (0..n).filter(|i| *i != from) {
                if ui.selectable_label(to == i, (i + 1).to_string()).clicked() {
                    d.fields.insert("to".into(), json!(i));
                }
            }
        });
    });
}

/// The index a page ends up at when moved before or after another, counting from before the move.
pub(super) fn move_target(from: usize, to: usize, after: bool) -> usize {
    let landing = if after { to + 1 } else { to };
    if landing > from { landing - 1 } else { landing }
}

fn move_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let from = d.n("from").unwrap_or(0.0) as usize;
    let to = d.n("to").unwrap_or(0.0) as usize;
    let dest = move_target(from, to, d.s("where") == "after");
    if from == dest {
        return Ok(Value::Null);
    }
    app.run("layout.pages.move", json!({"from": from, "to": dest}))
}

pub(crate) fn open_insert_page(app: &mut DesignApp) {
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    open(app, INSERT_PAGE, json!({"count": "1", "where": "after", "kind": "blank", "from": cur}));
}

fn insert_page_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let n = app.session.active().map_or(0, |st| st.doc.page_count());
    ui.horizontal(|ui| {
        ui.label("Number of new pages:");
        text(ui, d, "count", 50.0);
    });
    ui.add_space(6.0);
    let cur = crate::canvas::current_page(app).unwrap_or(0) + 1;
    for (k, label) in [("before", format!("Before page {cur}")), ("after", format!("After page {cur}"))] {
        if ui.radio(d.s("where") == k, label).clicked() {
            d.fields.insert("where".into(), json!(k));
        }
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Options").strong());
    for (k, label) in
        [("blank", "Insert blank pages"), ("textbox", "Create one text box on each page"), ("duplicate", "Duplicate all objects on page:")]
    {
        ui.horizontal(|ui| {
            if ui.radio(d.s("kind") == k, label).clicked() {
                d.fields.insert("kind".into(), json!(k));
            }
            if k == "duplicate" {
                let from = d.n("from").unwrap_or(0.0) as usize;
                craft_ui::ComboBox::from_id_salt("pr_dup_from").width(60.0).selected_text((from + 1).to_string()).show_ui(ui, |ui| {
                    for i in 0..n {
                        if ui.selectable_label(from == i, (i + 1).to_string()).clicked() {
                            d.fields.insert("from".into(), json!(i));
                        }
                    }
                });
            }
        });
    }
}

fn insert_page_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let count = d.n("count").filter(|v| *v >= 1.0 && *v <= 200.0).ok_or("Number of new pages: type a number from 1 to 200.")? as usize;
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    let after = if d.s("where") == "before" { cur.checked_sub(1) } else { Some(cur) };
    match d.s("kind").as_str() {
        "duplicate" => {
            let from = d.n("from").unwrap_or(0.0) as usize;
            for _ in 0..count {
                super::duplicate_page(app, from);
            }
            Ok(Value::Null)
        }
        kind => {
            let r = app.run("layout.pages.insert", json!({"count": count, "after": after.unwrap_or(0)}))?;
            if after.is_none() {
                app.run("layout.pages.move", json!({"from": 0, "to": count}))?;
            }
            if kind == "textbox" {
                let first = after.map_or(0, |a| a + 1);
                for page in first..first + count {
                    add_page_text_box(app, page)?;
                }
            }
            Ok(r)
        }
    }
}

/// A text box filling the page's margins.
fn add_page_text_box(app: &mut DesignApp, page: usize) -> Result<Value, String> {
    let (si, rect) = {
        let st = app.session.active().ok_or("Open a publication first.")?;
        let (si, pi) = st.doc.page_loc(page).ok_or("No such page.")?;
        let pg = st.doc.spreads.get(si).and_then(|s| s.pages.get(pi)).ok_or("No such page.")?;
        let m = &pg.margins;
        (si, [pg.x + m.inside, m.top, pg.x + pg.width - m.outside, pg.height - m.bottom])
    };
    app.run("frame.create", json!({"spread": si, "rect": rect, "content": "text"}))
}

// ───────────────────────────── Create Table ─────────────────────────────

pub(crate) fn open_table(app: &mut DesignApp) {
    open(app, TABLE, json!({"rows": "4", "cols": "4"}));
}

fn table_body(ui: &mut Ui, d: &mut Dialog) {
    egui::Grid::new("pr_table").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        labelled(ui, "Number of rows:", |ui| text(ui, d, "rows", 60.0));
        labelled(ui, "Number of columns:", |ui| text(ui, d, "cols", 60.0));
    });
}

fn table_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let rows = d.n("rows").filter(|v| *v >= 1.0 && *v <= 256.0).ok_or("Number of rows: type a number from 1 to 256.")? as u64;
    let cols = d.n("cols").filter(|v| *v >= 1.0 && *v <= 128.0).ok_or("Number of columns: type a number from 1 to 128.")? as u64;
    super::insert_table_of(app, rows, cols)
}

// ───────────────────────────── Layout Guides and Page Setup ─────────────────────────────

fn page_fields(app: &DesignApp) -> Option<Value> {
    let st = app.session.active()?;
    let page = crate::canvas::current_page(app).unwrap_or(0);
    let p = st.doc.page(page)?;
    let u = unit(app);
    Some(json!({
        "left": measure(p.margins.inside, u),
        "right": measure(p.margins.outside, u),
        "top": measure(p.margins.top, u),
        "bottom": measure(p.margins.bottom, u),
        "columns": p.columns.count.to_string(),
        "gutter": measure(p.columns.gutter, u),
        "width": measure(p.width, u),
        "height": measure(p.height, u),
    }))
}

pub(crate) fn open_guides(app: &mut DesignApp) {
    let Some(mut f) = page_fields(app) else { return };
    f["tab"] = json!("margins");
    open(app, GUIDES, with_init(f));
}

fn margin_rows(ui: &mut Ui, d: &mut Dialog) {
    egui::Grid::new("pr_margins").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        labelled(ui, "Left:", |ui| text(ui, d, "left", 70.0));
        labelled(ui, "Right:", |ui| text(ui, d, "right", 70.0));
        labelled(ui, "Top:", |ui| text(ui, d, "top", 70.0));
        labelled(ui, "Bottom:", |ui| text(ui, d, "bottom", 70.0));
    });
}

fn guides_body(_app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    tabs(ui, d, &[("margins", "Margin Guides"), ("grid", "Grid Guides")]);
    if d.s("tab") == "grid" {
        egui::Grid::new("pr_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            labelled(ui, "Columns:", |ui| text(ui, d, "columns", 70.0));
            labelled(ui, "Spacing:", |ui| text(ui, d, "gutter", 70.0));
        });
    } else {
        margin_rows(ui, d);
    }
}

fn margins_params(app: &DesignApp, d: &Dialog) -> Result<Option<Value>, String> {
    if !["left", "right", "top", "bottom"].iter().any(|k| changed(d, k)) {
        return Ok(None);
    }
    let u = unit(app);
    Ok(Some(json!({
        "inside": read(d, "left", "Left", u)?,
        "outside": read(d, "right", "Right", u)?,
        "top": read(d, "top", "Top", u)?,
        "bottom": read(d, "bottom", "Bottom", u)?,
    })))
}

fn guides_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let mut p = json!({});
    if let Some(m) = margins_params(app, d)? {
        p["margins"] = m;
    }
    if changed(d, "columns") || changed(d, "gutter") {
        p["columns"] = json!(d.n("columns").filter(|v| *v >= 1.0 && *v <= 216.0).ok_or("Columns: type a number from 1 to 216.")? as u64);
        p["gutter"] = json!(read(d, "gutter", "Spacing", unit(app))?);
    }
    if p.as_object().is_none_or(Map::is_empty) {
        return Ok(Value::Null);
    }
    app.run("layout.marginsAndColumns", p)
}

pub(crate) fn open_setup(app: &mut DesignApp) {
    let Some(f) = page_fields(app) else { return };
    open(app, SETUP, with_init(f));
}

fn setup_body(_app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    ui.label(egui::RichText::new("Page").strong());
    egui::Grid::new("pr_setup").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        labelled(ui, "Width:", |ui| text(ui, d, "width", 70.0));
        labelled(ui, "Height:", |ui| text(ui, d, "height", 70.0));
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Margin Guides").strong());
    margin_rows(ui, d);
}

fn setup_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let u = unit(app);
    let mut steps = Vec::new();
    if changed(d, "width") || changed(d, "height") {
        let (w, h) = (read(d, "width", "Width", u)?, read(d, "height", "Height", u)?);
        if !(1.0..=15_552.0).contains(&w) || !(1.0..=15_552.0).contains(&h) {
            return Err("The page must be between 0.01 and 216 inches each way.".into());
        }
        steps.push(json!({"command": "layout.documentSetup", "params": {"width": w, "height": h}}));
    }
    if let Some(m) = margins_params(app, d)? {
        steps.push(json!({"command": "layout.marginsAndColumns", "params": {"margins": m}}));
    }
    if steps.is_empty() {
        return Ok(Value::Null);
    }
    app.run("script.run", json!({"text": Value::Array(steps).to_string()}))
}

// ───────────────────────────── Format Object ─────────────────────────────

pub(crate) fn open_format(app: &mut DesignApp) {
    let Some(info) = crate::panels::sel_info(app) else {
        app.status("Select an object first.");
        return;
    };
    let u = unit(app);
    let r = info.page_rect;
    let text = app
        .session
        .active()
        .and_then(|st| st.selection.items.first().and_then(|id| st.doc.item(*id)).and_then(|it| it.text_frame()).map(|t| t.options.clone()));
    let inset = text.as_ref().map_or([0.0; 4], |o| o.inset);
    let rotation = app.session.execute("transform.info", &json!({})).ok().and_then(|v| v["rotation"].as_f64()).unwrap_or(0.0);
    let alt = app
        .session
        .active()
        .and_then(|st| st.selection.items.first().and_then(|id| st.doc.item(*id)).map(|it| it.alt_text.clone()))
        .unwrap_or_default();
    let fields = json!({
        "tab": "colors",
        "isText": text.is_some(),
        "isGraphic": info.is_graphic,
        "fill": info.fill,
        "line": info.stroke,
        "weight": info.stroke_weight.to_string(),
        "dash": info.stroke_kind,
        "transparency": ((1.0 - info.opacity) * 100.0).round().to_string(),
        "width": measure(r.width(), u),
        "height": measure(r.height(), u),
        "x": measure(r.x0, u),
        "y": measure(r.y0, u),
        "rotation": rotation.to_string(),
        "wrap": info.wrap,
        "valign": text.as_ref().map_or("top".to_string(), |o| serde_json::to_value(o.vertical_justification).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "top".to_string())),
        "insetTop": measure(inset[0], u),
        "insetLeft": measure(inset[1], u),
        "insetBottom": measure(inset[2], u),
        "insetRight": measure(inset[3], u),
        "columns": text.as_ref().map_or("1".to_string(), |o| o.columns.to_string()),
        "gutter": measure(text.as_ref().map_or(12.0, |o| o.gutter), u),
        "alt": alt,
    });
    open(app, FORMAT, with_init(fields));
}

fn format_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let mut names = vec![("colors", "Colors and Lines"), ("size", "Size"), ("layout", "Layout")];
    if d.b("isText") {
        names.push(("box", "Text Box"));
    }
    names.push(("alt", "Alt Text"));
    tabs(ui, d, &names);
    match d.s("tab").as_str() {
        "size" => {
            egui::Grid::new("pr_format_size").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                labelled(ui, "Height:", |ui| text(ui, d, "height", 80.0));
                labelled(ui, "Width:", |ui| text(ui, d, "width", 80.0));
                labelled(ui, "Rotation:", |ui| text(ui, d, "rotation", 80.0));
            });
        }
        "layout" => {
            egui::Grid::new("pr_format_layout").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                labelled(ui, "Horizontal position:", |ui| text(ui, d, "x", 80.0));
                labelled(ui, "Vertical position:", |ui| text(ui, d, "y", 80.0));
                labelled(ui, "Wrapping style:", |ui| {
                    combo(
                        ui,
                        d,
                        "wrap",
                        &[("none", "None"), ("boundingBox", "Square"), ("contour", "Tight"), ("jumpObject", "Top and Bottom")],
                        150.0,
                    );
                });
            });
        }
        "box" => {
            egui::Grid::new("pr_format_box").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                labelled(ui, "Vertical alignment:", |ui| {
                    combo(ui, d, "valign", &[("top", "Top"), ("center", "Middle"), ("bottom", "Bottom")], 110.0);
                });
                labelled(ui, "Left margin:", |ui| text(ui, d, "insetLeft", 70.0));
                labelled(ui, "Right margin:", |ui| text(ui, d, "insetRight", 70.0));
                labelled(ui, "Top margin:", |ui| text(ui, d, "insetTop", 70.0));
                labelled(ui, "Bottom margin:", |ui| text(ui, d, "insetBottom", 70.0));
                labelled(ui, "Columns:", |ui| text(ui, d, "columns", 70.0));
                labelled(ui, "Spacing:", |ui| text(ui, d, "gutter", 70.0));
            });
        }
        "alt" => {
            ui.label("Alternative text (for people who can't see the picture):");
            let mut s = d.s("alt");
            if ui.add(egui::TextEdit::multiline(&mut s).desired_width(460.0).desired_rows(4)).changed() {
                d.fields.insert("alt".into(), json!(s));
            }
        }
        _ => {
            egui::Grid::new("pr_format_colors").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Fill color:");
                let cur = d.s("fill");
                if let Some(c) = colors::picker(app, ui, "pr_format_fill", Target::Fill, Some("No Fill"), &cur) {
                    d.fields.insert("fill".into(), json!(c));
                }
                ui.end_row();
                labelled(ui, "Transparency (%):", |ui| text(ui, d, "transparency", 60.0));
                ui.label("Line color:");
                let cur = d.s("line");
                if let Some(c) = colors::picker(app, ui, "pr_format_line", Target::Line, Some("No Line"), &cur) {
                    d.fields.insert("line".into(), json!(c));
                }
                ui.end_row();
                labelled(ui, "Line weight (pt):", |ui| text(ui, d, "weight", 60.0));
                labelled(ui, "Line style:", |ui| {
                    combo(ui, d, "dash", &[("solid", "Solid"), ("dashed", "Dashed"), ("dotted", "Dotted")], 110.0);
                });
            });
        }
    }
}

fn format_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let u = unit(app);
    let mut steps: Vec<Value> = Vec::new();
    let mut step = |command: &str, params: Value| steps.push(json!({"command": command, "params": params}));
    if changed(d, "fill") {
        step("object.fill", json!({"swatch": d.s("fill")}));
    }
    if changed(d, "transparency") {
        let t = d.n("transparency").filter(|v| (0.0..=100.0).contains(v)).ok_or("Transparency: type a percentage from 0 to 100.")?;
        step("object.opacity", json!({"opacity": 1.0 - t / 100.0}));
    }
    let mut line = Map::new();
    if changed(d, "line") {
        line.insert("swatch".into(), json!(d.s("line")));
    }
    if changed(d, "weight") {
        line.insert("weight".into(), json!(d.n("weight").filter(|v| *v >= 0.0 && *v <= 1000.0).ok_or("Line weight: type a number.")?));
    }
    if changed(d, "dash") {
        line.insert(
            "type".into(),
            match d.s("dash").as_str() {
                "dashed" => json!({"kind": "dashed", "pattern": [4.0, 3.0]}),
                "dotted" => json!({"kind": "dotted"}),
                _ => json!({"kind": "solid"}),
            },
        );
    }
    if !line.is_empty() {
        step("object.stroke", Value::Object(line));
    }
    let mut xf = Map::new();
    for (key, attr, label) in
        [("width", "width", "Width"), ("height", "height", "Height"), ("x", "x", "Horizontal position"), ("y", "y", "Vertical position")]
    {
        if changed(d, key) {
            xf.insert(attr.into(), json!(read(d, key, label, u)?));
        }
    }
    if changed(d, "rotation") {
        xf.insert("rotation".into(), json!(d.n("rotation").filter(|v| v.is_finite()).ok_or("Rotation: type an angle in degrees.")?));
    }
    if !xf.is_empty() {
        xf.insert("ref".into(), json!(0));
        step("transform.set", Value::Object(xf));
    }
    if changed(d, "wrap") {
        step("object.textWrap", json!({"mode": d.s("wrap")}));
    }
    let mut frame = Map::new();
    if changed(d, "valign") {
        frame.insert("verticalJustification".into(), json!(d.s("valign")));
    }
    if ["insetTop", "insetLeft", "insetBottom", "insetRight"].iter().any(|k| changed(d, k)) {
        frame.insert(
            "inset".into(),
            json!([
                read(d, "insetTop", "Top margin", u)?,
                read(d, "insetLeft", "Left margin", u)?,
                read(d, "insetBottom", "Bottom margin", u)?,
                read(d, "insetRight", "Right margin", u)?
            ]),
        );
    }
    if changed(d, "columns") {
        frame.insert(
            "columns".into(),
            json!(d.n("columns").filter(|v| *v >= 1.0 && *v <= 40.0).ok_or("Columns: type a number from 1 to 40.")? as u64),
        );
    }
    if changed(d, "gutter") {
        frame.insert("gutter".into(), json!(read(d, "gutter", "Spacing", u)?));
    }
    if !frame.is_empty() {
        step("object.textFrameOptions", Value::Object(frame));
    }
    if changed(d, "alt") {
        step("object.altText", json!({"text": d.s("alt")}));
    }
    if steps.is_empty() {
        return Ok(Value::Null);
    }
    app.run("script.run", json!({"text": Value::Array(steps).to_string()}))
}

// ───────────────────────────── Date and Time ─────────────────────────────

pub(crate) fn open_date(app: &mut DesignApp) {
    open(app, DATE, json!({"choice": 0}));
}

fn date_body(ui: &mut Ui, d: &mut Dialog) {
    ui.label("Available formats:");
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(350.0);
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            let cur = d.n("choice").unwrap_or(0.0) as usize;
            for (i, s) in super::date_formats().iter().enumerate() {
                if ui.selectable_label(cur == i, s).clicked() {
                    d.fields.insert("choice".into(), json!(i));
                }
            }
        });
    });
}

fn date_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let list = super::date_formats();
    let text = list.get(d.n("choice").unwrap_or(0.0) as usize).cloned().unwrap_or_default();
    super::insert_text_or_box(app, &text);
    Ok(Value::Null)
}

// ───────────────────────────── Page Number Format ─────────────────────────────

pub(crate) fn open_number_format(app: &mut DesignApp) {
    let page = crate::canvas::current_page(app).unwrap_or(0);
    open(app, NUMBER, json!({"page": page + 1, "style": "arabic", "start": "1", "restart": false}));
}

fn number_body(ui: &mut Ui, d: &mut Dialog) {
    egui::Grid::new("pr_number").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        labelled(ui, "Number format:", |ui| {
            combo(
                ui,
                d,
                "style",
                &[
                    ("arabic", "1, 2, 3"),
                    ("lowerRoman", "i, ii, iii"),
                    ("upperRoman", "I, II, III"),
                    ("lowerLetters", "a, b, c"),
                    ("upperLetters", "A, B, C"),
                ],
                130.0,
            );
        });
    });
    ui.add_space(6.0);
    check(ui, d, "restart", "Start this section at:");
    ui.add_enabled_ui(d.b("restart"), |ui| text(ui, d, "start", 60.0));
}

fn number_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let mut p = json!({"page": d.n("page").unwrap_or(1.0) as u64, "style": d.s("style")});
    if d.b("restart") {
        p["startNumber"] = json!(d.n("start").filter(|v| *v >= 1.0).ok_or("Start this section at: type a page number.")? as u64);
    }
    app.run("layout.section", p)
}

// ───────────────────────────── Options ─────────────────────────────

const UNITS: &[(&str, &str)] = &[("inches", "Inches"), ("centimeters", "Centimeters"), ("picas", "Picas"), ("points", "Points")];

pub(crate) fn open_options(app: &mut DesignApp) {
    let u =
        app.session.active().and_then(|st| serde_json::to_value(st.doc.settings.horizontal_units).ok()).and_then(|v| v.as_str().map(str::to_string));
    let fields =
        json!({"units": u.unwrap_or_else(|| "inches".to_string()), "spelling": app.ui.dynamic_spelling, "hasDoc": app.session.active().is_some()});
    open(app, OPTIONS, with_init(fields));
}

fn options_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    if d.b("hasDoc") {
        ui.horizontal(|ui| {
            ui.label("Show measurements in units of:");
            combo(ui, d, "units", UNITS, 120.0);
        });
        ui.add_space(8.0);
    }
    check(ui, d, "spelling", "Check spelling as you type");
    ui.add_space(10.0);
    if ui.button("More Settings…").clicked() {
        let _ = app.run("app.preferences", json!({}));
    }
}

fn options_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    app.ui.dynamic_spelling = d.b("spelling");
    if changed(d, "units") && d.b("hasDoc") {
        return app.run("document.preferences", json!({"horizontalUnits": d.s("units"), "verticalUnits": d.s("units")}));
    }
    Ok(Value::Null)
}

// ───────────────────────────── Master pages ─────────────────────────────

pub(crate) fn open_apply_master(app: &mut DesignApp) {
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    open(app, APPLY_MASTER, json!({"master": null, "scope": "this", "page": cur}));
}

fn apply_master_body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let masters: Vec<(u64, String)> = app
        .session
        .active()
        .map(|st| {
            st.doc
                .parents
                .iter()
                .map(|p| {
                    let name = p.parent.as_ref().map_or("Master Page".to_string(), |i| format!("{}-{}", i.prefix, i.name));
                    (p.id.0, name)
                })
                .collect()
        })
        .unwrap_or_default();
    let cur = d.fields.get("master").and_then(Value::as_u64);
    let shown = cur.and_then(|c| masters.iter().find(|m| m.0 == c)).map_or("(None)".to_string(), |m| m.1.clone());
    ui.horizontal(|ui| {
        ui.label("Master page:");
        craft_ui::ComboBox::from_id_salt("pr_master").width(220.0).selected_text(shown).show_ui(ui, |ui| {
            if ui.selectable_label(cur.is_none(), "(None)").clicked() {
                d.fields.insert("master".into(), Value::Null);
            }
            for (id, name) in &masters {
                if ui.selectable_label(cur == Some(*id), name).clicked() {
                    d.fields.insert("master".into(), json!(id));
                }
            }
        });
    });
    ui.add_space(6.0);
    for (k, label) in [("this", "This page"), ("all", "All pages")] {
        if ui.radio(d.s("scope") == k, label).clicked() {
            d.fields.insert("scope".into(), json!(k));
        }
    }
}

fn apply_master_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let n = app.session.active().map_or(0, |st| st.doc.page_count());
    let pages: Vec<usize> = if d.s("scope") == "all" { (0..n).collect() } else { vec![d.n("page").unwrap_or(0.0) as usize] };
    app.run("layout.pages.applyParent", json!({"pages": pages, "parent": d.fields.get("master").cloned().unwrap_or(Value::Null)}))
}

pub(crate) fn open_new_master(app: &mut DesignApp) {
    open(app, NEW_MASTER, json!({"name": "Master Page"}));
}

fn new_master_body(ui: &mut Ui, d: &mut Dialog) {
    ui.horizontal(|ui| {
        ui.label("Description:");
        text(ui, d, "name", 240.0);
    });
}

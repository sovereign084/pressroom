//! The ribbon: tab strip and the groups of each tab, laid out the way a Publisher user expects to
//! find them (same tabs, groups, button names and order). Every button acts through the engine's
//! commands (`menus::activate`), so it does exactly what the menu item of the same command does.

use egui::{Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use super::{Context, PR, Tab, art};
use crate::DesignApp;

/// Height of the ribbon body: the groups and their labels on a rounded panel, with its margins.
pub const BODY_H: f32 = 100.0;
/// Height of a group's controls; the label sits below.
const CONTENT_H: f32 = 70.0;
const LABEL_H: f32 = 18.0;
/// One row of small buttons (three to a column).
const ROW_H: f32 = 22.0;

/// Font sizes offered in the Font Size box, as Publisher lists them.
const SIZES: &[f64] = &[8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0];

/// Run a command as its menu item would (dialogs for "…" commands, errors to the status line).
pub(super) fn act(app: &mut DesignApp, id: &str) {
    crate::menus::activate(app, id, &Value::Null);
}

pub(super) fn act_with(app: &mut DesignApp, id: &str, params: Value) {
    crate::menus::activate(app, id, &params);
}

/// Whether the command can run now (greys its button out otherwise).
pub(super) fn on(app: &DesignApp, id: &str) -> bool {
    crate::menus::menu_enabled(app, id)
}

fn checked(app: &DesignApp, id: &str) -> bool {
    crate::menus::checked(app, id, &Value::Null).unwrap_or(false)
}

fn has_doc(app: &DesignApp) -> bool {
    app.session.active().is_some()
}

fn has_selection(app: &DesignApp) -> bool {
    app.session.active().is_some_and(|d| !d.selection.items.is_empty() || d.selection.text.is_some())
}

fn text_w(ui: &Ui, s: &str, font: &FontId) -> f32 {
    ui.painter().layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x
}

fn label_font() -> FontId {
    FontId::proportional(11.5)
}

fn paint_button_bg(ui: &Ui, r: Rect, resp: &Response, on: bool) {
    let fill = if resp.is_pointer_button_down_on() {
        PR::PRESSED
    } else if on {
        PR::CHECKED
    } else if resp.hovered() {
        PR::HOVER
    } else {
        return;
    };
    // Soft, rounded highlights, as current Office draws them; a button that's on keeps a hairline.
    let edge = if on { Stroke::new(1.0, PR::HOVER_BORDER) } else { Stroke::NONE };
    ui.painter().rect(r.shrink(0.5), PR::ROUND, fill, edge, StrokeKind::Inside);
}

fn text_color(enabled: bool) -> Color32 {
    if enabled { PR::TEXT } else { PR::DISABLED }
}

/// A large button: 32 px icon over a one- or two-line label (`\n` breaks it), with ▾ when `arrow`.
fn big_inner(ui: &mut Ui, icon: &str, label: &str, enabled: bool, arrow: bool, on: bool) -> Response {
    let font = label_font();
    let mut lines: Vec<String> = label.split('\n').map(str::to_string).collect();
    if arrow {
        if lines.len() == 1 {
            lines.push("▾".into());
        } else if let Some(last) = lines.last_mut() {
            last.push_str(" ▾");
        }
    }
    let w = lines.iter().map(|l| text_w(ui, l, &font)).fold(0.0, f32::max).max(34.0) + 10.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, CONTENT_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    art::paint(ui.painter(), Rect::from_min_size(pos2(r.center().x - 16.0, r.min.y + 4.0), vec2(32.0, 32.0)), icon, enabled);
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(pos2(r.center().x, r.min.y + 46.0 + i as f32 * 13.0), egui::Align2::CENTER_CENTER, l, font.clone(), text_color(enabled));
    }
    let tip = label.replace('\n', " ");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &tip));
    resp.on_hover_text(tip)
}

pub(super) fn big(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> Response {
    big_inner(ui, icon, label, enabled, false, false)
}

/// A large button that drops a menu.
fn big_menu(ui: &mut Ui, icon: &str, label: &str, enabled: bool, contents: impl FnOnce(&mut Ui)) {
    let resp = big_inner(ui, icon, label, enabled, true, false);
    if enabled {
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(170.0);
            contents(ui);
        });
    }
}

/// A small button: 16 px icon and a label, one row high.
fn small_inner(ui: &mut Ui, icon: &str, label: &str, enabled: bool, arrow: bool, on: bool) -> Response {
    let font = label_font();
    let shown = if arrow { format!("{label} ▾") } else { label.to_string() };
    let w = if shown.is_empty() { 24.0 } else { text_w(ui, &shown, &font) + 30.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    art::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 4.0, r.center().y - 8.0), vec2(16.0, 16.0)), icon, enabled);
    if !shown.is_empty() {
        ui.painter().text(pos2(r.min.x + 25.0, r.center().y), egui::Align2::LEFT_CENTER, &shown, font, text_color(enabled));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    resp.on_hover_text(label)
}

fn small(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> Response {
    small_inner(ui, icon, label, enabled, false, false)
}

fn small_menu(ui: &mut Ui, icon: &str, label: &str, enabled: bool, contents: impl FnOnce(&mut Ui)) {
    let resp = small_inner(ui, icon, label, enabled, true, false);
    if enabled {
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(160.0);
            contents(ui);
        });
    }
}

/// An icon-only toggle (alignment, bullets…) with a tooltip.
fn tool(ui: &mut Ui, icon: &str, tip: &str, enabled: bool, on: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, enabled);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, tip));
    resp.on_hover_text(tip)
}

/// A letter toggle: Bold, Italic, Underline.
fn letter(ui: &mut Ui, text: &str, tip: &str, enabled: bool, on: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    let c = text_color(enabled);
    let p = ui.painter();
    match text {
        "B" => {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "B", crate::theme::semibold(14.0), c);
        }
        "I" => {
            // An upright font slanted by hand: two strokes of a serif I.
            let cx = r.center().x;
            let (top, bot) = (r.center().y - 6.0, r.center().y + 6.0);
            p.line_segment([pos2(cx + 2.5, top), pos2(cx - 2.5, bot)], Stroke::new(1.6, c));
            p.line_segment([pos2(cx, top), pos2(cx + 5.0, top)], Stroke::new(1.2, c));
            p.line_segment([pos2(cx - 5.0, bot), pos2(cx, bot)], Stroke::new(1.2, c));
        }
        _ => {
            p.text(r.center() - vec2(0.0, 1.0), egui::Align2::CENTER_CENTER, "U", FontId::proportional(14.0), c);
            p.line_segment([r.center() + vec2(-5.0, 7.0), r.center() + vec2(5.0, 7.0)], Stroke::new(1.2, c));
        }
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, tip));
    resp.on_hover_text(tip)
}

/// A group: its controls, its name below, an optional dialog launcher at the bottom right, and the
/// separator after it. Returns whether the launcher was clicked.
fn group(ui: &mut Ui, name: &str, launcher: bool, add: impl FnOnce(&mut Ui)) -> bool {
    let top = ui.cursor().min;
    let inner = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
        let content = ui.horizontal_top(|ui| {
            ui.set_height(CONTENT_H);
            ui.add_space(3.0);
            add(ui);
            ui.add_space(3.0);
        });
        let w = content.response.rect.width().max(text_w(ui, name, &label_font()) + if launcher { 26.0 } else { 12.0 });
        let (lr, _) = ui.allocate_exact_size(vec2(w, LABEL_H), Sense::hover());
        ui.painter().text(
            pos2(lr.center().x - if launcher { 6.0 } else { 0.0 }, lr.center().y),
            egui::Align2::CENTER_CENTER,
            name,
            label_font(),
            PR::GROUP_LABEL,
        );
        let mut clicked = false;
        if launcher {
            let br = Rect::from_min_size(pos2(lr.max.x - 15.0, lr.min.y + 3.0), vec2(12.0, 12.0));
            let resp = ui.interact(br, ui.scope_id().with(("launcher", name)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(br, PR::ROUND, PR::HOVER);
            }
            let s = Stroke::new(1.0, PR::GROUP_LABEL);
            ui.painter().line_segment([br.min + vec2(3.0, 3.0), br.max - vec2(3.0, 3.0)], s);
            ui.painter().line_segment([pos2(br.min.x + 5.0, br.max.y - 3.0), br.max - vec2(3.0, 3.0)], s);
            ui.painter().line_segment([pos2(br.max.x - 3.0, br.min.y + 5.0), br.max - vec2(3.0, 3.0)], s);
            clicked = resp.on_hover_text(format!("{name} Settings")).clicked();
        }
        clicked
    });
    let right = inner.response.rect.max.x + 3.0;
    ui.add_space(6.0);
    ui.painter().line_segment([pos2(right, top.y + 8.0), pos2(right, top.y + CONTENT_H + LABEL_H - 6.0)], Stroke::new(1.0, PR::GROUP_SEP));
    inner.inner
}

/// A column of up to three small rows.
fn column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        ui.add_space(1.0);
        add(ui);
    });
}

fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
        ui.set_height(ROW_H);
        add(ui);
    });
}

fn item(ui: &mut Ui, label: &str) -> bool {
    let clicked = ui.button(label).clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// Lay out the body of the current tab.
pub(super) fn body(app: &mut DesignApp, ui: &mut Ui, tab: Tab, context: Context) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(4.0);
        match tab {
            Tab::Home => home(app, ui),
            Tab::Insert => insert(app, ui),
            Tab::PageDesign => page_design(app, ui),
            Tab::Mailings => mailings(app, ui),
            Tab::Review => review(app, ui),
            Tab::View => view(app, ui),
            Tab::Help => help(app, ui),
            Tab::Format => match context {
                Context::Picture => picture_format(app, ui),
                Context::Drawing => drawing_format(app, ui),
                Context::Table => table_layout(app, ui),
                _ => text_box_format(app, ui),
            },
            Tab::File => {}
        }
    });
}

// ───────────────────────────── Help ─────────────────────────────

/// Help, Feedback, What's New and Check for Updates, as Publisher's Help tab offers them.
fn help(app: &mut DesignApp, ui: &mut Ui) {
    let repo = super::update::REPO;
    group(ui, "Help", false, |ui| {
        if big(ui, "help", "Help", true).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://github.com/{repo}#readme")));
        }
        if big(ui, "feedback", "Feedback", true).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://github.com/{repo}/issues/new")));
        }
        if big(ui, "whats-new", "What's\nNew", true).clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://github.com/{repo}/releases")));
        }
        if big(ui, "update", "Check for\nUpdates", true).clicked() {
            app.updates_rt.pressroom.check(ui.ctx(), true);
        }
    });
}

// ───────────────────────────── Home ─────────────────────────────

fn clipboard_group(app: &mut DesignApp, ui: &mut Ui) {
    group(ui, "Clipboard", false, |ui| {
        let paste_on = on(app, "edit.paste");
        let resp = big_inner(ui, "paste", "Paste", paste_on, true, false);
        // The icon pastes; the arrow (lower part) opens the paste options.
        if resp.clicked() && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| p.y < resp.rect.min.y + 40.0) {
            act(app, "edit.paste");
        } else if paste_on {
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_min_width(180.0);
                if item(ui, "Paste") {
                    act(app, "edit.paste");
                }
                if item(ui, "Keep Text Only") {
                    act(app, "edit.pasteWithoutFormatting");
                }
                if item(ui, "Paste in Place") {
                    act(app, "edit.pasteInPlace");
                }
            });
        }
        column(ui, |ui| {
            if small(ui, "cut", "Cut", on(app, "edit.cut")).clicked() {
                act(app, "edit.cut");
            }
            if small(ui, "copy", "Copy", on(app, "edit.copy")).clicked() {
                act(app, "edit.copy");
            }
            if small(ui, "format-painter", "Format Painter", on(app, "type.pickUpFormat")).clicked() {
                let _ = app.run("tool.select", json!({"tool": "eyedropper"}));
            }
        });
    });
}

/// Character attributes of the text being edited, or of the selected text frames.
fn char_attrs(app: &mut DesignApp) -> Option<Value> {
    crate::panels::text_attrs(app).map(|a| a["chars"].clone())
}

fn font_group(app: &mut DesignApp, ui: &mut Ui) {
    let chars = char_attrs(app);
    let text = chars.is_some();
    let ch = chars.unwrap_or(Value::Null);
    let style = ch["fontStyle"].as_str().unwrap_or("").to_lowercase();
    let bold = style.contains("bold") || style.contains("black") || style.contains("heavy");
    let italic = style.contains("italic") || style.contains("oblique");
    let underline = ch["underline"].as_bool().unwrap_or(false) || ch["underline"].is_object();
    if group(ui, "Font", true, |ui| {
        column(ui, |ui| {
            row(ui, |ui| {
                ui.add_enabled_ui(text, |ui| {
                    let fam = ch["fontFamily"].as_str().unwrap_or("").to_string();
                    crate::panels::font_family_picker(app, ui, &fam, 150.0);
                    ui.add_space(2.0);
                    if let Some(v) = crate::widgets::NumField::number("pr_font_size", ch["size"].as_f64(), "", 1)
                        .width(46.0)
                        .presets(SIZES)
                        .range(1.0, 1296.0)
                        .show(ui)
                    {
                        let _ = app.run("type.char", json!({"attrs": {"size": v}}));
                    }
                });
                ui.add_space(2.0);
                if tool(ui, "grow-font", "Grow Font (Ctrl+Shift+>)", text, false).clicked() {
                    act(app, "type.sizeUp");
                }
                if tool(ui, "shrink-font", "Shrink Font (Ctrl+Shift+<)", text, false).clicked() {
                    act(app, "type.sizeDown");
                }
                if tool(ui, "clear-format", "Clear All Formatting", text, false).clicked() {
                    // Back to the paragraph style's own formatting.
                    let _ = app.run("style.character.apply", json!({"name": "[None]"}));
                }
            });
            ui.add_space(4.0);
            row(ui, |ui| {
                if letter(ui, "B", "Bold (Ctrl+B)", text, bold).clicked() {
                    act(app, "type.bold");
                }
                if letter(ui, "I", "Italic (Ctrl+I)", text, italic).clicked() {
                    act(app, "type.italic");
                }
                if letter(ui, "U", "Underline (Ctrl+U)", text, underline).clicked() {
                    act(app, "type.underline");
                }
                if tool(ui, "subscript", "Subscript (Ctrl+=)", text, ch["position"].as_str() == Some("subscript")).clicked() {
                    let pos = if ch["position"].as_str() == Some("subscript") { "normal" } else { "subscript" };
                    let _ = app.run("type.char", json!({"attrs": {"position": pos}}));
                }
                if tool(ui, "superscript", "Superscript (Ctrl+Shift+=)", text, ch["position"].as_str() == Some("superscript")).clicked() {
                    let pos = if ch["position"].as_str() == Some("superscript") { "normal" } else { "superscript" };
                    let _ = app.run("type.char", json!({"attrs": {"position": pos}}));
                }
                let resp = tool(ui, "change-case", "Change Case", text, false);
                egui::Popup::menu(&resp).show(|ui| {
                    for (label, case) in
                        [("Sentence case.", "sentence"), ("lowercase", "lower"), ("UPPERCASE", "upper"), ("Capitalize Each Word", "title")]
                    {
                        if item(ui, label) {
                            act_with(app, "type.changeCase", json!({"case": case}));
                        }
                    }
                });
                let resp = tool(ui, "char-spacing", "Character Spacing", text, false);
                egui::Popup::menu(&resp).show(|ui| {
                    for (label, v) in [("Very Tight", -100.0), ("Tight", -50.0), ("Normal", 0.0), ("Loose", 50.0), ("Very Loose", 100.0)] {
                        if item(ui, label) {
                            let _ = app.run("type.char", json!({"attrs": {"tracking": v}}));
                        }
                    }
                    ui.separator();
                    if item(ui, "More Spacing…") {
                        act(app, "app.characterPanel");
                    }
                });
                let resp = tool(ui, "font-color", "Font Color", text, false);
                egui::Popup::menu(&resp).show(|ui| {
                    if let Some(doc) = app.session.active().map(|d| d.doc.clone()) {
                        let cur = ch["fill"].as_str().unwrap_or("").to_string();
                        if let Some(name) = crate::panels::swatch_list(ui, &doc, &cur) {
                            let _ = app.run("type.char", json!({"attrs": {"fill": name}}));
                            ui.close();
                        }
                    }
                });
            });
        });
    }) {
        act(app, "app.characterPanel");
    }
}

fn paragraph_group(app: &mut DesignApp, ui: &mut Ui) {
    let text = crate::panels::text_attrs(app);
    let en = text.is_some();
    let para = text.map(|a| a["para"].clone()).unwrap_or(Value::Null);
    let align = para["align"].as_str().unwrap_or("").to_string();
    if group(ui, "Paragraph", true, |ui| {
        column(ui, |ui| {
            row(ui, |ui| {
                let resp = tool(ui, "bullets", "Bullets", en, para["listType"].as_str() == Some("bullets"));
                egui::Popup::menu(&resp).show(|ui| {
                    if item(ui, "None") {
                        let _ = app.run("list.bullets", json!({"on": false}));
                    }
                    for b in ["•", "◦", "■", "◆", "✓", "➢"] {
                        if item(ui, &format!("{b}   Bullet")) {
                            let _ = app.run("list.bullets", json!({"on": true}));
                            let _ = app.run("type.para", json!({"attrs": {"bulletChar": b}}));
                        }
                    }
                    ui.separator();
                    if item(ui, "Bullets and Numbering…") {
                        act(app, "app.bulletsNumberingDialog");
                    }
                });
                let resp = tool(ui, "numbering", "Numbering", en, para["listType"].as_str() == Some("numbers"));
                egui::Popup::menu(&resp).show(|ui| {
                    if item(ui, "None") {
                        let _ = app.run("list.numbers", json!({"on": false}));
                    }
                    for (label, style) in
                        [("1. 2. 3.", "arabic"), ("a. b. c.", "lowerLetters"), ("A. B. C.", "upperLetters"), ("i. ii. iii.", "lowerRoman")]
                    {
                        if item(ui, label) {
                            let _ = app.run("list.numbers", json!({"on": true}));
                            let _ = app.run("type.para", json!({"attrs": {"numberStyle": style, "numberExpression": "^#.^t"}}));
                        }
                    }
                    ui.separator();
                    if item(ui, "Bullets and Numbering…") {
                        act(app, "app.bulletsNumberingDialog");
                    }
                });
                let left = para["leftIndent"].as_f64().unwrap_or(0.0);
                if tool(ui, "indent-less", "Decrease Indent Position", en && left > 0.0, false).clicked() {
                    let _ = app.run("type.para", json!({"attrs": {"leftIndent": (left - 18.0).max(0.0)}}));
                }
                if tool(ui, "indent-more", "Increase Indent Position", en, false).clicked() {
                    let _ = app.run("type.para", json!({"attrs": {"leftIndent": left + 18.0}}));
                }
                if tool(ui, "pilcrow", "Special Characters (Ctrl+Shift+Y)", has_doc(app), checked(app, "view.hiddenCharacters")).clicked() {
                    act(app, "view.hiddenCharacters");
                }
            });
            ui.add_space(4.0);
            row(ui, |ui| {
                for (icon, tip, cmd, key) in [
                    ("align-left", "Align Text Left (Ctrl+L)", "type.alignLeft", "left"),
                    ("align-center", "Center (Ctrl+E)", "type.alignCenter", "center"),
                    ("align-right", "Align Text Right (Ctrl+R)", "type.alignRight", "right"),
                    ("justify", "Justify (Ctrl+J)", "type.justify", "leftJustified"),
                    ("distribute", "Distribute (Ctrl+Shift+D)", "", "fullyJustified"),
                ] {
                    if tool(ui, icon, tip, en, align == key).clicked() {
                        if cmd.is_empty() {
                            let _ = app.run("type.para", json!({"attrs": {"align": key}}));
                        } else {
                            act(app, cmd);
                        }
                    }
                }
                let resp = tool(ui, "line-spacing", "Line Spacing", en, false);
                egui::Popup::menu(&resp).show(|ui| {
                    for (label, v) in [("1.0", 1.0), ("1.15", 1.15), ("1.5", 1.5), ("2.0", 2.0), ("2.5", 2.5), ("3.0", 3.0)] {
                        if item(ui, label) {
                            // Line spacing in lines → leading as a multiple of the size.
                            let size = ch_size(app);
                            let _ = app.run("type.char", json!({"attrs": {"leading": {"kind": "points", "value": size * 1.2 * v}}}));
                        }
                    }
                    ui.separator();
                    if item(ui, "Line Spacing Options…") {
                        act(app, "app.paragraphPanel");
                    }
                });
            });
        });
    }) {
        act(app, "app.paragraphPanel");
    }
}

fn ch_size(app: &mut DesignApp) -> f64 {
    char_attrs(app).and_then(|c| c["size"].as_f64()).unwrap_or(12.0)
}

fn styles_group(app: &mut DesignApp, ui: &mut Ui) {
    let en = crate::panels::text_attrs(app).is_some();
    group(ui, "Styles", false, |ui| {
        big_menu(ui, "styles", "Styles", en, |ui| {
            let names: Vec<String> = app
                .session
                .active()
                .map(|d| d.doc.styles.paragraph.iter().map(|s| s.name.clone()).filter(|n| !n.starts_with('[')).collect())
                .unwrap_or_default();
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for n in names {
                    if item(ui, &n) {
                        let _ = app.run("style.paragraph.apply", json!({"name": n}));
                    }
                }
            });
            ui.separator();
            if item(ui, "New Style…") {
                act(app, "style.paragraph.create");
            }
        });
    });
}

fn objects_group(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Objects", false, |ui| {
        if big(ui, "textbox", "Draw Text\nBox", doc).clicked() {
            super::start_draw(app, "type");
        }
        // Then Pictures, Table and Shapes, in Publisher's order.
        if big(ui, "picture", "Pictures", doc).clicked() {
            super::insert_picture(app);
        }
        if big_inner(ui, "table", "Table", doc, true, false).clicked() {
            super::insert_table(app);
        }
        big_menu(ui, "shapes", "Shapes", doc, |ui| shapes_menu(app, ui));
    });
}

fn arrange_group(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    let many = app.session.active().is_some_and(|d| d.selection.items.len() > 1);
    group(ui, "Arrange", false, |ui| {
        big_menu(ui, "wrap", "Wrap\nText", sel, |ui| {
            for (label, mode) in [
                ("None", "none"),
                ("Square", "boundingBox"),
                ("Tight", "contour"),
                ("Top and Bottom", "jumpObject"),
                ("Jump to Next Column", "jumpToNextColumn"),
            ] {
                if item(ui, label) {
                    act_with(app, "object.textWrap", json!({"mode": mode}));
                }
            }
            ui.separator();
            if item(ui, "More Layout Options…") {
                act(app, "object.textWrap");
            }
        });
        column(ui, |ui| {
            small_menu(ui, "bring-forward", "Bring Forward", sel, |ui| {
                if item(ui, "Bring Forward") {
                    act(app, "object.bringForward");
                }
                if item(ui, "Bring to Front") {
                    act(app, "object.bringToFront");
                }
            });
            small_menu(ui, "send-backward", "Send Backward", sel, |ui| {
                if item(ui, "Send Backward") {
                    act(app, "object.sendBackward");
                }
                if item(ui, "Send to Back") {
                    act(app, "object.sendToBack");
                }
            });
            small_menu(ui, "align", "Align", sel, |ui| {
                for (label, edge) in [
                    ("Align Left", "left"),
                    ("Align Center", "hcenter"),
                    ("Align Right", "right"),
                    ("Align Top", "top"),
                    ("Align Middle", "vcenter"),
                    ("Align Bottom", "bottom"),
                ] {
                    if item(ui, label) {
                        let to = if many { "selection" } else { "margins" };
                        act_with(app, "object.align", json!({"edge": edge, "to": to}));
                    }
                }
                ui.separator();
                if item(ui, "Distribute Horizontally") {
                    act_with(app, "object.distribute", json!({"axis": "horizontal"}));
                }
                if item(ui, "Distribute Vertically") {
                    act_with(app, "object.distribute", json!({"axis": "vertical"}));
                }
            });
        });
        column(ui, |ui| {
            if small(ui, "group", "Group", many).clicked() {
                act(app, "object.group");
            }
            if small(ui, "ungroup", "Ungroup", on(app, "object.ungroup")).clicked() {
                act(app, "object.ungroup");
            }
            small_menu(ui, "rotate", "Rotate", sel, |ui| {
                if item(ui, "Rotate Right 90°") {
                    act_with(app, "transform.rotate", json!({"angle": -90.0}));
                }
                if item(ui, "Rotate Left 90°") {
                    act_with(app, "transform.rotate", json!({"angle": 90.0}));
                }
                if item(ui, "Flip Vertical") {
                    act_with(app, "transform.flip", json!({"axis": "vertical"}));
                }
                if item(ui, "Flip Horizontal") {
                    act_with(app, "transform.flip", json!({"axis": "horizontal"}));
                }
            });
        });
    });
}

fn editing_group(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Editing", false, |ui| {
        column(ui, |ui| {
            if small(ui, "find", "Find", doc).clicked() {
                act(app, "app.findChange");
            }
            if small(ui, "replace", "Replace", doc).clicked() {
                act(app, "app.findChange");
            }
            small_menu(ui, "select", "Select", doc, |ui| {
                if item(ui, "Select All Text in Text Box") {
                    act(app, "edit.selectAll");
                }
                if item(ui, "Select All") {
                    act(app, "edit.selectAll");
                }
                if item(ui, "Select Objects") {
                    let _ = app.run("tool.select", json!({"tool": "selection"}));
                }
            });
        });
    });
}

fn home(app: &mut DesignApp, ui: &mut Ui) {
    clipboard_group(app, ui);
    font_group(app, ui);
    paragraph_group(app, ui);
    styles_group(app, ui);
    objects_group(app, ui);
    arrange_group(app, ui);
    editing_group(app, ui);
}

// ───────────────────────────── Insert ─────────────────────────────

fn page_menu(app: &mut DesignApp, ui: &mut Ui) {
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    if item(ui, "Insert Blank Page") {
        act_with(app, "layout.pages.insert", json!({"count": 1, "after": cur}));
    }
    if item(ui, "Insert Duplicate Page") {
        super::duplicate_page(app, cur);
    }
    if item(ui, "Insert Page…") {
        act(app, "layout.pages.insert");
    }
}

fn insert(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Pages", false, |ui| {
        big_menu(ui, "page-insert", "Page", doc, |ui| page_menu(app, ui));
    });
    group(ui, "Tables", false, |ui| {
        if big_inner(ui, "table", "Table", doc, true, false).clicked() {
            super::insert_table(app);
        }
    });
    group(ui, "Illustrations", false, |ui| {
        if big(ui, "picture", "Pictures", doc).clicked() {
            super::insert_picture(app);
        }
        big(ui, "online-picture", "Online\nPictures", false).on_hover_text("Not available in PressRoom: use Pictures");
        big_menu(ui, "shapes", "Shapes", doc, |ui| shapes_menu(app, ui));
        if big(ui, "placeholder", "Picture\nPlaceholder", doc).clicked() {
            super::start_draw(app, "rectangleFrame");
        }
    });
    group(ui, "Building Blocks", false, |ui| {
        for (icon, label) in
            [("page-parts", "Page\nParts"), ("calendars", "Calendars"), ("borders", "Borders &\nAccents"), ("ads", "Advertise-\nments")]
        {
            big_inner(ui, icon, label, false, true, false).on_hover_text("Coming in a later PressRoom update");
        }
    });
    group(ui, "Text", false, |ui| {
        if big(ui, "textbox", "Draw Text\nBox", doc).clicked() {
            super::start_draw(app, "type");
        }
        if big(ui, "insert-file", "Insert\nFile", doc).clicked() {
            act(app, "app.placeDialog");
        }
        big_inner(ui, "wordart", "WordArt", false, true, false).on_hover_text("Coming in a later PressRoom update");
        column(ui, |ui| {
            small_menu(ui, "symbol", "Symbol", doc, |ui| {
                egui::Grid::new("pr_symbols").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                    for (i, s) in
                        ["©", "®", "™", "§", "¶", "†", "•", "…", "–", "—", "°", "±", "×", "÷", "½", "¼", "¾", "€", "£", "¢", "✝", "♥", "★", "✓"]
                            .into_iter()
                            .enumerate()
                    {
                        if ui.add_sized(vec2(26.0, 26.0), egui::Button::new(s)).clicked() {
                            let _ = app.run("text.insert", json!({"text": s}));
                            ui.close();
                        }
                        if i % 6 == 5 {
                            ui.end_row();
                        }
                    }
                });
                ui.separator();
                if item(ui, "More Symbols…") {
                    act(app, "app.glyphsPanel");
                }
            });
            if small(ui, "datetime", "Date & Time", doc).clicked() {
                super::insert_date(app);
            }
            small(ui, "insert-file", "Object", false).on_hover_text("Coming in a later PressRoom update");
        });
    });
    group(ui, "Links", false, |ui| {
        if big(ui, "link", "Link", doc && has_selection(app)).clicked() {
            let _ = app.run("window.panel.activate", json!({"panel": "hyperlinks"}));
        }
        if big(ui, "bookmark", "Bookmark", doc).clicked() {
            let _ = app.run("window.panel.activate", json!({"panel": "bookmarks"}));
        }
    });
    group(ui, "Header & Footer", false, |ui| {
        if big(ui, "header", "Header", doc).clicked() {
            super::edit_header_footer(app, true);
        }
        if big(ui, "footer", "Footer", doc).clicked() {
            super::edit_header_footer(app, false);
        }
        big_menu(ui, "page-number", "Page\nNumber", doc, |ui| {
            if item(ui, "Insert Page Number") {
                act(app, "app.insertCurrentPageNumber");
            }
            if item(ui, "Page Number Format…") {
                act(app, "layout.section");
            }
        });
    });
}

fn shapes_menu(app: &mut DesignApp, ui: &mut Ui) {
    for (label, tool) in [("Rectangle", "rectangle"), ("Oval", "ellipse"), ("Line", "line"), ("Polygon", "polygon")] {
        if item(ui, label) {
            super::start_draw(app, tool);
        }
    }
}

// ───────────────────────────── Page Design ─────────────────────────────

fn page_size(app: &DesignApp) -> Option<(f64, f64)> {
    let st = app.session.active()?;
    let n = crate::canvas::current_page(app).unwrap_or(0);
    let p = st.doc.page(n)?;
    Some((p.width, p.height))
}

/// Page sizes offered in Size, in inches as Publisher names them.
const PAGE_SIZES: &[(&str, f64, f64)] = &[
    ("Letter (8.5 x 11\")", 8.5, 11.0),
    ("Legal (8.5 x 14\")", 8.5, 14.0),
    ("Tabloid (11 x 17\")", 11.0, 17.0),
    ("Half Letter (5.5 x 8.5\")", 5.5, 8.5),
    ("Postcard (4 x 6\")", 4.0, 6.0),
    ("A4 (8.27 x 11.69\")", 8.27, 11.69),
    ("A5 (5.83 x 8.27\")", 5.83, 8.27),
];

fn page_design(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Template", false, |ui| {
        if big(ui, "template", "Change\nTemplate", true).clicked() {
            super::open_backstage(ui.ctx(), super::Backstage::New);
        }
    });
    let size = page_size(app);
    if group(ui, "Page Setup", true, |ui| {
        big_menu(ui, "margins", "Margins", doc, |ui| {
            for (label, inch) in [("Narrow  (0.25\")", 0.25), ("Moderate  (0.5\")", 0.5), ("Wide  (1\")", 1.0), ("None", 0.0)] {
                if item(ui, label) {
                    act_with(app, "layout.marginsAndColumns", json!({"margins": inch * 72.0}));
                }
            }
            ui.separator();
            if item(ui, "Custom Margins…") {
                act(app, "layout.marginsAndColumns");
            }
        });
        big_menu(ui, "orientation", "Orientation", doc, |ui| {
            if let Some((w, h)) = size {
                if item(ui, "Portrait") && w > h {
                    act_with(app, "layout.documentSetup", json!({"width": h, "height": w}));
                }
                if item(ui, "Landscape") && h > w {
                    act_with(app, "layout.documentSetup", json!({"width": h, "height": w}));
                }
            }
        });
        big_menu(ui, "size", "Size", doc, |ui| {
            for (label, w, h) in PAGE_SIZES {
                let (w, h) = (w * 72.0, h * 72.0);
                let landscape = size.is_some_and(|(sw, sh)| sw > sh);
                let (w, h) = if landscape { (h, w) } else { (w, h) };
                if item(ui, label) {
                    act_with(app, "layout.documentSetup", json!({"width": w, "height": h}));
                }
            }
            ui.separator();
            if item(ui, "Page Setup…") {
                act(app, "layout.documentSetup");
            }
        });
    }) {
        act(app, "layout.documentSetup");
    }
    group(ui, "Layout", false, |ui| {
        big_menu(ui, "guides", "Guides", doc, |ui| {
            if item(ui, "No Guides") {
                act_with(app, "layout.marginsAndColumns", json!({"columns": 1}));
            }
            for n in 2..=4 {
                if item(ui, &format!("{n} Columns")) {
                    act_with(app, "layout.marginsAndColumns", json!({"columns": n}));
                }
            }
            ui.separator();
            if item(ui, "Grid and Baseline Guides…") {
                act(app, "layout.marginsAndColumns");
            }
            if item(ui, "Add Ruler Guides…") {
                act(app, "layout.createGuides");
            }
        });
        column(ui, |ui| {
            let mut snap = checked(app, "view.snapToGuides");
            ui.add_enabled_ui(doc, |ui| {
                ui.label(egui::RichText::new("Align To").color(PR::TEXT).font(label_font()));
                if ui.checkbox(&mut snap, egui::RichText::new("Guides").font(label_font())).changed() {
                    act(app, "view.snapToGuides");
                }
                let mut smart = checked(app, "view.smartGuides");
                if ui.checkbox(&mut smart, egui::RichText::new("Objects").font(label_font())).changed() {
                    act(app, "view.smartGuides");
                }
            });
        });
    });
    group(ui, "Pages", false, |ui| {
        let cur = crate::canvas::current_page(app).unwrap_or(0);
        let pages = app.session.active().map(|d| d.doc.page_count()).unwrap_or(0);
        if big(ui, "page-delete", "Delete", doc && pages > 1).clicked() {
            super::delete_page(app, cur);
        }
        if big(ui, "page-move", "Move", doc && pages > 1).clicked() {
            act(app, "layout.pages.move");
        }
        if big(ui, "rename", "Rename", doc).clicked() {
            act(app, "layout.section");
        }
        big_menu(ui, "master", "Master\nPages", doc, |ui| {
            if item(ui, "Edit Master Pages") {
                super::set_master_view(app, true);
            }
            if item(ui, "Apply Master Page…") {
                act(app, "layout.pages.applyParent");
            }
            if item(ui, "Add Master Page…") {
                act(app, "layout.parents.new");
            }
        });
    });
    group(ui, "Schemes", false, |ui| {
        big(ui, "schemes", "Color\nSchemes", false).on_hover_text("Coming in a later PressRoom update");
        big(ui, "fonts", "Fonts", false).on_hover_text("Coming in a later PressRoom update");
    });
    group(ui, "Page Background", false, |ui| {
        big_menu(ui, "background", "Background", doc, |ui| {
            if item(ui, "No Background") {
                super::set_background(app, None);
            }
            if let Some(d) = app.session.active().map(|d| d.doc.clone())
                && let Some(name) = crate::panels::swatch_list(ui, &d, "")
            {
                super::set_background(app, Some(name));
                ui.close();
            }
        });
    });
}

// ───────────────────────────── Mailings / Review ─────────────────────────────

fn mailings(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Start", false, |ui| {
        big_menu(ui, "mail-merge", "Mail\nMerge", doc, |ui| {
            if item(ui, "Step-by-Step Mail Merge Wizard…") {
                let _ = app.run("window.panel.activate", json!({"panel": "datamerge"}));
            }
        });
        big(ui, "mail-merge", "E-mail\nMerge", false).on_hover_text("Not available in PressRoom");
    });
}

fn review(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Proofing", false, |ui| {
        if big(ui, "spelling", "Spelling", doc).clicked() {
            act(app, "spelling.check");
        }
        big(ui, "research", "Research", false).on_hover_text("Not available in PressRoom");
        big(ui, "thesaurus", "Thesaurus", false).on_hover_text("Not available in PressRoom");
    });
    group(ui, "Language", false, |ui| {
        big(ui, "translate", "Translate", false).on_hover_text("Not available in PressRoom");
        big(ui, "language", "Language", doc).clicked().then(|| act(app, "app.characterPanel"));
    });
}

// ───────────────────────────── View ─────────────────────────────

fn view(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    let master = super::master_view(app);
    group(ui, "Views", false, |ui| {
        if big_inner(ui, "normal-view", "Normal", doc, false, doc && !master).clicked() {
            super::set_master_view(app, false);
        }
        if big_inner(ui, "master-view", "Master\nPage", doc, false, master).clicked() {
            super::set_master_view(app, true);
        }
    });
    let spread = super::two_page(ui.ctx());
    group(ui, "Layout", false, |ui| {
        if big_inner(ui, "single-page", "Single\nPage", doc, false, doc && !spread).clicked() {
            super::set_two_page(app, ui.ctx(), false);
        }
        if big_inner(ui, "two-page", "Two-Page\nSpread", doc, false, spread).clicked() {
            super::set_two_page(app, ui.ctx(), true);
        }
    });
    group(ui, "Show", false, |ui| {
        let nav = !super::nav_hidden(ui.ctx());
        column(ui, |ui| {
            for (label, id) in [("Boundaries", "view.frameEdges"), ("Guides", "view.guides"), ("Baselines", "view.baselineGrid")] {
                let mut v = checked(app, id);
                if ui.add_enabled(doc, egui::Checkbox::new(&mut v, egui::RichText::new(label).font(label_font()))).changed() {
                    act(app, id);
                }
            }
        });
        ui.add_space(8.0);
        column(ui, |ui| {
            let mut v = checked(app, "view.rulers");
            if ui.add_enabled(doc, egui::Checkbox::new(&mut v, egui::RichText::new("Rulers").font(label_font()))).changed() {
                act(app, "view.rulers");
            }
            let mut n = nav;
            if ui.add_enabled(doc, egui::Checkbox::new(&mut n, egui::RichText::new("Page Navigation").font(label_font()))).changed() {
                super::set_nav_hidden(ui.ctx(), !n);
            }
            let mut threads = checked(app, "view.textThreads");
            if ui.add_enabled(doc, egui::Checkbox::new(&mut threads, egui::RichText::new("Text Box Links").font(label_font()))).changed() {
                act(app, "view.textThreads");
            }
        });
    });
    group(ui, "Zoom", false, |ui| {
        big_menu(ui, "zoom", "Zoom", doc, |ui| {
            for p in [400.0, 200.0, 150.0, 100.0, 75.0, 66.0, 50.0, 33.0, 25.0, 10.0] {
                if item(ui, &format!("{p}%")) {
                    act_with(app, "view.zoom", json!({"zoom": p / 100.0}));
                }
            }
        });
        if big(ui, "zoom-100", "100%", doc).clicked() {
            act(app, "view.actualSize");
        }
        column(ui, |ui| {
            if small(ui, "whole-page", "Whole Page", doc).clicked() {
                act(app, "view.fitPage");
            }
            if small(ui, "page-width", "Page Width", doc).clicked() {
                super::fit_page_width(app);
            }
            if small(ui, "selected-objects", "Selected Objects", doc && has_selection(app)).clicked() {
                act(app, "view.fitSelection");
            }
        });
    });
    group(ui, "Window", false, |ui| {
        let docs: Vec<(usize, String)> = app.session.documents().iter().enumerate().map(|(i, d)| (i, d.title())).collect();
        let active = app.session.active_index();
        big_menu(ui, "switch-windows", "Switch\nWindows", !docs.is_empty(), |ui| {
            for (i, title) in docs {
                if ui.selectable_label(Some(i) == active, title).clicked() {
                    app.session.set_active(i);
                    ui.close();
                }
            }
        });
    });
}

// ───────────────────────────── Format (contextual) ─────────────────────────────

fn text_box_format(app: &mut DesignApp, ui: &mut Ui) {
    let frame = app.session.active().is_some_and(|d| !d.selection.items.is_empty() || d.selection.text.is_some());
    group(ui, "Text", false, |ui| {
        big_menu(ui, "text-fit", "Text\nFit", frame, |ui| {
            if item(ui, "Grow Text Box to Fit") {
                let ids = super::text_frame_ids(app);
                act_with(app, "object.fit", json!({"mode": "fitFrameToContent", "ids": ids}));
            }
            if item(ui, "Do Not Autofit") {
                let ids = super::text_frame_ids(app);
                act_with(app, "object.textFrameOptions", json!({"autoSize": "off", "ids": ids}));
            }
        });
        column(ui, |ui| {
            if small(ui, "columns", "Columns", frame).clicked() {
                act(app, "object.textFrameOptions");
            }
            if small(ui, "margins", "Margins", frame).clicked() {
                act(app, "object.textFrameOptions");
            }
        });
    });
    group(ui, "Linking", false, |ui| {
        if big(ui, "create-link", "Create\nLink", frame).clicked() {
            super::start_link(app);
        }
        if big(ui, "break-link", "Break", on(app, "text.unthread")).clicked() {
            act(app, "text.unthread");
        }
        column(ui, |ui| {
            if small(ui, "align-left", "Previous", frame).clicked() {
                super::follow_link(app, false);
            }
            if small(ui, "align-right", "Next", frame).clicked() {
                super::follow_link(app, true);
            }
        });
    });
    group(ui, "Alignment", false, |ui| {
        column(ui, |ui| {
            for v in ["top", "center", "bottom"] {
                row(ui, |ui| {
                    for (h, icon) in [("left", "align-left"), ("center", "align-center"), ("right", "align-right")] {
                        let tip = format!("Align {} {}", if v == "center" { "Middle" } else { v }, h);
                        if tool(ui, icon, &tip, frame, false).clicked() {
                            let ids = super::text_frame_ids(app);
                            let _ = app.run("object.textFrameOptions", json!({"verticalJustification": v, "ids": ids}));
                            let cmd = match h {
                                "left" => "type.alignLeft",
                                "center" => "type.alignCenter",
                                _ => "type.alignRight",
                            };
                            act(app, cmd);
                        }
                    }
                });
            }
        });
    });
    font_group(app, ui);
    group(ui, "Typography", false, |ui| {
        big_menu(ui, "drop-cap", "Drop\nCap", frame, |ui| {
            if item(ui, "No Drop Cap") {
                let _ = app.run("type.para", json!({"attrs": {"dropCapLines": 0, "dropCapChars": 0}}));
            }
            for n in [2, 3, 4] {
                if item(ui, &format!("Drop Cap, {n} lines")) {
                    let _ = app.run("type.para", json!({"attrs": {"dropCapLines": n, "dropCapChars": 1}}));
                }
            }
        });
    });
    shape_styles_group(app, ui);
    arrange_group(app, ui);
}

fn shape_styles_group(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    group(ui, "Shape Styles", false, |ui| {
        column(ui, |ui| {
            small_menu(ui, "shape-fill", "Shape Fill", sel, |ui| {
                if let Some(d) = app.session.active().map(|d| d.doc.clone())
                    && let Some(name) = crate::panels::swatch_list(ui, &d, "")
                {
                    act_with(app, "object.fill", json!({"swatch": name}));
                    ui.close();
                }
            });
            small_menu(ui, "shape-outline", "Shape Outline", sel, |ui| {
                ui.menu_button("Weight", |ui| {
                    for w in [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0] {
                        if item(ui, &format!("{w} pt")) {
                            act_with(app, "object.stroke", json!({"weight": w}));
                        }
                    }
                });
                ui.menu_button("Dashes", |ui| {
                    for (label, kind) in [("Solid", "solid"), ("Dotted", "dotted")] {
                        if item(ui, label) {
                            act_with(app, "object.stroke", json!({"type": {"kind": kind}}));
                        }
                    }
                    if item(ui, "Dashed") {
                        act_with(app, "object.stroke", json!({"type": {"kind": "dashed", "pattern": [4.0, 3.0]}}));
                    }
                });
                if item(ui, "No Outline") {
                    act_with(app, "object.stroke", json!({"swatch": "[None]"}));
                }
                ui.separator();
                if let Some(d) = app.session.active().map(|d| d.doc.clone())
                    && let Some(name) = crate::panels::swatch_list(ui, &d, "")
                {
                    act_with(app, "object.stroke", json!({"swatch": name}));
                    ui.close();
                }
            });
        });
    });
}

fn picture_format(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    group(ui, "Adjust", false, |ui| {
        for (icon, label) in [("corrections", "Corrections"), ("recolor", "Recolor"), ("compress", "Compress\nPictures")] {
            big_inner(ui, icon, label, false, true, false).on_hover_text("Coming in a later PressRoom update");
        }
        column(ui, |ui| {
            if small(ui, "change-picture", "Change Picture", sel).clicked() {
                super::insert_picture(app);
            }
            if small(ui, "reset-picture", "Reset Picture", sel).clicked() {
                act(app, "fit.contentProportionally");
            }
        });
    });
    group(ui, "Picture Styles", false, |ui| {
        small_menu(ui, "shape-outline", "Picture Border", sel, |ui| {
            for w in [0.5, 1.0, 2.25, 4.5] {
                if item(ui, &format!("{w} pt")) {
                    act_with(app, "object.stroke", json!({"weight": w, "swatch": "[Black]"}));
                }
            }
            if item(ui, "No Outline") {
                act_with(app, "object.stroke", json!({"swatch": "[None]"}));
            }
        });
    });
    arrange_group(app, ui);
    group(ui, "Crop", false, |ui| {
        if big(ui, "crop", "Crop", sel).clicked() {
            let _ = app.run("tool.select", json!({"tool": "directSelection"}));
        }
        column(ui, |ui| {
            if small(ui, "whole-page", "Fit", sel).clicked() {
                act(app, "fit.contentProportionally");
            }
            if small(ui, "picture", "Fill", sel).clicked() {
                act(app, "fit.fillFrameProportionally");
            }
            if small(ui, "align-to", "Center", sel).clicked() {
                act(app, "fit.centerContent");
            }
        });
    });
}

fn drawing_format(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Insert Shapes", false, |ui| {
        big_menu(ui, "shapes", "Shapes", doc, |ui| shapes_menu(app, ui));
    });
    shape_styles_group(app, ui);
    arrange_group(app, ui);
}

/// Table Tools › Layout, as in Publisher: select, delete and insert rows and columns, merge and
/// split cells, and where text sits in its cells.
fn table_layout(app: &mut DesignApp, ui: &mut Ui) {
    let in_table = on(app, "table.deleteRow");
    group(ui, "Table", false, |ui| {
        big_menu(ui, "select", "Select", in_table, |ui| {
            for (label, what) in [("Select Cell", "cell"), ("Select Column", "column"), ("Select Row", "row"), ("Select Table", "table")] {
                if item(ui, label) {
                    act_with(app, "table.select", json!({"what": what}));
                }
            }
        });
        if big(ui, "table", "Table\nOptions", true).clicked() {
            act(app, "app.tablePanel");
        }
    });
    group(ui, "Rows & Columns", false, |ui| {
        big_menu(ui, "table-delete", "Delete", in_table, |ui| {
            for (label, cmd) in [("Delete Columns", "table.deleteColumn"), ("Delete Rows", "table.deleteRow"), ("Delete Table", "table.delete")] {
                if item(ui, label) {
                    act(app, cmd);
                }
            }
        });
        for (icon, label, cmd) in [
            ("row-above", "Insert\nAbove", "table.insertRowAbove"),
            ("row-below", "Insert\nBelow", "table.insertRowBelow"),
            ("col-left", "Insert\nLeft", "table.insertColumnLeft"),
            ("col-right", "Insert\nRight", "table.insertColumnRight"),
        ] {
            if big(ui, icon, label, on(app, cmd)).clicked() {
                act(app, cmd);
            }
        }
    });
    group(ui, "Merge", false, |ui| {
        if big(ui, "merge-cells", "Merge\nCells", on(app, "table.merge")).clicked() {
            act(app, "table.merge");
        }
        if big(ui, "split-cells", "Split\nCells", on(app, "table.unmerge")).clicked() {
            act(app, "table.unmerge");
        }
    });
    group(ui, "Alignment", false, |ui| {
        column(ui, |ui| {
            for (icon, tip, vj) in
                [("cell-top", "Align Top", "top"), ("cell-middle", "Align Middle", "center"), ("cell-bottom", "Align Bottom", "bottom")]
            {
                if tool(ui, icon, tip, in_table, false).clicked() {
                    act_with(app, "table.setCell", json!({"vj": vj}));
                }
            }
        });
    });
}

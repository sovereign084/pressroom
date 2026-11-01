//! The ribbon: tab strip and the groups of each tab, laid out the way a Publisher user expects to
//! find them (same tabs, groups, button names and order). Every button acts through the engine's
//! commands (`menus::activate`), so it does exactly what the menu item of the same command does.

use egui::{Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use super::{Context, PR, Tab, art, tips};
use crate::DesignApp;

/// Height of the ribbon body: the groups and their labels on a rounded panel, with its margins.
pub const BODY_H: f32 = 100.0;
/// Height of a group's controls; the label sits below.
const CONTENT_H: f32 = 70.0;
const LABEL_H: f32 = 18.0;
/// One row of small buttons (three to a column).
const ROW_H: f32 = 22.0;
/// Icon sizes, as Publisher draws them: large buttons' and small buttons' (whose pictograms fill
/// more of their box than the art's 32-unit grid leaves, hence 20 for Publisher's 16).
const BIG_ICON: f32 = 36.0;
const SMALL_ICON: f32 = 20.0;

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
    // The hover highlight fades in and out rather than snapping.
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.1);
    let fill = if resp.is_pointer_button_down_on() {
        super::pal().pressed
    } else if on {
        super::pal().checked
    } else if hover > 0.0 {
        super::pal().hover.gamma_multiply(hover)
    } else {
        return;
    };
    // Soft, rounded highlights, as current Office draws them; a button that's on keeps a hairline.
    let edge = if on { Stroke::new(1.0, super::pal().hover_border) } else { Stroke::NONE };
    ui.painter().rect(r.shrink(0.5), PR::ROUND, fill, edge, StrokeKind::Inside);
}

fn text_color(enabled: bool) -> Color32 {
    if enabled { super::pal().text } else { super::pal().disabled }
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
    art::paint(ui.painter(), Rect::from_min_size(pos2(r.center().x - BIG_ICON / 2.0, r.min.y + 2.0), vec2(BIG_ICON, BIG_ICON)), icon, enabled);
    for (i, l) in lines.iter().enumerate() {
        ui.painter().text(pos2(r.center().x, r.min.y + 47.0 + i as f32 * 13.0), egui::Align2::CENTER_CENTER, l, font.clone(), text_color(enabled));
    }
    let tip = label.replace('\n', " ");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &tip));
    tips::screentip(resp, label)
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
    let w = if shown.is_empty() { 28.0 } else { text_w(ui, &shown, &font) + 34.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    art::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 4.0, r.center().y - SMALL_ICON / 2.0), vec2(SMALL_ICON, SMALL_ICON)), icon, enabled);
    if !shown.is_empty() {
        ui.painter().text(pos2(r.min.x + 29.0, r.center().y), egui::Align2::LEFT_CENTER, &shown, font, text_color(enabled));
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    tips::screentip(resp, label)
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
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(SMALL_ICON, SMALL_ICON)), icon, enabled);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, tip));
    tips::screentip(resp, tip)
}

/// A letter toggle: Bold, Italic, Underline.
fn letter(ui: &mut Ui, text: &str, tip: &str, enabled: bool, on: bool) -> Response {
    let (r, resp) = ui.allocate_exact_size(vec2(28.0, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    paint_button_bg(ui, r, &resp, on && enabled);
    let c = text_color(enabled);
    let p = ui.painter();
    match text {
        "B" => {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "B", crate::theme::semibold(16.5), c);
        }
        "I" => {
            // An upright font slanted by hand: two strokes of a serif I.
            let cx = r.center().x;
            let (top, bot) = (r.center().y - 7.0, r.center().y + 7.0);
            p.line_segment([pos2(cx + 3.0, top), pos2(cx - 3.0, bot)], Stroke::new(1.8, c));
            p.line_segment([pos2(cx, top), pos2(cx + 6.0, top)], Stroke::new(1.3, c));
            p.line_segment([pos2(cx - 6.0, bot), pos2(cx, bot)], Stroke::new(1.3, c));
        }
        _ => {
            p.text(r.center() - vec2(0.0, 1.5), egui::Align2::CENTER_CENTER, "U", FontId::proportional(16.5), c);
            p.line_segment([r.center() + vec2(-6.0, 8.0), r.center() + vec2(6.0, 8.0)], Stroke::new(1.3, c));
        }
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, tip));
    tips::screentip(resp, tip)
}

/// A group: its controls, its name below, an optional dialog launcher at the bottom right, and the
/// separator after it. Returns whether the launcher was clicked.
fn group(ui: &mut Ui, name: &str, launcher: bool, add: impl FnOnce(&mut Ui)) -> bool {
    let index = GROUP_INDEX.with(|i| i.replace(i.get() + 1));
    let top = ui.cursor().min;
    // Too narrow for the whole ribbon: this group folds into one button, as Office's do.
    if index >= GROUPS_SHOWN.with(std::cell::Cell::get) {
        let w = text_w(ui, name, &label_font()) + 20.0;
        let (r, resp) = ui.allocate_exact_size(vec2(w.max(FOLDED_W), CONTENT_H + LABEL_H), Sense::click());
        paint_button_bg(ui, r, &resp, false);
        let icon = match name {
            "Clipboard" => "paste",
            "Font" => "fonts",
            "Paragraph" => "align-left",
            "Styles" => "styles",
            "Objects" | "Text" => "textbox",
            "Arrange" => "wrap",
            "Editing" => "find",
            "Size" => "size",
            "Tables" | "Table" | "Table Formats" => "table",
            "Illustrations" | "Adjust" | "Picture Styles" => "picture",
            "Pages" => "page-insert",
            "Shape Styles" | "Fill" => "shape-fill",
            "Borders" => "shape-outline",
            "Zoom" => "zoom",
            "Show" => "guides",
            _ => "page-parts",
        };
        art::paint(ui.painter(), Rect::from_min_size(pos2(r.center().x - BIG_ICON / 2.0, r.min.y + 4.0), vec2(BIG_ICON, BIG_ICON)), icon, true);
        ui.painter().text(pos2(r.center().x, r.min.y + 48.0), egui::Align2::CENTER_CENTER, name, label_font(), super::pal().text);
        ui.painter().text(pos2(r.center().x, r.min.y + 62.0), egui::Align2::CENTER_CENTER, "\u{23F7}", FontId::proportional(9.0), super::pal().text);
        let mut launched = false;
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
            ui.horizontal_top(|ui| {
                ui.set_height(CONTENT_H);
                add(ui);
            });
            if launcher && ui.small_button(format!("{name} Settings…")).clicked() {
                launched = true;
                ui.close();
            }
        });
        ui.add_space(6.0);
        ui.painter().line_segment(
            [pos2(r.max.x + 3.0, top.y + 8.0), pos2(r.max.x + 3.0, top.y + CONTENT_H + LABEL_H - 6.0)],
            Stroke::new(1.0, super::pal().group_sep),
        );
        return launched;
    }
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
            super::pal().group_label,
        );
        let mut clicked = false;
        if launcher {
            let br = Rect::from_min_size(pos2(lr.max.x - 15.0, lr.min.y + 3.0), vec2(12.0, 12.0));
            let resp = ui.interact(br, ui.scope_id().with(("launcher", name)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(br, PR::ROUND, super::pal().hover);
            }
            let s = Stroke::new(1.0, super::pal().group_label);
            ui.painter().line_segment([br.min + vec2(3.0, 3.0), br.max - vec2(3.0, 3.0)], s);
            ui.painter().line_segment([pos2(br.min.x + 5.0, br.max.y - 3.0), br.max - vec2(3.0, 3.0)], s);
            ui.painter().line_segment([pos2(br.max.x - 3.0, br.min.y + 5.0), br.max - vec2(3.0, 3.0)], s);
            clicked = tips::screentip(resp, &format!("{name} Settings")).clicked();
        }
        clicked
    });
    let right = inner.response.rect.max.x + 3.0;
    ui.add_space(6.0);
    ui.painter().line_segment([pos2(right, top.y + 8.0), pos2(right, top.y + CONTENT_H + LABEL_H - 6.0)], Stroke::new(1.0, super::pal().group_sep));
    GROUP_WIDTHS.with(|w| {
        let mut w = w.borrow_mut();
        if w.len() <= index {
            w.resize(index + 1, 0.0);
        }
        if let Some(slot) = w.get_mut(index) {
            *slot = right - top.x + 6.0;
        }
    });
    inner.inner
}

/// A folded group's width.
const FOLDED_W: f32 = 62.0;

thread_local! {
    /// The next group this frame.
    static GROUP_INDEX: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many of this frame's groups show whole, left to right.
    static GROUPS_SHOWN: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) };
    /// This frame's whole groups' widths.
    static GROUP_WIDTHS: std::cell::RefCell<Vec<f32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// How many groups fit whole in `room`, the rest folding into buttons from the right.
pub(super) fn groups_that_fit(widths: &[f32], count: usize, room: f32) -> usize {
    let whole: f32 = widths.iter().sum();
    if whole <= room {
        return count;
    }
    let mut used = 0.0;
    for (k, w) in widths.iter().enumerate() {
        let folded = (count.saturating_sub(k + 1)) as f32 * (FOLDED_W + 6.0);
        if used + w + folded > room {
            return k;
        }
        used += w;
    }
    count
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
    // Each tab's whole groups' widths are known from the first frame it showed them all.
    let key = ui.scope_id().with(("pr_group_widths", tab, context));
    let known: Option<std::sync::Arc<Vec<f32>>> = ui.ctx().data(|d| d.get_temp(key));
    let room = ui.clip_rect().width() - 34.0;
    let shown = known.as_ref().map_or(usize::MAX, |w| groups_that_fit(w, w.len(), room));
    GROUPS_SHOWN.with(|s| s.set(shown));
    GROUP_INDEX.with(|i| i.set(0));
    GROUP_WIDTHS.with(|w| w.borrow_mut().clear());
    ui.horizontal_top(|ui| {
        ui.add_space(4.0);
        match tab {
            Tab::TableDesign => table_design(app, ui),
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
    if known.is_none() {
        let widths = GROUP_WIDTHS.with(|w| w.borrow().clone());
        if !widths.is_empty() {
            ui.ctx().data_mut(|d| d.insert_temp(key, std::sync::Arc::new(widths)));
            ui.ctx().request_repaint();
        }
    }
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
            if !super::paste_picture(app) {
                act(app, "edit.paste");
            }
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
            let any = app.session.active().is_some_and(|d| d.selection.text.is_some() || !d.selection.items.is_empty());
            let brush = small_inner(ui, "format-painter", "Format Painter", any, false, super::painter::armed());
            if brush.double_clicked() {
                super::painter::button(app, true);
            } else if brush.clicked() {
                super::painter::button(app, false);
            }
        });
    });
}

/// Character attributes of the text being edited, or of the selected text frames.
fn char_attrs(app: &mut DesignApp) -> Option<Value> {
    crate::panels::text_attrs(app).map(|a| a["chars"].clone())
}

/// Character formatting from the Font group: for the text selected, or, with none (Publisher keeps
/// the group working), for the next text box drawn.
fn set_chars(app: &mut DesignApp, text: bool, attrs: Value) {
    if text {
        let _ = app.run("type.char", json!({"attrs": attrs}));
    } else {
        super::set_next_text(&attrs);
    }
}

/// What the Font group shows with no text selected: the formatting waiting for the next text box
/// over the publication's [Basic Paragraph].
fn next_text_attrs(app: &DesignApp) -> Value {
    let mut v = json!({});
    if let Some(st) = app.session.active() {
        // The style's font as it resolves (a new publication's names none of its own).
        let (_, c) = st.doc.styles.resolve_para_style(designcraft_doc::BASIC_PARAGRAPH);
        v = json!({"fontFamily": c.font_family, "fontStyle": c.font_style, "size": c.size});
    }
    for (k, val) in super::next_text().unwrap_or_default() {
        v[k] = val;
    }
    v
}

fn font_group(app: &mut DesignApp, ui: &mut Ui) {
    let chars = char_attrs(app);
    let text = chars.is_some();
    // With nothing selected the group still works (for the next text box), as in Publisher;
    // with a picture or shape selected it's greyed.
    let nothing = app.session.active().is_some_and(|st| st.selection.items.is_empty() && st.selection.text.is_none());
    let doc = text || nothing;
    let ch = chars.unwrap_or_else(|| if doc { next_text_attrs(app) } else { Value::Null });
    let family = ch["fontFamily"].as_str().unwrap_or("").to_string();
    let style_name = ch["fontStyle"].as_str().unwrap_or("").to_string();
    // Bold and Italic as Publisher's buttons show them: Arial Black isn't bold, it's a font.
    let bold = super::fontnames::is_bold(&style_name);
    let italic = super::fontnames::is_italic(&style_name);
    let underline = ch["underline"].as_bool().unwrap_or(false) || ch["underline"].is_object();
    if group(ui, "Font", true, |ui| {
        column(ui, |ui| {
            row(ui, |ui| {
                let shown = if family.is_empty() { String::new() } else { super::fontnames::windows_name(&family, &style_name) };
                if let Some(name) = super::fontnames::picker(app, ui, &shown, 150.0, doc) {
                    let (f, s) = super::fontnames::resolve(app, &name, bold, italic);
                    set_chars(app, text, json!({"fontFamily": f, "fontStyle": s}));
                }
                ui.add_space(2.0);
                ui.add_enabled_ui(doc, |ui| {
                    if let Some(v) = crate::widgets::NumField::number("pr_font_size", ch["size"].as_f64(), "", 1)
                        .width(46.0)
                        .presets(SIZES)
                        .range(1.0, 1296.0)
                        .show(ui)
                    {
                        set_chars(app, text, json!({"size": v}));
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
                let toggle = |app: &mut DesignApp, b: bool, i: bool| {
                    let (f, s) = super::fontnames::resolve(app, &super::fontnames::windows_name(&family, &style_name), b, i);
                    super::set_next_text(&json!({"fontFamily": f, "fontStyle": s}));
                };
                if letter(ui, "B", "Bold (Ctrl+B)", doc, bold).clicked() {
                    if text { act(app, "type.bold") } else { toggle(app, !bold, italic) }
                }
                if letter(ui, "I", "Italic (Ctrl+I)", doc, italic).clicked() {
                    if text { act(app, "type.italic") } else { toggle(app, bold, !italic) }
                }
                if letter(ui, "U", "Underline (Ctrl+U)", doc, underline).clicked() {
                    if text { act(app, "type.underline") } else { super::set_next_text(&json!({"underline": !underline})) }
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
                        super::dialogs::open_spacing(app);
                    }
                });
                let resp = tool(ui, "font-color", "Font Color", doc, false);
                egui::Popup::menu(&resp).show(|ui| {
                    let cur = ch["fill"].as_str().unwrap_or("").to_string();
                    super::colors::menu(app, ui, super::colors::Target::Font, None, &cur);
                });
            });
        });
    }) {
        super::dialogs::open_font(app);
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
                            // Line spacing in lines → leading as a multiple of the size, for whole
                            // paragraphs, as Publisher sets it.
                            let size = ch_size(app);
                            let set = json!({"command": "type.char", "params": {"attrs": {"leading": {"kind": "points", "value": size * 1.2 * v}}}});
                            let steps = super::dialogs::over_paragraphs(app, vec![set]);
                            let _ = app.run("script.run", json!({"text": Value::Array(steps).to_string()}));
                        }
                    }
                    ui.separator();
                    if item(ui, "Line Spacing Options…") {
                        super::dialogs::open_paragraph(app);
                    }
                });
            });
        });
    }) {
        super::dialogs::open_paragraph(app);
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
        big_menu(ui, "table", "Table", doc, |ui| table_grid(app, ui));
        big_menu(ui, "shapes", "Shapes", doc, |ui| shapes_menu(app, ui));
    });
}

/// The size grid under the Table button, and Insert Table for any other size.
fn table_grid(app: &mut DesignApp, ui: &mut Ui) {
    const CELL: f32 = 18.0;
    let (cols, rows) = (10usize, 8usize);
    let (grid, resp) = ui.allocate_exact_size(vec2(cols as f32 * CELL, rows as f32 * CELL), Sense::click());
    let hover =
        resp.hover_pos().map(|p| (((p.x - grid.min.x) / CELL).ceil().max(1.0) as usize, ((p.y - grid.min.y) / CELL).ceil().max(1.0) as usize));
    for r in 0..rows {
        for c in 0..cols {
            let cell = Rect::from_min_size(grid.min + vec2(c as f32 * CELL, r as f32 * CELL), vec2(CELL, CELL)).shrink(1.5);
            let lit = hover.is_some_and(|(hc, hr)| c < hc && r < hr);
            ui.painter().rect(
                cell,
                1.0,
                if lit { super::pal().checked } else { super::pal().card },
                Stroke::new(1.0, if lit { super::pal().select } else { super::pal().ribbon_edge }),
                egui::StrokeKind::Inside,
            );
        }
    }
    let label = hover.map_or("Insert Table".to_string(), |(c, r)| format!("{c} x {r} Table"));
    ui.label(label);
    if resp.clicked()
        && let Some((c, r)) = hover
    {
        if let Err(e) = super::insert_table_of(app, r as u64, c as u64) {
            app.status(format!("Couldn't insert the table: {e}"));
        }
        ui.close();
    }
    ui.separator();
    if item(ui, "Insert Table…") {
        super::dialogs::open_table(app);
    }
}

fn arrange_group(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    let many = app.session.active().is_some_and(|d| d.selection.items.len() > 1);
    group(ui, "Arrange", false, |ui| {
        big_menu(ui, "wrap", "Wrap\nText", sel, |ui| {
            for (label, mode) in [("None", "none"), ("Square", "boundingBox"), ("Tight", "contour"), ("Top and Bottom", "jumpObject")] {
                if item(ui, label) {
                    act_with(app, "object.textWrap", json!({"mode": mode}));
                }
            }
            ui.separator();
            if item(ui, "More Layout Options…") {
                super::dialogs::open_format(app);
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
                super::find::open(app, false);
            }
            if small(ui, "replace", "Replace", doc).clicked() {
                super::find::open(app, true);
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
        super::dialogs::open_insert_page(app);
    }
}

fn insert(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Pages", false, |ui| {
        big_menu(ui, "page-insert", "Page", doc, |ui| page_menu(app, ui));
    });
    group(ui, "Tables", false, |ui| {
        big_menu(ui, "table", "Table", doc, |ui| table_grid(app, ui));
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
        use super::blocks;
        big_menu(ui, "page-parts", "Page\nParts", doc, |ui| blocks::gallery(app, ui, "Page Parts", &blocks::PAGE_PARTS));
        big_menu(ui, "calendars", "Calendars", doc, |ui| blocks::gallery(app, ui, "Calendars", &blocks::CALENDARS));
        big_menu(ui, "borders", "Borders &\nAccents", doc, |ui| blocks::gallery(app, ui, "Borders & Accents", &blocks::BORDERS));
        big_menu(ui, "ads", "Advertise-\nments", doc, |ui| blocks::gallery(app, ui, "Advertisements", &blocks::ADS));
    });
    group(ui, "Text", false, |ui| {
        if big(ui, "textbox", "Draw Text\nBox", doc).clicked() {
            super::start_draw(app, "type");
        }
        if big(ui, "insert-file", "Insert\nFile", doc).clicked() {
            super::insert_file(app);
        }
        big_menu(ui, "wordart", "WordArt", doc, |ui| super::wordart::gallery(app, ui));
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
                    super::taskpane::open(app, "glyphs");
                }
            });
            if small(ui, "datetime", "Date & Time", doc).clicked() {
                super::dialogs::open_date(app);
            }
        });
    });
    group(ui, "Links", false, |ui| {
        if big(ui, "link", "Link", doc && has_selection(app)).clicked() {
            super::dialogs::open_hyperlink(app);
        }
        if big(ui, "bookmark", "Bookmark", doc).clicked() {
            super::taskpane::open(app, "bookmarks");
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
            for (label, top, align) in [
                ("Top Left", true, "left"),
                ("Top Center", true, "center"),
                ("Top Right", true, "right"),
                ("Bottom Left", false, "left"),
                ("Bottom Center", false, "center"),
                ("Bottom Right", false, "right"),
            ] {
                if item(ui, label) {
                    super::page_number_at(app, top, align);
                }
            }
            ui.separator();
            if item(ui, "Insert Page Number") {
                act(app, "app.insertCurrentPageNumber");
            }
            if item(ui, "Page Number Format…") {
                super::dialogs::open_number_format(app);
            }
        });
    });
}

fn shapes_menu(app: &mut DesignApp, ui: &mut Ui) {
    super::shapes::gallery(app, ui, super::shapes::Use::Draw);
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
                super::dialogs::open_guides(app);
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
                super::dialogs::open_setup(app);
            }
        });
    }) {
        super::dialogs::open_setup(app);
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
                super::dialogs::open_guides(app);
            }
            if item(ui, "Add Ruler Guides…") {
                act(app, "layout.createGuides");
            }
        });
        column(ui, |ui| {
            let mut snap = checked(app, "view.snapToGuides");
            ui.add_enabled_ui(doc, |ui| {
                ui.label(egui::RichText::new("Align To").color(super::pal().text).font(label_font()));
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
            super::dialogs::open_move(app, cur);
        }
        if big(ui, "rename", "Rename", doc).clicked() {
            super::dialogs::open_rename(app, cur);
        }
        big_menu(ui, "master", "Master\nPages", doc, |ui| {
            if item(ui, "Edit Master Pages") {
                super::set_master_view(app, true);
            }
            if item(ui, "Apply Master Page…") {
                super::dialogs::open_apply_master(app);
            }
            if item(ui, "Add Master Page…") {
                super::dialogs::open_new_master(app);
            }
        });
    });
    group(ui, "Schemes", false, |ui| {
        big_menu(ui, "schemes", "Color\nSchemes", doc, |ui| super::scheme::color_gallery(app, ui));
        big_menu(ui, "fonts", "Fonts", doc, |ui| super::scheme::font_gallery(app, ui));
    });
    group(ui, "Page Background", false, |ui| {
        big_menu(ui, "background", "Background", doc, |ui| {
            super::colors::menu(app, ui, super::colors::Target::Background, Some("No Background"), "");
        });
    });
}

// ───────────────────────────── Mailings / Review ─────────────────────────────

fn mailings(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Start", false, |ui| {
        big_menu(ui, "mail-merge", "Mail\nMerge", doc, |ui| {
            if item(ui, "Step-by-Step Mail Merge Wizard…") {
                super::taskpane::open(app, "dataMerge");
            }
        });
        if big(ui, "insert-file", "Select\nRecipients", doc).clicked()
            && let Some(pick) = app.services.pick_open.as_mut()
            && let Some(path) = pick("data")
            && let Err(e) = app.run("data.source.select", json!({"path": path}))
        {
            app.status(format!("Couldn't read the list: {e}"));
        }
        if big(ui, "page-parts", "Edit\nRecipient List", doc).clicked() {
            super::taskpane::open(app, "dataMerge");
        }
    });
    group(ui, "Write & Insert Fields", false, |ui| {
        big_menu(ui, "textbox", "Insert\nMerge Field", doc, |ui| {
            let fields = app.session.execute("data.fields", &json!({})).unwrap_or(Value::Null);
            let names: Vec<String> =
                fields.as_array().map(|a| a.iter().filter_map(|f| f["name"].as_str().map(str::to_string)).collect()).unwrap_or_default();
            if names.is_empty() {
                ui.label("Select Recipients first.");
            }
            for name in names {
                if item(ui, &name) {
                    let _ = app.run("data.placeholder.add", json!({"field": name}));
                }
            }
        });
    });
    group(ui, "Preview & Finish", false, |ui| {
        if big(ui, "whole-page", "Preview\nResults", doc).clicked() {
            super::taskpane::open(app, "dataMerge");
        }
        if big(ui, "mail-merge", "Finish &\nMerge", doc).clicked() {
            match app.run("data.merge", json!({})) {
                Ok(_) => app.status("The merged publication is ready: print it from File, Print."),
                Err(e) => app.status(format!("Couldn't merge: {e}")),
            }
        }
    });
}

fn review(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Proofing", false, |ui| {
        if big(ui, "spelling", "Spelling", doc).clicked() {
            super::spelling::open(app);
        }
        let mut on_spelling = app.ui.dynamic_spelling;
        if ui.checkbox(&mut on_spelling, egui::RichText::new("Check spelling\nas you type").font(label_font())).changed() {
            app.ui.dynamic_spelling = on_spelling;
        }
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
    group(ui, "Graphics", false, |ui| {
        if big(ui, "picture", "Graphics\nManager", doc).clicked() {
            super::taskpane::open(app, "links");
        }
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
            let (fit, grow) = app
                .session
                .active()
                .and_then(|st| {
                    st.doc
                        .item(super::selected_text_frame(app)?)?
                        .text_frame()
                        .map(|t| (t.options.text_fit, t.options.auto_size != designcraft_doc::AutoSize::Off))
                })
                .unwrap_or_default();
            use designcraft_doc::TextFit;
            for (label, on, params) in [
                ("Best Fit", fit == TextFit::Best, json!({"textFit": "best"})),
                ("Shrink Text On Overflow", fit == TextFit::Shrink, json!({"textFit": "shrink"})),
                ("Grow Text Box to Fit", grow, json!({"autoSize": "heightOnly", "autoSizeReference": "topLeft"})),
                ("Do Not Autofit", fit == TextFit::None && !grow, json!({"textFit": "none", "autoSize": "off"})),
            ] {
                if ui.selectable_label(on, label).clicked() {
                    let mut p = params;
                    p["ids"] = super::text_frame_ids(app);
                    act_with(app, "object.textFrameOptions", p);
                    ui.close();
                }
            }
        });
        column(ui, |ui| {
            small_menu(ui, "columns", "Columns", frame, |ui| {
                for n in 1..=3 {
                    if item(ui, &format!("{n}")) {
                        let ids = super::text_frame_ids(app);
                        act_with(app, "object.textFrameOptions", json!({"columns": n, "ids": ids}));
                    }
                }
                ui.separator();
                if item(ui, "More Columns…") {
                    super::dialogs::open_format(app);
                }
            });
            small_menu(ui, "margins", "Margins", frame, |ui| {
                for (label, pt) in [("None (0\")", 0.0), ("Narrow (0.04\")", 2.88), ("Moderate (0.1\")", 7.2), ("Wide (0.2\")", 14.4)] {
                    if item(ui, label) {
                        let ids = super::text_frame_ids(app);
                        act_with(app, "object.textFrameOptions", json!({"inset": pt, "ids": ids}));
                    }
                }
                ui.separator();
                if item(ui, "Custom Margins…") {
                    super::dialogs::open_format(app);
                }
            });
            if small(ui, "hyphenation", "Hyphenation", frame).clicked() {
                super::dialogs::open_hyphenation(app);
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
        big_menu(ui, "font-color", "Text\nEffects", frame, |ui| {
            ui.menu_button("Text Fill", |ui| super::colors::menu(app, ui, super::colors::Target::Font, None, ""));
            ui.menu_button("Text Outline", |ui| super::colors::menu(app, ui, super::colors::Target::TextOutline, Some("No Outline"), ""));
            effect_rows(app, ui);
        });
        column(ui, |ui| {
            small_menu(ui, "fonts", "Number Style", frame, |ui| {
                for (label, figures) in [
                    ("Default", "default"),
                    ("Lining", "proportionalLining"),
                    ("Tabular Lining", "tabularLining"),
                    ("Old-Style", "proportionalOldstyle"),
                    ("Tabular Old-Style", "tabularOldstyle"),
                ] {
                    if item(ui, label) {
                        act_with(app, "type.openType", json!({"figures": figures}));
                    }
                }
            });
            small_menu(ui, "fonts", "Ligatures", frame, |ui| {
                for (label, on) in [("Use Ligatures", true), ("No Ligatures", false)] {
                    if item(ui, label) {
                        let _ = app.run("type.char", json!({"attrs": {"ligatures": on}}));
                    }
                }
            });
            if small(ui, "fonts", "Small Caps", frame).clicked() {
                let cur = crate::panels::text_attrs(app).and_then(|a| a["chars"]["capitalization"].as_str().map(str::to_string)).unwrap_or_default();
                let to = if cur == "smallCaps" { "normal" } else { "smallCaps" };
                let _ = app.run("type.char", json!({"attrs": {"capitalization": to}}));
            }
        });
    });
    shape_styles_group(app, ui);
    arrange_group(app, ui);
    size_group(app, ui);
}

/// Height and Width, as the Format tabs have them.
fn size_group(app: &mut DesignApp, ui: &mut Ui) {
    let info = crate::panels::sel_info(app);
    let unit = app.session.active().map_or(designcraft_geom::units::Unit::Inches, |d| d.doc.settings.horizontal_units);
    let (h, w) = info.as_ref().map_or((None, None), |i| (Some(i.page_rect.height()), Some(i.page_rect.width())));
    group(ui, "Size", false, |ui| {
        column(ui, |ui| {
            ui.add_enabled_ui(info.is_some(), |ui| {
                for (label, id, value, key) in [("Height:", "pr_size_h", h, "height"), ("Width:", "pr_size_w", w, "width")] {
                    row(ui, |ui| {
                        ui.add_sized(vec2(44.0, ROW_H), egui::Label::new(egui::RichText::new(label).font(label_font())));
                        if let Some(v) = crate::widgets::NumField::measure(id, value, unit).width(66.0).range(0.5, 15_552.0).show(ui) {
                            let mut p = json!({"ref": 0});
                            p[key] = json!(v);
                            act_with(app, "transform.set", p);
                        }
                    });
                }
            });
        });
    });
}

/// Shadow, Glow, Soft Edges and Bevel, on a shape, a picture or a text box's text.
fn effect_rows(app: &mut DesignApp, ui: &mut Ui) {
    let ids = if crate::panels::sel_info(app).is_some() { Value::Null } else { super::text_frame_ids(app) };
    let with_ids = |mut p: Value| {
        if !ids.is_null() {
            p["ids"] = ids.clone();
        }
        p
    };
    ui.menu_button("Shadow", |ui| {
        if item(ui, "No Shadow") {
            act_with(app, "object.effects", with_ids(json!({"effects": {"dropShadow": {"on": false}}})));
        }
        for (label, distance, size, angle, opacity) in
            [("Soft, Lower Right", 4.0, 5.0, 135.0, 0.4), ("Sharp, Lower Right", 3.0, 0.0, 135.0, 0.6), ("Soft, Below", 4.0, 6.0, 90.0, 0.4), ("Soft All Around", 0.0, 8.0, 90.0, 0.5)]
        {
            if item(ui, label) {
                act_with(app, "object.effects", with_ids(json!({"effects": {"dropShadow": {"on": true, "distance": distance, "size": size, "angle": angle, "opacity": opacity, "color": "[Black]"}}})));
            }
        }
    });
    ui.menu_button("Glow", |ui| {
        if item(ui, "No Glow") {
            act_with(app, "object.effects", with_ids(json!({"effects": {"outerGlow": {"on": false}}})));
        }
        for (label, size) in [("Small", 5.0), ("Medium", 10.0), ("Large", 18.0)] {
            if item(ui, label) {
                let color =
                    if app.session.active().is_some_and(|d| d.doc.swatch("Scheme Accent 1").is_some()) { "Scheme Accent 1" } else { "[Black]" };
                act_with(
                    app,
                    "object.effects",
                    with_ids(json!({"effects": {"outerGlow": {"on": true, "color": color, "opacity": 0.7, "size": size, "mode": "Normal"}}})),
                );
            }
        }
    });
    ui.menu_button("Soft Edges", |ui| {
        for (label, w) in [("No Soft Edges", 0.0), ("2.5 pt", 2.5), ("5 pt", 5.0), ("10 pt", 10.0), ("25 pt", 25.0)] {
            if item(ui, label) {
                act_with(app, "object.feather", with_ids(json!({"width": w})));
            }
        }
    });
    ui.menu_button("Bevel", |ui| {
        if item(ui, "No Bevel") {
            act_with(app, "object.bevel", with_ids(json!({"on": false})));
        }
        for (label, size) in [("Soft", 3.0), ("Round", 6.0), ("Wide", 12.0)] {
            if item(ui, label) {
                act_with(app, "object.bevel", with_ids(json!({"on": true, "size": size})));
            }
        }
    });
}

fn shape_styles_group(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    group(ui, "Shape Styles", false, |ui| {
        column(ui, |ui| {
            small_menu(ui, "shape-fill", "Shape Fill", sel, |ui| {
                super::colors::menu(app, ui, super::colors::Target::Fill, Some("No Fill"), "");
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
                ui.menu_button("Arrows", |ui| {
                    for (label, start, end) in [
                        ("No Arrows", "none", "none"),
                        ("Arrow at the End", "none", "triangle"),
                        ("Arrow at the Start", "triangle", "none"),
                        ("Arrows at Both Ends", "triangle", "triangle"),
                        ("Round Ends", "circleSolid", "circleSolid"),
                    ] {
                        if item(ui, label) {
                            act_with(app, "object.stroke", json!({"start": start, "end": end}));
                        }
                    }
                });
                ui.separator();
                super::colors::menu(app, ui, super::colors::Target::Line, Some("No Outline"), "");
            });
            small_menu(ui, "wrap", "Shape Effects", sel, |ui| effect_rows(app, ui));
        });
    });
}

fn picture_format(app: &mut DesignApp, ui: &mut Ui) {
    let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
    group(ui, "Adjust", false, |ui| {
        // Galleries drawn from the picture itself, as Publisher's are.
        big_menu(ui, "corrections", "Corrections", sel, |ui| super::pictures::corrections_gallery(app, ui));
        big_menu(ui, "recolor", "Recolor", sel, |ui| super::pictures::recolor_gallery(app, ui));
        if big(ui, "compress", "Compress\nPictures", has_doc(app)).clicked() {
            super::pictures::open_compress(app);
        }
        column(ui, |ui| {
            if small(ui, "change-picture", "Change Picture", sel).clicked() {
                super::insert_picture(app);
            }
            // Corrections and recolouring off, and the picture fitted to its frame again.
            if small(ui, "reset-picture", "Reset Picture", sel).clicked() {
                let steps = json!([{"command": "picture.reset", "params": {}}, {"command": "object.fit", "params": {"mode": "fitProportionally"}}]);
                let _ = app.run("script.run", json!({"text": steps.to_string()}));
            }
        });
    });
    group(ui, "Picture Styles", false, |ui| {
        big_menu(ui, "picture", "Picture\nStyles", sel, |ui| {
            for (label, steps) in picture_styles() {
                if item(ui, label) {
                    let _ = app.run("script.run", json!({"text": steps.to_string()}));
                }
            }
        });
        column(ui, |ui| {
            small_menu(ui, "shape-outline", "Picture Border", sel, |ui| {
                for w in [0.5, 1.0, 2.25, 4.5] {
                    if item(ui, &format!("{w} pt")) {
                        act_with(app, "object.stroke", json!({"weight": w, "swatch": "[Black]"}));
                    }
                }
                ui.separator();
                super::colors::menu(app, ui, super::colors::Target::Line, Some("No Outline"), "");
            });
            small_menu(ui, "wrap", "Picture Effects", sel, |ui| effect_rows(app, ui));
            small_menu(ui, "textbox", "Caption", sel, |ui| {
                for (label, position) in [("Below the Picture", "below"), ("Above the Picture", "above")] {
                    if item(ui, label) {
                        act_with(app, "object.caption", json!({"text": "Caption", "position": position, "offset": 4, "height": 24}));
                    }
                }
            });
        });
    });
    let two_pictures = app.session.active().is_some_and(|d| {
        d.selection.items.len() == 2
            && d.selection.items.iter().all(|id| d.doc.item(*id).is_some_and(|it| matches!(it.content, designcraft_doc::Content::Graphic(_))))
    });
    group(ui, "Switch", false, |ui| {
        if big(ui, "change-picture", "Swap", two_pictures).clicked() {
            let ids: Vec<u64> = app.session.active().map(|d| d.selection.items.iter().map(|i| i.0).collect()).unwrap_or_default();
            let steps = json!([
                {"command": "picture.swap", "params": {"ids": ids}},
                {"command": "object.fit", "params": {"mode": "fillProportionally", "ids": ids}},
            ]);
            let _ = app.run("script.run", json!({"text": steps.to_string()}));
        }
    });
    arrange_group(app, ui);
    group(ui, "Crop", false, |ui| {
        // Crop stays lit while cropping; a second click (or Esc, or a click away) ends it.
        let cropping = app.session.tool_id() == "directSelection";
        if big_inner(ui, "crop", "Crop", sel, false, cropping).clicked() {
            let _ = app.run("tool.select", json!({"tool": if cropping { "selection" } else { "directSelection" }}));
        }
        column(ui, |ui| {
            if small(ui, "whole-page", "Fit", sel).clicked() {
                act(app, "fit.contentProportionally");
            }
            if small(ui, "picture", "Fill", sel).clicked() {
                act(app, "fit.fillFrameProportionally");
            }
            if small(ui, "align-to", "Clear Crop", sel).clicked() {
                let _ = app.run("object.fit", json!({"mode": "fitFrameToContent"}));
            }
        });
    });
    size_group(app, ui);
}

/// Picture Styles: PressRoom's own, each one undo step.
fn picture_styles() -> Vec<(&'static str, Value)> {
    let step = |c: &str, p: Value| json!({"command": c, "params": p});
    let clear = || {
        vec![
            step("object.convertShape", json!({"to": "rectangle"})),
            step("object.stroke", json!({"swatch": "[None]"})),
            step("object.effects", json!({"effects": {"dropShadow": {"on": false}, "outerGlow": {"on": false}}})),
        ]
    };
    let with = |more: Vec<Value>| {
        let mut v = clear();
        v.extend(more);
        Value::Array(v)
    };
    vec![
        ("No Style", Value::Array(clear())),
        ("Thin Black Frame", with(vec![step("object.stroke", json!({"swatch": "[Black]", "weight": 1.0, "type": {"kind": "solid"}}))])),
        (
            "Thick White Frame",
            with(vec![
                step("object.stroke", json!({"swatch": "[Paper]", "weight": 6.0, "align": "inside"})),
                step(
                    "object.effects",
                    json!({"effects": {"dropShadow": {"on": true, "distance": 3.0, "size": 5.0, "angle": 135.0, "opacity": 0.45, "color": "[Black]"}}}),
                ),
            ]),
        ),
        (
            "Soft Shadow",
            with(vec![step(
                "object.effects",
                json!({"effects": {"dropShadow": {"on": true, "distance": 4.0, "size": 7.0, "angle": 135.0, "opacity": 0.4, "color": "[Black]"}}}),
            )]),
        ),
        ("Rounded Corners", with(vec![step("object.convertShape", json!({"to": "roundedRectangle"}))])),
        ("Oval", with(vec![step("object.convertShape", json!({"to": "ellipse"}))])),
    ]
}

fn drawing_format(app: &mut DesignApp, ui: &mut Ui) {
    let doc = has_doc(app);
    group(ui, "Insert Shapes", false, |ui| {
        big_menu(ui, "shapes", "Shapes", doc, |ui| shapes_menu(app, ui));
        let sel = app.session.active().is_some_and(|d| !d.selection.items.is_empty());
        big_menu(ui, "rotate", "Change\nShape", sel, |ui| super::shapes::gallery(app, ui, super::shapes::Use::Change));
    });
    shape_styles_group(app, ui);
    arrange_group(app, ui);
    size_group(app, ui);
}

/// Table Tools › Design: the cell's fill, its lines, and ready-made looks.
fn table_design(app: &mut DesignApp, ui: &mut Ui) {
    let in_table = on(app, "table.deleteRow");
    group(ui, "Table Formats", false, |ui| {
        big_menu(ui, "table", "Table\nFormats", in_table, |ui| {
            for (label, format) in [("Plain Grid", 0), ("Header Row", 1), ("Banded Rows", 2), ("No Lines", 3)] {
                if item(ui, label) {
                    apply_table_format(app, format);
                }
            }
        });
    });
    group(ui, "Fill", false, |ui| {
        big_menu(ui, "shape-fill", "Fill", in_table, |ui| {
            super::colors::menu(app, ui, super::colors::Target::Cell, Some("No Fill"), "");
        });
    });
    group(ui, "Borders", false, |ui| {
        column(ui, |ui| {
            small_menu(ui, "shape-outline", "Line Weight", in_table, |ui| {
                for w in [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0] {
                    if item(ui, &format!("{w} pt")) {
                        act_with(app, "table.setCell", json!({"stroke": {"weight": w, "edges": "all"}}));
                    }
                }
            });
            small_menu(ui, "shape-outline", "Line Color", in_table, |ui| {
                let doc = app.session.active().map(|d| d.doc.clone());
                if let Some(d) = doc
                    && let Some(name) = crate::panels::swatch_list(ui, &d, "")
                {
                    act_with(app, "table.setCell", json!({"stroke": {"color": name, "edges": "all"}}));
                }
            });
            small_menu(ui, "shape-outline", "Borders", in_table, |ui| {
                for (label, edges, weight) in [
                    ("All Borders", "all", 0.75),
                    ("Outside Borders", "outer", 0.75),
                    ("Inside Borders", "inner", 0.75),
                    ("Top Border", "top", 0.75),
                    ("Bottom Border", "bottom", 0.75),
                    ("Left Border", "left", 0.75),
                    ("Right Border", "right", 0.75),
                    ("No Borders", "all", 0.0),
                ] {
                    if item(ui, label) {
                        act_with(app, "table.setCell", json!({"stroke": {"weight": weight, "edges": edges}}));
                    }
                }
            });
        });
    });
}

/// A ready-made look over the whole table, as one undo step, keeping the selection.
fn apply_table_format(app: &mut DesignApp, format: u8) {
    let Some((rows, cols)) = app
        .session
        .execute("table.get", &json!({}))
        .ok()
        .map(|t| (t["rows"].as_u64().unwrap_or(0), t["columns"].as_u64().or_else(|| t["cols"].as_u64()).unwrap_or(0)))
    else {
        return;
    };
    if rows == 0 || cols == 0 {
        return;
    }
    let all = json!({"rows": [0, rows - 1], "cols": [0, cols - 1]});
    let set = |extra: Value, region: &Value| {
        let mut p = region.clone();
        if let (Some(p), Some(e)) = (p.as_object_mut(), extra.as_object()) {
            for (k, v) in e {
                p.insert(k.clone(), v.clone());
            }
        }
        json!({"command": "table.setCell", "params": p})
    };
    let mut steps =
        vec![set(json!({"fill": "[None]", "stroke": {"weight": if format == 3 { 0.0 } else { 0.75 }, "color": "[Black]", "edges": "all"}}), &all)];
    let shade = if app.session.active().is_some_and(|d| d.doc.swatch("Scheme Accent 1").is_some()) { "Scheme Accent 1" } else { "[Black]" };
    if format == 1 {
        steps.push(set(json!({"fill": shade, "tint": 0.3}), &json!({"rows": [0, 0], "cols": [0, cols - 1]})));
    }
    if format == 2 {
        for r in (0..rows).step_by(2) {
            steps.push(set(json!({"fill": shade, "tint": 0.15}), &json!({"rows": [r, r], "cols": [0, cols - 1]})));
        }
    }
    let _ = app.run("script.run", json!({"text": Value::Array(steps).to_string()}));
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
            super::taskpane::open(app, "table");
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

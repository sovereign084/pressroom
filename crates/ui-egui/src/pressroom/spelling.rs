//! Spelling, as Publisher has it: F7 walks the publication's misspelled words in a dialog with
//! Ignore, Ignore All, Change, Change All and Add, and a right-click on a word offers its
//! suggestions.

use egui::Ui;
use serde_json::{Value, json};

use crate::DesignApp;
use crate::dialogs::Dialog;

pub(crate) const DIALOG: &str = "pr.spelling";

/// What's misspelled from a place in the publication on: the first word at or after `from`, and
/// its place. Words on the session's ignore list don't count.
fn next_word(app: &mut DesignApp, from: &Value) -> Option<Value> {
    let found = app.session.execute("spelling.check", &json!({"suggestions": false})).ok()?;
    let ignored = app.ui.pressroom_ignored.clone();
    let list = found.as_array()?;
    let start = from.get("start").and_then(Value::as_u64).unwrap_or(0);
    let story = from.get("story").and_then(Value::as_u64);
    // The stories come in their own order: find `from`'s story, then go on through the rest.
    let at = story.and_then(|s| list.iter().position(|w| w["story"].as_u64() == Some(s))).unwrap_or(0);
    let wanted = |w: &&Value| !ignored.contains(&w["word"].as_str().unwrap_or("").to_lowercase());
    let later = list.iter().skip(at).find(|w| wanted(w) && (w["story"].as_u64() != story || w["start"].as_u64().unwrap_or(0) >= start));
    later.or_else(|| list.iter().take(at).find(wanted)).cloned()
}

/// Selects `word` so it shows on the page, and fills the dialog in for it; or says the check is done.
fn show_word(app: &mut DesignApp, d: &mut Dialog, word: Option<Value>) {
    let Some(w) = word else {
        d.fields.insert("done".into(), json!(true));
        return;
    };
    let _ = app.run("text.select", json!({"story": w["story"], "anchor": w["start"], "focus": w["end"]}));
    app.session.set_tool("type");
    if let Some(page) = page_of(app, &w) {
        crate::canvas::go_to_page(app, page);
    }
    let text = w["word"].as_str().unwrap_or("").to_string();
    let suggestions = app.session.execute("spelling.suggest", &json!({"word": text})).unwrap_or(json!([]));
    let first = suggestions.get(0).and_then(Value::as_str).unwrap_or(&text).to_string();
    d.fields.insert("change".into(), json!(first));
    d.fields.insert("suggestions".into(), suggestions);
    d.fields.insert("current".into(), w);
}

/// The page the word's text box is on.
fn page_of(app: &DesignApp, w: &Value) -> Option<usize> {
    let st = app.session.active()?;
    let story = st.doc.story(designcraft_doc::StoryId(w["story"].as_u64()?))?;
    let frame = *story.frames.first()?;
    let cs = app.session.cache.get(&st.doc, story.id, None);
    let start = w["start"].as_u64()? as usize;
    let frame = cs.frames.iter().find(|f| f.lines.iter().any(|l| l.range.contains(&start))).map_or(frame, |f| f.frame);
    st.doc.page_of_item(frame)
}

/// F7: from the first word.
pub(super) fn open(app: &mut DesignApp) {
    let mut d = Dialog::new(DIALOG, json!({}));
    let first = next_word(app, &json!({}));
    show_word(app, &mut d, first);
    app.ui.dialog = Some(d);
}

/// After acting on the current word: goes on from `from`.
fn advance(app: &mut DesignApp, d: &mut Dialog, from: Value) {
    let next = next_word(app, &from);
    show_word(app, d, next);
}

pub(crate) fn body(app: &mut DesignApp, ui: &mut Ui, d: &mut Dialog) {
    if d.b("done") {
        ui.add_space(8.0);
        ui.label("The spelling check is complete.");
        ui.add_space(8.0);
        return;
    }
    let cur = d.fields.get("current").cloned().unwrap_or(Value::Null);
    let word = cur["word"].as_str().unwrap_or("").to_string();
    let (story, start, end) = (cur["story"].clone(), cur["start"].as_u64().unwrap_or(0), cur["end"].as_u64().unwrap_or(0));
    let mut change = d.s("change");
    let suggestions: Vec<String> = d
        .fields
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let mut act = None;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(270.0);
            ui.label("Not in dictionary:");
            ui.label(egui::RichText::new(&word).strong());
            ui.add_space(6.0);
            ui.label("Change to:");
            if ui.add(egui::TextEdit::singleline(&mut change).desired_width(260.0)).changed() {
                d.fields.insert("change".into(), json!(change));
            }
            ui.add_space(6.0);
            ui.label("Suggestions:");
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(246.0);
                egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                    ui.set_min_height(110.0);
                    if suggestions.is_empty() {
                        ui.label(egui::RichText::new("(no suggestions)").color(super::pal().group_label));
                    }
                    for s in &suggestions {
                        if ui.selectable_label(*s == change, s).clicked() {
                            d.fields.insert("change".into(), json!(s));
                        }
                    }
                });
            });
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.add_space(16.0);
            let size = egui::vec2(110.0, 0.0);
            for (label, what) in [("Ignore", 0), ("Ignore All", 1), ("Change", 2), ("Change All", 3), ("Add", 4)] {
                if ui.add_sized(size, egui::Button::new(label)).clicked() {
                    act = Some(what);
                }
                if what == 1 {
                    ui.add_space(8.0);
                }
            }
        });
    });
    let to = d.s("change");
    let lower = word.to_lowercase();
    match act {
        Some(0) => advance(app, d, json!({"story": story, "start": end})),
        Some(1) => {
            app.ui.pressroom_ignored.push(lower);
            advance(app, d, json!({"story": story, "start": end}));
        }
        Some(2) => {
            let _ = app.run("spelling.change", json!({"story": story, "start": start, "end": end, "to": to}));
            advance(app, d, json!({"story": story, "start": start + to.len() as u64}));
        }
        Some(3) => {
            change_all(app, &lower, &to);
            advance(app, d, json!({"story": story, "start": start + to.len() as u64}));
        }
        Some(4) => {
            let _ = app.run("spelling.addWord", json!({"word": word}));
            advance(app, d, json!({"story": story, "start": end}));
        }
        _ => {}
    }
}

/// Every place the word appears, as one undo step (last first, so the places stay right).
fn change_all(app: &mut DesignApp, lower: &str, to: &str) {
    let Ok(found) = app.session.execute("spelling.check", &json!({"suggestions": false})) else { return };
    let mut steps: Vec<Value> = found
        .as_array()
        .into_iter()
        .flatten()
        .filter(|w| w["word"].as_str().unwrap_or("").to_lowercase() == lower)
        .map(|w| json!({"command": "spelling.change", "params": {"story": w["story"], "start": w["start"], "end": w["end"], "to": to}}))
        .collect();
    steps.reverse();
    let _ = app.run("script.run", json!({"text": Value::Array(steps).to_string()}));
}

/// The misspelled word at the text cursor, for the right-click menu: captured when the menu opens,
/// because the menu's body draws again every frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Clicked {
    pub story: u64,
    pub start: u64,
    pub end: u64,
    pub word: String,
    pub suggestions: Vec<String>,
}

pub(crate) fn capture(app: &mut DesignApp) {
    app.ui.pressroom_spell = None;
    if !app.ui.dynamic_spelling {
        return;
    }
    let Some(t) = app.session.active().and_then(|st| st.selection.text) else { return };
    let caret = t.range().start as u64;
    let Ok(found) = app.session.execute("spelling.check", &json!({"story": t.story.0, "suggestions": true})) else { return };
    let ignored = app.ui.pressroom_ignored.clone();
    app.ui.pressroom_spell = found
        .as_array()
        .into_iter()
        .flatten()
        .find(|w| w["start"].as_u64().unwrap_or(u64::MAX) <= caret && caret <= w["end"].as_u64().unwrap_or(0))
        .filter(|w| !ignored.contains(&w["word"].as_str().unwrap_or("").to_lowercase()))
        .map(|w| Clicked {
            story: t.story.0,
            start: w["start"].as_u64().unwrap_or(0),
            end: w["end"].as_u64().unwrap_or(0),
            word: w["word"].as_str().unwrap_or("").to_string(),
            suggestions: w["suggestions"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        });
}

/// The rows at the top of the right-click menu on a misspelled word.
pub(super) fn menu_rows(app: &mut DesignApp, ui: &mut Ui) {
    let Some(c) = app.ui.pressroom_spell.clone() else { return };
    if c.suggestions.is_empty() {
        ui.add_enabled(false, egui::Button::new("(no spelling suggestions)"));
    }
    for s in &c.suggestions {
        if ui.button(egui::RichText::new(s).strong()).clicked() {
            let _ = app.run("spelling.change", json!({"story": c.story, "start": c.start, "end": c.end, "to": s}));
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Ignore All").clicked() {
        app.ui.pressroom_ignored.push(c.word.to_lowercase());
        ui.close();
    }
    if ui.button("Add to Dictionary").clicked() {
        let _ = app.run("spelling.addWord", json!({"word": c.word}));
        ui.close();
    }
    ui.separator();
}

//! Find and Replace, as Publisher has it: a task pane with Find what, Replace with, and its two
//! options, not a page-layout program's dialog of text, pattern and object searches.

use egui::Ui;
use serde_json::{Value, json};

use super::taskpane::FIND;
use crate::DesignApp;

#[derive(Clone, Debug, Default)]
struct State {
    replace: bool,
    find: String,
    change: String,
    whole_word: bool,
    case: bool,
    status: String,
}

fn id() -> egui::Id {
    egui::Id::unique("pr_find")
}

/// Waits for the pane's next frame, which has the context to store it in.
static OPENING: std::sync::Mutex<Option<(bool, String)>> = std::sync::Mutex::new(None);

/// Opens the pane on Find or Replace, with the selected text ready to look for.
pub(super) fn open(app: &mut DesignApp, replace: bool) {
    let selected = crate::panels::text_attrs(app).and_then(|a| a["selectedText"].as_str().map(str::to_string)).unwrap_or_default();
    *OPENING.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((replace, selected));
    app.ui.pressroom_pane = Some(FIND.to_string());
}

fn params(s: &State) -> Value {
    json!({"find": s.find, "change": s.change, "caseSensitive": s.case, "wholeWord": s.whole_word, "scope": "document"})
}

pub(super) fn pane(app: &mut DesignApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut s: State = ctx.data(|d| d.get_temp(id())).unwrap_or_default();
    if let Some((replace, selected)) = OPENING.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
        s.replace = replace;
        s.status.clear();
        if !selected.is_empty() {
            s.find = selected;
        }
    }
    ui.horizontal(|ui| {
        if ui.radio(!s.replace, "Find").clicked() {
            s.replace = false;
        }
        if ui.radio(s.replace, "Replace").clicked() {
            s.replace = true;
        }
    });
    ui.add_space(6.0);
    ui.label("Find what:");
    ui.add(egui::TextEdit::singleline(&mut s.find).desired_width(f32::INFINITY));
    if s.replace {
        ui.add_space(4.0);
        ui.label("Replace with:");
        ui.add(egui::TextEdit::singleline(&mut s.change).desired_width(f32::INFINITY));
    }
    ui.add_space(8.0);
    let ready = !s.find.is_empty();
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(ready, egui::Button::new("Find Next")).clicked() {
            s.status = match app.run("find.next", params(&s)) {
                Ok(Value::Null) => "That text was not found.".to_string(),
                Ok(_) => String::new(),
                Err(e) => e,
            };
        }
        if s.replace {
            if ui.add_enabled(ready, egui::Button::new("Replace")).clicked() {
                let mut p = params(&s);
                p["first"] = json!(true);
                s.status = match app.run("find.change", p) {
                    Ok(_) => {
                        // On to the next one, as Publisher goes.
                        let _ = app.run("find.next", params(&s));
                        String::new()
                    }
                    Err(e) => e,
                };
            }
            if ui.add_enabled(ready, egui::Button::new("Replace All")).clicked() {
                s.status = match app.run("find.change", params(&s)) {
                    Ok(r) => match r["count"].as_u64().unwrap_or(0) {
                        0 => "That text was not found.".to_string(),
                        1 => "Replaced 1 time.".to_string(),
                        n => format!("Replaced {n} times."),
                    },
                    Err(e) => e,
                };
            }
        }
    });
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Find options").strong());
    ui.checkbox(&mut s.whole_word, "Match whole word only");
    ui.checkbox(&mut s.case, "Match case");
    if !s.status.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&s.status).color(super::pal().group_label));
    }
    ctx.data_mut(|d| d.insert_temp(id(), s));
}

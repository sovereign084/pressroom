//! Format Painter: pick up the formatting of the selected text or object, then paint it on the next
//! text or object clicked. A double-click on the button keeps it painting until Esc. Ctrl+Shift+C
//! and Ctrl+Shift+V do the same from the keyboard.

use serde_json::{Map, Value, json};

use crate::DesignApp;

/// The character formatting that travels with the painter.
const TEXT_ATTRS: &[&str] = &[
    "fontFamily",
    "fontStyle",
    "size",
    "fill",
    "fillTint",
    "underline",
    "strikethrough",
    "position",
    "capitalization",
    "tracking",
    "stroke",
    "strokeWeight",
    "leading",
];

#[derive(Clone, Debug, PartialEq)]
enum Picked {
    Text(Map<String, Value>),
    Object(u64),
}

#[derive(Default)]
struct State {
    picked: Option<Picked>,
    /// Waiting for the click to paint on.
    armed: bool,
    /// Keeps painting after a click (a double-click on the button).
    sticky: bool,
}

static STATE: std::sync::Mutex<State> = std::sync::Mutex::new(State { picked: None, armed: false, sticky: false });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether a click will paint: the ribbon lights the button.
pub(super) fn armed() -> bool {
    state().armed
}

/// Ctrl+Shift+C: picks up the selection's formatting.
pub(super) fn copy(app: &mut DesignApp) {
    let typing = app.session.active().is_some_and(|d| d.selection.text.is_some());
    let picked = if typing {
        crate::panels::text_attrs(app).map(|a| {
            let chars = a["chars"].as_object().cloned().unwrap_or_default();
            Picked::Text(chars.into_iter().filter(|(k, _)| TEXT_ATTRS.contains(&k.as_str())).collect())
        })
    } else {
        app.session.active().and_then(|d| d.selection.items.first()).map(|id| Picked::Object(id.0))
    };
    if picked.is_some() {
        state().picked = picked;
    }
}

/// Ctrl+Shift+V: paints what was picked up on the selection.
pub(super) fn paste(app: &mut DesignApp) {
    let picked = state().picked.clone();
    match picked {
        Some(Picked::Text(attrs)) => {
            // On an object that holds text, or on selected text.
            let _ = app.run("type.char", json!({"attrs": attrs}));
        }
        Some(Picked::Object(from)) => {
            let ids: Vec<u64> =
                app.session.active().map(|d| d.selection.items.iter().map(|i| i.0).filter(|i| *i != from).collect()).unwrap_or_default();
            if !ids.is_empty() {
                let _ = app.run("object.matchAttributes", json!({"from": from, "ids": ids}));
            }
        }
        None => {}
    }
}

/// The ribbon button: a click paints once, a double-click keeps painting.
pub(super) fn button(app: &mut DesignApp, double: bool) {
    let was = armed();
    if was && !double {
        cancel();
        return;
    }
    copy(app);
    let mut s = state();
    if s.picked.is_some() {
        s.armed = true;
        s.sticky = double;
    }
}

pub(super) fn cancel() {
    let mut s = state();
    s.armed = false;
    s.sticky = false;
}

/// Each frame: paints when a click on the page ends, and shows the painting cursor.
pub(super) fn frame(app: &mut DesignApp, ctx: &egui::Context) {
    if !armed() {
        return;
    }
    if app.ui.dialog.is_some() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel();
        return;
    }
    let over = ctx.pointer_hover_pos().is_some_and(|p| app.canvas_rect.is_some_and(|r| r.contains(p)));
    if !over {
        return;
    }
    ctx.set_cursor_icon(egui::CursorIcon::Copy);
    if !ctx.input(|i| i.pointer.primary_released()) {
        return;
    }
    // A click in text without a selection paints its word, as Publisher's does.
    let picked_text = matches!(state().picked, Some(Picked::Text(_)));
    let caret = app.session.active().and_then(|d| d.selection.text).filter(|t| t.is_caret());
    if let Some(t) = caret {
        let restore = t.range().start;
        if let Some((a, b)) = word_at(app, t.story, restore) {
            let _ = app.run("text.select", json!({"story": t.story.0, "anchor": a, "focus": b}));
            paste(app);
            let _ = app.run("text.select", json!({"story": t.story.0, "anchor": restore, "focus": restore}));
        }
    } else if !picked_text || app.session.active().is_some_and(|d| d.selection.text.is_some() || !d.selection.items.is_empty()) {
        paste(app);
    }
    if !state().sticky {
        cancel();
    }
}

/// The word around a place in a story's text, as byte offsets.
fn word_at(app: &DesignApp, story: designcraft_doc::StoryId, at: usize) -> Option<(usize, usize)> {
    let text = &app.session.active()?.doc.story(story)?.text;
    let is_word = |c: char| c.is_alphanumeric() || c == '\'' || c == '\u{2019}';
    let at = at.min(text.len());
    let head = text.get(..at)?;
    let tail = text.get(at..)?;
    let a = head.char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map_or(at, |(i, _)| i);
    let b = at + tail.char_indices().find(|(_, c)| !is_word(*c)).map_or(tail.len(), |(i, _)| i);
    (a < b).then_some((a, b))
}

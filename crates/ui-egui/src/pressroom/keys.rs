//! Publisher's keyboard shortcuts. They run before the keymap, take their keys, and leave the rest
//! (copy, cut, paste, Delete, arrows, Esc) to it. Single letters never switch tools, as they would
//! in a page-layout program: in PressRoom they only ever type.

use egui::{Key, Modifiers};
use serde_json::json;

use super::{Backstage, ribbon};
use crate::DesignApp;

const CTRL: Modifiers = Modifiers::COMMAND;
const CTRL_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::COMMAND };
const ALT: Modifiers = Modifiers::ALT;
const ALT_SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::ALT };

enum Do {
    Cmd(&'static str),
    Para(&'static str),
    SizeStep(f64),
    Tracking(f64),
    Position(&'static str),
    Backstage(Backstage),
    GroupToggle,
    InsertPage,
    DuplicatePage,
    MasterToggle,
    ZoomToggle,
    ClearChar,
    Spelling,
    SmallCaps,
    NoHyphen,
    Font,
    Hyperlink,
    Hyphenation,
    CopyFormat,
    PasteFormat,
    Find,
    Replace,
    Date,
    Time,
    PageNumber,
    DeleteObject,
    Page(bool),
}

/// The shortcut table, from Microsoft's list for Publisher. Each row wants exactly these modifiers,
/// so Ctrl+Shift+F is never taken for Ctrl+F.
const TABLE: &[(Modifiers, Key, Do)] = &[
    (CTRL_SHIFT, Key::Period, Do::Cmd("type.sizeUp")),
    (CTRL_SHIFT, Key::Comma, Do::Cmd("type.sizeDown")),
    (CTRL_SHIFT, Key::Equals, Do::Position("superscript")),
    (CTRL_SHIFT, Key::Plus, Do::Position("superscript")),
    (CTRL_SHIFT, Key::K, Do::SmallCaps),
    (CTRL_SHIFT, Key::D, Do::Para("fullyJustified")),
    (CTRL_SHIFT, Key::G, Do::GroupToggle),
    (CTRL_SHIFT, Key::N, Do::InsertPage),
    (CTRL_SHIFT, Key::U, Do::DuplicatePage),
    (CTRL_SHIFT, Key::Y, Do::Cmd("view.hiddenCharacters")),
    (CTRL_SHIFT, Key::L, Do::Cmd("view.fitPage")),
    (CTRL_SHIFT, Key::O, Do::Cmd("view.frameEdges")),
    (CTRL_SHIFT, Key::S, Do::Cmd("app.saveDialog")),
    (CTRL_SHIFT, Key::F, Do::Font),
    (CTRL_SHIFT, Key::H, Do::Hyphenation),
    (CTRL_SHIFT, Key::C, Do::CopyFormat),
    (CTRL_SHIFT, Key::V, Do::PasteFormat),
    (CTRL_SHIFT, Key::X, Do::DeleteObject),
    (CTRL_SHIFT, Key::Num0, Do::NoHyphen),
    (CTRL_SHIFT, Key::CloseBracket, Do::Tracking(10.0)),
    (CTRL_SHIFT, Key::CloseCurlyBracket, Do::Tracking(10.0)),
    (CTRL_SHIFT, Key::OpenBracket, Do::Tracking(-10.0)),
    (CTRL_SHIFT, Key::OpenCurlyBracket, Do::Tracking(-10.0)),
    (ALT_SHIFT, Key::F6, Do::Cmd("object.sendToBack")),
    (ALT_SHIFT, Key::T, Do::Time),
    (ALT_SHIFT, Key::D, Do::Date),
    (ALT_SHIFT, Key::P, Do::PageNumber),
    (ALT, Key::F6, Do::Cmd("object.bringToFront")),
    (ALT, Key::Backspace, Do::Cmd("edit.undo")),
    (CTRL, Key::B, Do::Cmd("type.bold")),
    (CTRL, Key::I, Do::Cmd("type.italic")),
    (CTRL, Key::U, Do::Cmd("type.underline")),
    (CTRL, Key::E, Do::Cmd("type.alignCenter")),
    (CTRL, Key::L, Do::Cmd("type.alignLeft")),
    (CTRL, Key::R, Do::Cmd("type.alignRight")),
    (CTRL, Key::J, Do::Cmd("type.justify")),
    (CTRL, Key::CloseBracket, Do::SizeStep(1.0)),
    (CTRL, Key::OpenBracket, Do::SizeStep(-1.0)),
    (CTRL, Key::Equals, Do::Position("subscript")),
    (CTRL, Key::Space, Do::ClearChar),
    (CTRL, Key::Z, Do::Cmd("edit.undo")),
    (CTRL, Key::Y, Do::Cmd("edit.redo")),
    (CTRL, Key::S, Do::Cmd("app.save")),
    (CTRL, Key::P, Do::Backstage(Backstage::Print)),
    (CTRL, Key::N, Do::Backstage(Backstage::New)),
    (CTRL, Key::O, Do::Backstage(Backstage::Open)),
    (CTRL, Key::F, Do::Find),
    (CTRL, Key::H, Do::Replace),
    (CTRL, Key::K, Do::Hyperlink),
    (CTRL, Key::A, Do::Cmd("edit.selectAll")),
    (CTRL, Key::M, Do::MasterToggle),
    (CTRL, Key::G, Do::Cmd("view.goToPage")),
    (CTRL, Key::W, Do::Cmd("file.close")),
    (CTRL, Key::F4, Do::Cmd("file.close")),
    (CTRL, Key::F7, Do::Cmd("view.baselineGrid")),
    (CTRL, Key::PageDown, Do::Page(true)),
    (CTRL, Key::PageUp, Do::Page(false)),
    (Modifiers::SHIFT, Key::F4, Do::Find),
    (Modifiers::NONE, Key::F3, Do::Find),
    (Modifiers::NONE, Key::F4, Do::Cmd("edit.redo")),
    (Modifiers::NONE, Key::F5, Do::Cmd("view.goToPage")),
    (Modifiers::NONE, Key::F7, Do::Spelling),
    (Modifiers::NONE, Key::F9, Do::ZoomToggle),
];

/// The inherited keymap's commands that PressRoom's window keeps. Everything else there is a
/// page-layout program's key that Publisher doesn't have (Ctrl+D Place, Ctrl+3 Hide, Tab Hide
/// Panels, the panel keys on F6, F8 and F10...).
pub const INHERITED_KEYS: &[&str] = &[
    "edit.copy",
    "edit.cut",
    "edit.paste",
    "edit.clear",
    "view.zoomIn",
    "view.zoomOut",
    "window.nextDocument",
    "window.previousDocument",
    "app.quit",
];

/// Letter and digit keys: typed, never tool shortcuts.
const PLAIN: &[Key] = &[
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
    Key::Num0,
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
    Key::Backslash,
    Key::Slash,
    Key::Minus,
    Key::Equals,
    Key::Period,
    Key::Comma,
    Key::OpenBracket,
    Key::CloseBracket,
];

/// Take and run this frame's Publisher shortcuts. Call before the keymap's own dispatch.
pub fn shortcuts(app: &mut DesignApp, ctx: &egui::Context) {
    if ctx.text_edit_focused() || app.ui.dialog.is_some() || app.ui.palette.is_some() || app.session.tool_composing() {
        return;
    }
    let typing = app.session.wants_text();
    let mut hits = Vec::new();
    let paste = ctx.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Key { key: Key::V, pressed: true, repeat: false, modifiers, .. } if modifiers.matches_exact(CTRL)))
    });
    ctx.input_mut(|i| {
        for (k, (mods, key, _)) in TABLE.iter().enumerate() {
            let exact = i.events.iter().any(
                |e| matches!(e, egui::Event::Key { key: pressed, pressed: true, modifiers, .. } if pressed == key && modifiers.matches_exact(*mods)),
            );
            if exact && i.consume_key(*mods, *key) {
                hits.push(k);
            }
        }
        if !typing {
            for key in PLAIN {
                for mods in [Modifiers::NONE, Modifiers::SHIFT] {
                    let _ = i.consume_shortcut(&egui::KeyboardShortcut::new(mods, *key));
                }
            }
        }
    });
    for k in hits {
        let Some((_, _, what)) = TABLE.get(k) else { continue };
        run(app, ctx, what);
    }
    if paste {
        super::paste_picture(app);
    }
}

fn run(app: &mut DesignApp, ctx: &egui::Context, what: &Do) {
    match what {
        Do::Cmd(id) => ribbon::act(app, id),
        Do::Para(align) => {
            let _ = app.run("type.para", json!({"attrs": {"align": align}}));
        }
        Do::SizeStep(d) => {
            let size = crate::panels::text_attrs(app).and_then(|a| a["chars"]["size"].as_f64()).unwrap_or(12.0);
            let _ = app.run("type.char", json!({"attrs": {"size": (size + d).max(1.0)}}));
        }
        Do::Position(p) => {
            let cur = crate::panels::text_attrs(app).and_then(|a| a["chars"]["position"].as_str().map(str::to_string)).unwrap_or_default();
            let pos = if cur == *p { "normal" } else { p };
            let _ = app.run("type.char", json!({"attrs": {"position": pos}}));
        }
        Do::Backstage(page) => super::open_backstage(ctx, *page),
        Do::GroupToggle => {
            if ribbon::on(app, "object.ungroup") {
                ribbon::act(app, "object.ungroup");
            } else {
                ribbon::act(app, "object.group");
            }
        }
        Do::InsertPage => {
            let cur = crate::canvas::current_page(app).unwrap_or(0);
            ribbon::act_with(app, "layout.pages.insert", json!({"count": 1, "after": cur}));
        }
        Do::MasterToggle => {
            let on = !super::master_view(app);
            super::set_master_view(app, on);
        }
        Do::ClearChar => {
            let _ = app.run("style.character.apply", json!({"name": "[None]"}));
        }
        Do::Tracking(d) => {
            let cur = crate::panels::text_attrs(app).and_then(|a| a["chars"]["tracking"].as_f64()).unwrap_or(0.0);
            let _ = app.run("type.char", json!({"attrs": {"tracking": cur + d}}));
        }
        Do::DuplicatePage => {
            let cur = crate::canvas::current_page(app).unwrap_or(0);
            super::duplicate_page(app, cur);
        }
        Do::SmallCaps => {
            let cur = crate::panels::text_attrs(app).and_then(|a| a["chars"]["capitalization"].as_str().map(str::to_string)).unwrap_or_default();
            let to = if cur == "smallCaps" { "normal" } else { "smallCaps" };
            let _ = app.run("type.char", json!({"attrs": {"capitalization": to}}));
        }
        Do::NoHyphen => {
            let cur = crate::panels::text_attrs(app).and_then(|a| a["chars"]["noBreak"].as_bool()).unwrap_or(false);
            let _ = app.run("type.char", json!({"attrs": {"noBreak": !cur}}));
        }
        Do::Font => super::dialogs::open_font(app),
        Do::Hyperlink => super::dialogs::open_hyperlink(app),
        Do::Hyphenation => super::dialogs::open_hyphenation(app),
        Do::Spelling => super::spelling::open(app),
        Do::CopyFormat => super::painter::copy(app),
        Do::PasteFormat => super::painter::paste(app),
        Do::Find => super::find::open(app, false),
        Do::Replace => super::find::open(app, true),
        Do::Date => super::insert_date(app),
        Do::Time => super::insert_time(app),
        Do::PageNumber => ribbon::act(app, "app.insertCurrentPageNumber"),
        Do::DeleteObject => {
            if app.session.active().is_some_and(|d| !d.selection.items.is_empty() && d.selection.text.is_none()) {
                ribbon::act(app, "edit.clear");
            }
        }
        Do::Page(next) => {
            let n = app.session.active().map(|d| d.doc.page_count()).unwrap_or(0);
            let cur = crate::canvas::current_page(app).unwrap_or(0);
            let to = if *next { (cur + 1).min(n.saturating_sub(1)) } else { cur.saturating_sub(1) };
            crate::canvas::go_to_page(app, to);
        }
        Do::ZoomToggle => {
            let z = app.view().map(|v| v.zoom).unwrap_or(1.0);
            if (z - 1.0).abs() < 0.01 {
                ribbon::act(app, "view.fitPage");
            } else {
                ribbon::act(app, "view.actualSize");
            }
        }
    }
}

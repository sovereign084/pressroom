//! Publisher's right-click menus. A page-layout program's rows (Paste Into, Transform, Fitting,
//! Story Editor…) don't belong in PressRoom's window, so these tables stand on their own and don't
//! import DesignCraft's menus.

use egui::Ui;
use serde_json::{Value, json};

use super::{dialogs, ribbon};
use crate::DesignApp;
use crate::menus::ContextMenu;

#[derive(Clone, Copy)]
pub(super) enum Row {
    /// A command by id, greyed when it can't run.
    Cmd(&'static str, &'static str),
    /// A command with parameters.
    With(&'static str, &'static str, &'static str),
    /// PressRoom's own action.
    Act(&'static str, fn(&mut DesignApp)),
    Sub(&'static str, &'static [Row]),
    Sep,
}

const ORDER: &[Row] = &[
    Row::Cmd("Bring to Front", "object.bringToFront"),
    Row::Cmd("Bring Forward", "object.bringForward"),
    Row::Cmd("Send Backward", "object.sendBackward"),
    Row::Cmd("Send to Back", "object.sendToBack"),
];

const ROTATE: &[Row] = &[
    Row::With("Rotate Right 90°", "transform.rotate", r#"{"angle": -90}"#),
    Row::With("Rotate Left 90°", "transform.rotate", r#"{"angle": 90}"#),
    Row::With("Flip Vertical", "transform.flip", r#"{"axis": "vertical"}"#),
    Row::With("Flip Horizontal", "transform.flip", r#"{"axis": "horizontal"}"#),
];

pub(super) const OBJECT: &[Row] = &[
    Row::Cmd("Cut", "edit.cut"),
    Row::Cmd("Copy", "edit.copy"),
    Row::Cmd("Paste", "edit.paste"),
    Row::Cmd("Delete Object", "edit.clear"),
    Row::Sep,
    Row::Act("Save as Picture…", save_as_picture),
    Row::Act("Change Picture…", change_picture),
    Row::Cmd("Group", "object.group"),
    Row::Cmd("Ungroup", "object.ungroup"),
    Row::Sub("Order", ORDER),
    Row::Sub("Rotate or Flip", ROTATE),
    Row::Sep,
    Row::Act("Format Object…", dialogs::open_format),
    Row::Act("Hyperlink…", dialogs::open_hyperlink),
];

pub(super) const TEXT: &[Row] = &[
    Row::Cmd("Cut", "edit.cut"),
    Row::Cmd("Copy", "edit.copy"),
    Row::Cmd("Paste", "edit.paste"),
    Row::Cmd("Keep Text Only", "edit.pasteWithoutFormatting"),
    Row::Sep,
    Row::Act("Font…", dialogs::open_font),
    Row::Act("Paragraph…", dialogs::open_paragraph),
    Row::Act("Bullets and Numbering…", bullets),
    Row::Sep,
    Row::Act("Hyperlink…", dialogs::open_hyperlink),
    Row::Act("Format Text Box…", format_text_box),
    Row::Sep,
    Row::Cmd("Select All", "edit.selectAll"),
];

const ZOOM: &[Row] = &[
    Row::Cmd("Whole Page", "view.fitPage"),
    Row::Act("Page Width", super::fit_page_width),
    Row::Cmd("100%", "view.actualSize"),
    Row::With("50%", "view.zoom", r#"{"zoom": 0.5}"#),
    Row::With("75%", "view.zoom", r#"{"zoom": 0.75}"#),
    Row::With("150%", "view.zoom", r#"{"zoom": 1.5}"#),
    Row::With("200%", "view.zoom", r#"{"zoom": 2.0}"#),
];

const GUIDES: &[Row] = &[
    Row::With("Add Horizontal Ruler Guide", "guide.add", r#"{"orientation": "horizontal", "position": 72}"#),
    Row::With("Add Vertical Ruler Guide", "guide.add", r#"{"orientation": "vertical", "position": 72}"#),
];

pub(super) const CANVAS: &[Row] = &[
    Row::Cmd("Paste", "edit.paste"),
    Row::Sep,
    Row::Sub("Zoom", ZOOM),
    Row::Sub("Ruler Guides", GUIDES),
    Row::Sep,
    Row::Cmd("Select All", "edit.selectAll"),
];

fn bullets(app: &mut DesignApp) {
    ribbon::act(app, "app.bulletsNumberingDialog");
}

fn format_text_box(app: &mut DesignApp) {
    let frame = super::selected_text_frame(app);
    if let Some(id) = frame {
        let _ = app.run("selection.set", json!({"ids": [id.0]}));
    }
    dialogs::open_format(app);
}

fn change_picture(app: &mut DesignApp) {
    super::insert_picture(app);
}

/// Saves the selected objects as a PNG: the page drawn at twice its size, cut to the selection.
fn save_as_picture(app: &mut DesignApp) {
    let Some((page, r)) = app.session.active().and_then(|st| {
        let first = *st.selection.items.first()?;
        let page = st.doc.page_of_item(first)?;
        let (si, pi) = st.doc.page_loc(page)?;
        let px = st.doc.spreads.get(si)?.pages.get(pi)?.x;
        let mut b: Option<designcraft_geom::Rect> = None;
        for id in &st.selection.items {
            let loc = st.doc.find(*id)?;
            let it = st.doc.item(*id)?;
            let on_spread = st.doc.parent_xf(&loc).transform_rect_bbox(it.visible_bounds());
            b = Some(b.map_or(on_spread, |r| r.union(on_spread)));
        }
        let b = b?;
        Some((page, designcraft_geom::Rect::new(b.x0 - px, b.y0, b.x1 - px, b.y1)))
    }) else {
        return;
    };
    let Some(st) = app.session.active() else { return };
    const SCALE: f64 = 2.0;
    let mut renderer = designcraft_render::Renderer::new();
    let options = designcraft_render::RenderOptions { printing_only: true, ..Default::default() };
    let Some(img) = renderer.render_page(&st.doc, &app.session.cache, page, SCALE, false, &options) else { return };
    let png = cropped(&img, r, SCALE).to_png();
    let name = format!("{} picture.png", st.doc.title);
    let Some(path) = app.pick_save(&name) else { return };
    let result = app.services.write.as_mut().map(|w| w(&path, &png));
    if let Some(Err(e)) = result {
        app.status(format!("Couldn't save the picture: {e}"));
    }
}

/// The part of a rendered page inside `r` (points on the page), kept inside the page.
fn cropped(img: &designcraft_render::Rendered, r: designcraft_geom::Rect, scale: f64) -> designcraft_render::Rendered {
    let (w, h) = (img.width as usize, img.height as usize);
    let x0 = ((r.x0 * scale).floor().max(0.0) as usize).min(w);
    let y0 = ((r.y0 * scale).floor().max(0.0) as usize).min(h);
    let x1 = ((r.x1 * scale).ceil().max(0.0) as usize).clamp(x0, w);
    let y1 = ((r.y1 * scale).ceil().max(0.0) as usize).clamp(y0, h);
    let mut pixels = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
    for y in y0..y1 {
        if let Some(row) = img.pixels.get(y * w * 4 + x0 * 4..y * w * 4 + x1 * 4) {
            pixels.extend_from_slice(row);
        }
    }
    designcraft_render::Rendered { width: (x1 - x0) as u32, height: (y1 - y0) as u32, pixels }
}

fn enabled(app: &DesignApp, row: &Row) -> bool {
    match row {
        Row::Cmd(_, id) | Row::With(_, id, _) => crate::menus::menu_enabled(app, id),
        Row::Act(label, _) => {
            let has_picture = || {
                app.session.active().is_some_and(|st| {
                    st.selection
                        .items
                        .first()
                        .and_then(|id| st.doc.item(*id))
                        .is_some_and(|it| matches!(it.content, designcraft_doc::Content::Graphic(_) | designcraft_doc::Content::Unassigned))
                })
            };
            *label != "Change Picture…" || has_picture()
        }
        _ => true,
    }
}

fn rows(app: &mut DesignApp, ui: &mut Ui, table: &'static [Row]) {
    for row in table {
        match *row {
            Row::Sep => {
                ui.separator();
            }
            Row::Sub(label, sub) => {
                ui.menu_button(label, |ui| rows(app, ui, sub));
            }
            Row::Cmd(label, id) => {
                if ui.add_enabled(enabled(app, row), egui::Button::new(label)).clicked() {
                    ribbon::act(app, id);
                    ui.close();
                }
            }
            Row::With(label, id, params) => {
                if ui.add_enabled(enabled(app, row), egui::Button::new(label)).clicked() {
                    ribbon::act_with(app, id, serde_json::from_str::<Value>(params).unwrap_or(Value::Null));
                    ui.close();
                }
            }
            Row::Act(label, act) => {
                if ui.add_enabled(enabled(app, row), egui::Button::new(label)).clicked() {
                    act(app);
                    ui.close();
                }
            }
        }
    }
}

/// The menu a right-click opens on the page.
pub(crate) fn context(app: &mut DesignApp, ui: &mut Ui, kind: ContextMenu) {
    ui.set_min_width(210.0);
    match kind {
        ContextMenu::Text => {
            super::spelling::menu_rows(app, ui);
            rows(app, ui, TEXT);
        }
        ContextMenu::Object => rows(app, ui, OBJECT),
        ContextMenu::Canvas => rows(app, ui, CANVAS),
    }
}

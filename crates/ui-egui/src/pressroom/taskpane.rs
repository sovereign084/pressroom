//! Publisher's task pane: a pane at the window's right. PressRoom's window has no dock, so the
//! inherited panels a command opens (Hyperlinks, Mail Merge, Design Checker…) show here, and the
//! ones Publisher has as dialogs (Font, Paragraph) open as those.

use egui::{FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::Value;

use super::PR;
use crate::DesignApp;

const WIDTH: f32 = 290.0;
/// The Find and Replace pane, PressRoom's own.
pub(super) const FIND: &str = "pr.find";

/// The pane's title: Publisher's name for the feature where it has one.
fn title(id: &str) -> &'static str {
    match id {
        FIND => "Find and Replace",
        "preflight" => "Design Checker",
        "dataMerge" => "Mail Merge",
        "glyphs" => "Symbols",
        "links" => "Graphics Manager",
        "table" => "Table Options",
        "hyperlinks" => "Hyperlinks",
        "bookmarks" => "Bookmarks",
        "typography" => "Typography",
        "paragraphStyles" | "characterStyles" => "Styles",
        "swatches" => "Colors",
        _ => crate::dock::panel_label(id),
    }
}

/// Opens `id` in the pane, or as its dialog.
pub(super) fn open(app: &mut DesignApp, id: &str) {
    match id {
        "character" => super::dialogs::open_font(app),
        "paragraph" => super::dialogs::open_paragraph(app),
        _ => app.ui.pressroom_pane = Some(id.to_string()),
    }
}

/// The commands that would show a dock panel. None for every other command.
pub(crate) fn command(app: &mut DesignApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    let panel = p.get("panel").and_then(Value::as_str);
    match id {
        "window.panel" | "window.panel.activate" => open(app, panel.unwrap_or("properties")),
        "app.tablePanel" => open(app, "table"),
        "window.panel.close" => {
            if panel.is_none_or(|p| app.ui.pressroom_pane.as_deref() == Some(p)) {
                app.ui.pressroom_pane = None;
            }
        }
        _ => return None,
    }
    Some(Ok(Value::Null))
}

pub(super) fn show(app: &mut DesignApp, ui: &mut Ui) {
    let Some(id) = app.ui.pressroom_pane.clone() else { return };
    egui::Panel::right("pr_task_pane")
        .exact_size(WIDTH)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(super::pal().pane).stroke(Stroke::new(1.0, super::pal().ribbon_edge)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            let head = Rect::from_min_size(full.min, vec2(full.width(), 36.0));
            ui.painter().text(
                head.left_center() + vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                title(&id),
                FontId::proportional(15.0),
                super::pal().text,
            );
            let x = Rect::from_min_size(pos2(head.max.x - 30.0, head.center().y - 11.0), vec2(22.0, 22.0));
            let resp = ui.interact(x, ui.scope_id().with("pr_pane_close"), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(x, PR::ROUND, super::pal().hover);
            }
            let c = x.center();
            let stroke = Stroke::new(1.3, super::pal().text);
            ui.painter().line_segment([c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)], stroke);
            ui.painter().line_segment([c + vec2(4.5, -4.5), c + vec2(-4.5, 4.5)], stroke);
            if resp.on_hover_text("Close").clicked() {
                app.ui.pressroom_pane = None;
            }
            ui.advance_cursor_after_rect(head);
            egui::ScrollArea::vertical().id_salt("pr_pane_scroll").auto_shrink([false, false]).show(ui, |ui| {
                egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                    if id == FIND {
                        super::find::pane(app, ui);
                    } else {
                        crate::dock::panel_body(app, ui, &id);
                    }
                });
            });
        });
}

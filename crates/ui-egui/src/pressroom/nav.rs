//! The Page Navigation pane: a column of page thumbnails (facing pages side by side), the page
//! number under each, the page in view highlighted; click to go there, right-click for the page
//! commands.

use egui::{Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;

use super::{PR, ribbon};
use crate::DesignApp;
use crate::panels::pages::{ThumbOf, thumb};

const WIDE: f32 = 132.0;
const NARROW: f32 = 34.0;
/// Thumbnail height.
const THUMB_H: f32 = 78.0;

pub(super) fn pane(app: &mut DesignApp, ui: &mut Ui, narrow: bool) {
    egui::Panel::left("pr_nav")
        .exact_size(if narrow { NARROW } else { WIDE })
        .resizable(false)
        .frame(egui::Frame::NONE.fill(PR::PANE).stroke(Stroke::new(1.0, PR::RIBBON_EDGE)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            // Header: "Page Navigation" and the collapse chevron.
            let head = Rect::from_min_size(full.min, vec2(full.width(), 26.0));
            if !narrow {
                ui.painter().text(head.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, "Pages", FontId::proportional(12.5), PR::TEXT);
            }
            let cr = Rect::from_min_size(pos2(head.max.x - 24.0, head.min.y + 5.0), vec2(18.0, 16.0));
            let resp = ui.interact(cr, egui::Id::new("pr_nav_collapse"), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(cr, 0.0, PR::HOVER);
            }
            let c = cr.center();
            let d = if narrow { 1.0 } else { -1.0 };
            for off in [-2.5, 2.5] {
                ui.painter().line_segment([c + vec2(off + 2.0 * d, -4.0), c + vec2(off - 2.0 * d, 0.0)], Stroke::new(1.2, PR::GROUP_LABEL));
                ui.painter().line_segment([c + vec2(off - 2.0 * d, 0.0), c + vec2(off + 2.0 * d, 4.0)], Stroke::new(1.2, PR::GROUP_LABEL));
            }
            if resp.on_hover_text(if narrow { "Expand Page Navigation Pane" } else { "Collapse Page Navigation Pane" }).clicked() {
                super::update(ui.ctx(), |s| s.nav_narrow = !narrow);
            }
            ui.advance_cursor_after_rect(head);
            if narrow {
                return;
            }
            let ctx = ui.ctx().clone();
            let cur = crate::canvas::current_page(app).unwrap_or(0);
            let spreads: Vec<Vec<(usize, f64, f64)>> = match app.session.active() {
                Some(st) => {
                    let mut abs = 0usize;
                    st.doc
                        .spreads
                        .iter()
                        .map(|s| {
                            s.pages
                                .iter()
                                .map(|p| {
                                    abs += 1;
                                    (abs - 1, p.width, p.height)
                                })
                                .collect()
                        })
                        .collect()
                }
                None => return,
            };
            let labels: Vec<String> = (0..spreads.iter().map(Vec::len).sum()).map(|i| app.session.page_label(i)).collect();
            egui::ScrollArea::vertical().id_salt("pr_nav_scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(6.0);
                for pages in spreads {
                    let widths: Vec<f32> = pages
                        .iter()
                        .map(|&(_, w, h)| (THUMB_H * (w / h.max(1.0)) as f32).min(54.0 * if pages.len() > 1 { 1.0 } else { 2.0 }))
                        .collect();
                    let row_w: f32 = widths.iter().sum();
                    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), THUMB_H + 26.0), Sense::hover());
                    let mut x = row.center().x - row_w / 2.0;
                    for (k, &(abs, w, h)) in pages.iter().enumerate() {
                        let tw = widths.get(k).copied().unwrap_or(40.0);
                        let th = (tw / (w / h.max(1.0)) as f32).min(THUMB_H);
                        let r = Rect::from_min_size(pos2(x, row.min.y + 4.0 + (THUMB_H - th)), vec2(tw, th));
                        x += tw;
                        let id = egui::Id::new(("pr_nav_page", abs));
                        let resp = ui.interact(r.expand(3.0), id, Sense::click());
                        let selected = abs == cur;
                        if selected {
                            ui.painter().rect_filled(r.expand(4.0), 0.0, Color32::from_rgb(253, 231, 196));
                            ui.painter().rect_stroke(r.expand(4.0), 0.0, Stroke::new(1.5, PR::SELECT), StrokeKind::Inside);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r.expand(4.0), 0.0, PR::HOVER);
                        }
                        ui.painter().rect_filled(r.translate(vec2(2.0, 2.0)), 0.0, Color32::from_black_alpha(40));
                        match thumb(app, &ctx, ThumbOf::Page(abs), th) {
                            Some(tex) => {
                                ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            }
                            None => {
                                ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                            }
                        }
                        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, Color32::from_gray(150)), StrokeKind::Outside);
                        let label = labels.get(abs).cloned().unwrap_or_else(|| (abs + 1).to_string());
                        ui.painter().text(
                            pos2(r.center().x, row.max.y - 10.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(11.5),
                            PR::TEXT,
                        );
                        if resp.clicked() {
                            crate::canvas::go_to_page(app, abs);
                        }
                        if resp.double_clicked() {
                            ribbon::act(app, "view.fitPage");
                        }
                        resp.context_menu(|ui| page_menu(app, ui, abs));
                    }
                    ui.add_space(4.0);
                }
            });
        });
}

/// Right-click on a page thumbnail: Publisher's page commands.
fn page_menu(app: &mut DesignApp, ui: &mut Ui, abs: usize) {
    let n = app.session.active().map(|d| d.doc.page_count()).unwrap_or(1);
    if ui.button("Insert Page").clicked() {
        ribbon::act_with(app, "layout.pages.insert", json!({"count": 1, "after": abs}));
        ui.close();
    }
    if ui.button("Insert Duplicate Page").clicked() {
        super::duplicate_page(app, abs);
        ui.close();
    }
    if ui.add_enabled(n > 1, egui::Button::new("Delete")).clicked() {
        super::delete_page(app, abs);
        ui.close();
    }
    if ui.add_enabled(n > 1, egui::Button::new("Move…")).clicked() {
        ribbon::act(app, "layout.pages.move");
        ui.close();
    }
    if ui.button("Rename…").clicked() {
        ribbon::act_with(app, "layout.section", json!({"page": abs + 1}));
        ui.close();
    }
    ui.separator();
    if ui.button("View Two-Page Spread").clicked() {
        let ctx = ui.ctx().clone();
        let on = !super::two_page(&ctx);
        super::set_two_page(app, &ctx, on);
        ui.close();
    }
}

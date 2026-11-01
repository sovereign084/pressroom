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
/// The thumbnails' corner rounding, as Windows 11 rounds its previews.
const THUMB_ROUND: u8 = 3;

/// The page being dragged in the pane.
#[derive(Clone, Copy)]
struct DragPage(usize);

pub(super) fn pane(app: &mut DesignApp, ui: &mut Ui, narrow: bool) {
    egui::Panel::left("pr_nav")
        .exact_size(if narrow { NARROW } else { WIDE })
        .resizable(false)
        .frame(egui::Frame::NONE.fill(super::pal().pane).stroke(Stroke::new(1.0, super::pal().ribbon_edge)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            // Header: "Page Navigation" and the collapse chevron.
            let head = Rect::from_min_size(full.min, vec2(full.width(), 36.0));
            if !narrow {
                ui.painter().text(
                    head.left_center() + vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Pages",
                    FontId::proportional(16.0),
                    super::pal().text,
                );
            }
            let cr = Rect::from_min_size(pos2(head.max.x - 26.0, head.center().y - 9.0), vec2(20.0, 18.0));
            let resp = ui.interact(cr, egui::Id::unique("pr_nav_collapse"), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(cr, PR::ROUND, super::pal().hover);
            }
            let c = cr.center();
            let d = if narrow { 1.0 } else { -1.0 };
            for off in [-2.5, 2.5] {
                ui.painter().line_segment([c + vec2(off + 2.0 * d, -4.0), c + vec2(off - 2.0 * d, 0.0)], Stroke::new(1.2, super::pal().group_label));
                ui.painter().line_segment([c + vec2(off - 2.0 * d, 0.0), c + vec2(off + 2.0 * d, 4.0)], Stroke::new(1.2, super::pal().group_label));
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
            let mut moved = None;
            egui::ScrollArea::vertical().id_salt("pr_nav_scroll").auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(6.0);
                for pages in spreads {
                    let widths: Vec<f32> = pages
                        .iter()
                        .map(|&(_, w, h)| (THUMB_H * (w / h.max(1.0)) as f32).min(54.0 * if pages.len() > 1 { 1.0 } else { 2.0 }))
                        .collect();
                    let row_w: f32 = widths.iter().sum();
                    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), THUMB_H + 26.0), Sense::hover());
                    // The spread's pages, bottom-aligned side by side and centred in the row.
                    let mut x = row.center().x - row_w / 2.0;
                    let mut sheets = Vec::with_capacity(pages.len());
                    for (k, &(abs, w, h)) in pages.iter().enumerate() {
                        let tw = widths.get(k).copied().unwrap_or(40.0);
                        let th = (tw / (w / h.max(1.0)) as f32).min(THUMB_H);
                        sheets.push((abs, th, Rect::from_min_size(pos2(x, row.min.y + 4.0 + (THUMB_H - th)), vec2(tw, th))));
                        x += tw;
                    }
                    let Some(spread) = sheets.iter().map(|s| s.2).reduce(|a, b| a.union(b)) else {
                        continue;
                    };
                    // Hovering lifts the spread a little off the pane, its shadow growing with it.
                    let id = egui::Id::unique(("pr_nav_spread", sheets.first().map_or(0, |s| s.0)));
                    let lift = ui.ctx().animate_bool_with_time(id, ui.rect_contains_pointer(spread.expand(3.0)), 0.12);
                    let up = vec2(0.0, -2.0 * lift);
                    let has_cur = sheets.iter().any(|s| s.0 == cur);
                    if has_cur {
                        // The page shown: a rounded frame in the accent colour on its light tint.
                        for &(abs, _, r) in &sheets {
                            if abs == cur {
                                ui.painter().rect_filled(r.expand(5.0), 6.0, super::pal().checked);
                                ui.painter().rect_stroke(r.expand(5.0), 6.0, Stroke::new(2.0, super::pal().select), StrokeKind::Inside);
                            }
                        }
                    } else if lift > 0.0 {
                        ui.painter().rect_filled(spread.expand(5.0), 6.0, super::pal().hover.gamma_multiply(lift));
                    }
                    let dark = super::pal().dark;
                    let shadow = egui::epaint::Shadow {
                        offset: [0, (1.0 + 3.0 * lift).round() as i8],
                        blur: (4.0 + 8.0 * lift).round() as u8,
                        spread: 0,
                        color: Color32::from_black_alpha(if dark { 110 } else { 40 } + (30.0 * lift) as u8),
                    };
                    ui.painter().add(shadow.as_shape(spread.translate(up), THUMB_ROUND));
                    let n = sheets.len();
                    for (k, &(abs, th, base)) in sheets.iter().enumerate() {
                        let resp = ui.interact(base.expand(3.0), egui::Id::unique(("pr_nav_page", abs)), Sense::click_and_drag());
                        resp.dnd_set_drag_payload(DragPage(abs));
                        // A page dragged over: the line shows where it lands, before or after this
                        // one, across the near edge of the thumbnail.
                        if resp.dnd_hover_payload::<DragPage>().is_some_and(|d| d.0 != abs) {
                            let after = ui
                                .input(|i| i.pointer.hover_pos())
                                .is_some_and(|p| if n > 1 { p.x > base.center().x } else { p.y > base.center().y });
                            let line = match (n > 1, after) {
                                (true, false) => [base.left_top() - vec2(4.0, 2.0), base.left_bottom() + vec2(-4.0, 2.0)],
                                (true, true) => [base.right_top() + vec2(4.0, -2.0), base.right_bottom() + vec2(4.0, 2.0)],
                                (false, false) => [base.left_top() - vec2(6.0, 5.0), base.right_top() + vec2(6.0, -5.0)],
                                (false, true) => [base.left_bottom() + vec2(-6.0, 5.0), base.right_bottom() + vec2(6.0, 5.0)],
                            };
                            ui.painter().line_segment(line, Stroke::new(3.0, super::pal().select));
                        }
                        if let Some(d) = resp.dnd_release_payload::<DragPage>()
                            && d.0 != abs
                        {
                            let after = ui
                                .input(|i| i.pointer.interact_pos())
                                .is_some_and(|p| if n > 1 { p.x > base.center().x } else { p.y > base.center().y });
                            moved = Some((d.0, super::dialogs::move_target(d.0, abs, after)));
                        }
                        let r = base.translate(up);
                        // Facing pages read as one sheet: round only the spread's outer corners.
                        let (first, last) = (k == 0, k + 1 == n);
                        let round = egui::CornerRadius {
                            nw: if first { THUMB_ROUND } else { 0 },
                            sw: if first { THUMB_ROUND } else { 0 },
                            ne: if last { THUMB_ROUND } else { 0 },
                            se: if last { THUMB_ROUND } else { 0 },
                        };
                        let page = match thumb(app, &ctx, ThumbOf::Page(abs), th) {
                            Some(tex) => egui::epaint::RectShape::filled(r, round, Color32::WHITE)
                                .with_texture(tex.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))),
                            None => egui::epaint::RectShape::filled(r, round, Color32::WHITE),
                        };
                        ui.painter().add(page);
                        ui.painter().rect_stroke(
                            r,
                            round,
                            Stroke::new(1.0, Color32::from_black_alpha(if dark { 90 } else { 38 })),
                            StrokeKind::Outside,
                        );
                        let label = labels.get(abs).cloned().unwrap_or_else(|| (abs + 1).to_string());
                        ui.painter().text(
                            pos2(base.center().x, row.max.y - 10.0),
                            egui::Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(11.5),
                            super::pal().text,
                        );
                        let page_title = app.session.active().and_then(|d| d.doc.page(abs).and_then(|p| p.title.clone()));
                        let resp = match page_title {
                            Some(t) => resp.on_hover_text(t),
                            None => resp,
                        };
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
            if let Some((from, to)) = moved
                && from != to
            {
                ribbon::act_with(app, "layout.pages.move", json!({"from": from, "to": to}));
            }
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
        super::dialogs::open_move(app, abs);
        ui.close();
    }
    if ui.button("Rename…").clicked() {
        super::dialogs::open_rename(app, abs);
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

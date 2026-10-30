//! The start-up splash: as PressRoom opens, a small card with its emblem and name covers the window
//! while the publication loads behind it, then fades away to show it, the way Office apps open.

use egui::{Color32, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::{PR, pal};
use crate::DesignApp;

/// The splash shows at least this long (seconds), so it never just flickers…
const MIN: f64 = 0.6;
/// …and at most this long, whatever is still loading (a Publisher file takes a minute).
const MAX: f64 = 3.0;
/// Its fade into the window.
const FADE: f64 = 0.25;

/// Paint the splash over the window while `app.splash` is on; it turns itself off once faded.
pub(super) fn show(app: &mut DesignApp, ctx: &egui::Context) {
    if !app.splash || cfg!(test) {
        return;
    }
    let id = egui::Id::unique("pr_splash");
    let now = ctx.input(|i| i.time);
    let (start, ready_at) = ctx.data(|d| d.get_temp::<(f64, Option<f64>)>(id)).unwrap_or((now, None));
    let shown = now - start;
    // Ready: the files it was opened with are open and drawn on the page.
    let loaded = app.os_documents.is_empty() && (app.session.active().is_none() || app.canvas.shown.is_some());
    let ready_at = ready_at.or_else(|| ((shown >= MIN && loaded) || shown >= MAX).then_some(now));
    let alpha = ready_at.map_or(1.0, |r| 1.0 - ((now - r) / FADE) as f32);
    if alpha <= 0.0 {
        app.splash = false;
        ctx.data_mut(|d| d.remove::<(f64, Option<f64>)>(id));
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, (start, ready_at)));
    ctx.request_repaint();
    let screen = ctx.content_rect();
    egui::Area::new(id).order(egui::Order::Tooltip).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        ui.multiply_opacity(alpha.clamp(0.0, 1.0));
        // The whole window, so nothing behind takes a click meanwhile.
        let (_, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
        let p = pal();
        ui.painter().rect_filled(screen, 0.0, p.top);
        // The card grows a touch as it fades, as if coming forward into the window.
        let grow = 1.0 + 0.03 * (1.0 - alpha);
        let card = Rect::from_center_size(screen.center(), vec2(380.0, 230.0) * grow);
        let shadow = egui::epaint::Shadow { offset: [0, 10], blur: 36, spread: 0, color: Color32::from_black_alpha(if p.dark { 140 } else { 45 }) };
        ui.painter().add(shadow.as_shape(card, 12.0));
        ui.painter().rect(card, 12.0, p.card, Stroke::new(1.0, p.card_edge), StrokeKind::Inside);
        let accent = if p.dark { p.top_accent } else { PR::TEAL };
        let mark = Rect::from_center_size(pos2(card.center().x, card.min.y + 64.0), vec2(72.0, 72.0));
        crate::about::paint_mark(ui, mark, accent);
        ui.painter().text(pos2(card.center().x, card.min.y + 130.0), egui::Align2::CENTER_CENTER, "PressRoom", FontId::proportional(30.0), p.text);
        ui.painter().text(
            pos2(card.center().x, card.min.y + 166.0),
            egui::Align2::CENTER_CENTER,
            "Starting\u{2026}",
            FontId::proportional(12.0),
            p.group_label,
        );
        // Dots that drift across, slow in the middle, while it loads.
        let y = card.min.y + 196.0;
        let half = card.width() * 0.32;
        for i in 0..5 {
            let t = (shown * 0.55 - f64::from(i) * 0.075).rem_euclid(1.0);
            let x = card.center().x + half * (2.0 * t as f32 - 1.0).powi(3);
            let fade = (std::f64::consts::PI * t).sin() as f32;
            ui.painter().circle_filled(pos2(x, y), 2.6, accent.gamma_multiply(fade));
        }
    });
}

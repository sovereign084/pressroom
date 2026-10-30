//! PressRoom's zoom glide: when a zoom button, the zoom slider, a shortcut or Fit Page changes the
//! zoom, the page eases to its new size and place over a fifth of a second instead of jumping, as
//! Office's does. The wheel and a pinch zoom as before (they move a little at a time already), and
//! going to another page at the same zoom still jumps.
//!
//! While it glides, the canvas shows (and takes clicks on) the view part of the way there; the
//! view the command chose is put back after each frame, so everything else sees it at once.

use super::*;

/// How long a glide takes, in seconds.
const GLIDE: f64 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Glide {
    from: View,
    to: View,
    start: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    /// The document the views belong to (each has its own).
    doc: u64,
    /// The view the canvas showed last frame.
    shown: Option<View>,
    glide: Option<Glide>,
}

/// The view the command chose and the one shown this frame instead.
pub(super) struct Gliding {
    target: View,
    shown: View,
}

fn state_id(app: &DesignApp) -> egui::Id {
    egui::Id::unique(("pr_zoom_glide", app.pane))
}

/// Before the canvas: a zoom change since the last frame starts a glide, and while one runs the
/// canvas is given the view on its way.
pub(super) fn begin(app: &mut DesignApp, ctx: &egui::Context) -> Option<Gliding> {
    if !app.ui.pressroom_shell || app.ui.screen_mode == ScreenMode::Presentation {
        return None;
    }
    let rect = app.canvas_rect?;
    let doc = app.session.active()?.uid;
    let target = *app.view()?;
    let now = ctx.input(|i| i.time);
    let id = state_id(app);
    let mut st = ctx.data(|d| d.get_temp::<State>(id)).unwrap_or_default();
    if st.doc != doc {
        st = State { doc, ..State::default() };
    }
    if let Some(shown) = st.shown
        && shown != target
        && st.glide.is_none_or(|g| g.to != target)
    {
        st.glide = worth_gliding(&shown, &target, rect).then_some(Glide { from: shown, to: target, start: now });
    }
    let t = st.glide.map(|g| (now - g.start) / GLIDE);
    let gliding = match (st.glide, t) {
        (Some(g), Some(t)) if t < 1.0 => {
            let shown = between(&g.from, &g.to, rect, ease(t));
            if let Some(v) = app.view_mut() {
                *v = shown;
            }
            ctx.request_repaint();
            Some(Gliding { target, shown })
        }
        _ => {
            st.glide = None;
            None
        }
    };
    ctx.data_mut(|d| d.insert_temp(id, st));
    gliding
}

/// After the canvas: remember what it showed and give the view back its target. Scrolling or
/// zooming during a glide (the canvas moved the view itself) ends it where the user put it.
pub(super) fn end(app: &mut DesignApp, ctx: &egui::Context, gliding: Option<Gliding>) {
    if !app.ui.pressroom_shell {
        return;
    }
    let Some(cur) = app.view().copied() else { return };
    let id = state_id(app);
    let mut st = ctx.data(|d| d.get_temp::<State>(id)).unwrap_or_default();
    match gliding {
        Some(g) if cur == g.shown => {
            st.shown = Some(g.shown);
            if let Some(v) = app.view_mut() {
                *v = g.target;
            }
        }
        _ => {
            st.glide = None;
            st.shown = Some(cur);
        }
    }
    if let Some(doc) = app.session.active().map(|d| d.uid) {
        st.doc = doc;
    }
    ctx.data_mut(|d| d.insert_temp(id, st));
}

/// A zoom (not a scroll at the same zoom, nor a jump far across the publication).
fn worth_gliding(a: &View, b: &View, rect: Rect) -> bool {
    if a.rotation != b.rotation || !(a.zoom > 0.0 && b.zoom > 0.0) || (b.zoom / a.zoom).ln().abs() < 0.01 {
        return false;
    }
    let (ca, cb) = (centre(a, rect), centre(b, rect));
    let far = 3.0 * f64::from(rect.width().max(rect.height()));
    (cb - ca).hypot() * a.zoom.min(b.zoom) <= far
}

/// The canvas point at the middle of the view.
fn centre(v: &View, rect: Rect) -> Point {
    let z = v.zoom.max(1e-9);
    Point::new(v.origin.x + f64::from(rect.width()) / 2.0 / z, v.origin.y + f64::from(rect.height()) / 2.0 / z)
}

/// The view a share `t` of the way from `a` to `b`: the middle moves evenly and the zoom by equal
/// factors, so the page grows smoothly.
fn between(a: &View, b: &View, rect: Rect, t: f64) -> View {
    let (ca, cb) = (centre(a, rect), centre(b, rect));
    let zoom = (a.zoom.ln() + (b.zoom.ln() - a.zoom.ln()) * t).exp();
    let c = Point::new(ca.x + (cb.x - ca.x) * t, ca.y + (cb.y - ca.y) * t);
    let origin = Point::new(c.x - f64::from(rect.width()) / 2.0 / zoom, c.y - f64::from(rect.height()) / 2.0 / zoom);
    View { zoom, origin, ..*b }
}

fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(zoom: f64, x: f64, y: f64) -> View {
        View { zoom, origin: Point::new(x, y), fitted: false, rotation: 0 }
    }

    #[test]
    fn a_zoom_glides_through_views_in_between_and_a_scroll_does_not() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let (a, b) = (view(0.5, 0.0, 0.0), view(1.0, 200.0, 150.0));
        assert!(worth_gliding(&a, &b, rect));
        // A scroll at the same zoom, or a jump to a far page, doesn't glide.
        assert!(!worth_gliding(&a, &view(0.5, 300.0, 0.0), rect));
        assert!(!worth_gliding(&a, &view(1.0, 20_000.0, 0.0), rect));
        assert_eq!(between(&a, &b, rect, 0.0), a);
        let end = between(&a, &b, rect, 1.0);
        assert!((end.zoom - 1.0).abs() < 1e-9 && (end.origin - b.origin).hypot() < 1e-6, "{end:?}");
        // Halfway, the zoom is the geometric middle and the page's middle point is halfway there.
        let mid = between(&a, &b, rect, 0.5);
        assert!((mid.zoom - 0.5_f64.sqrt()).abs() < 1e-9, "{mid:?}");
        let c = centre(&mid, rect);
        let (ca, cb) = (centre(&a, rect), centre(&b, rect));
        assert!((c.x - (ca.x + cb.x) / 2.0).abs() < 1e-6 && (c.y - (ca.y + cb.y) / 2.0).abs() < 1e-6);
        assert!(ease(0.0) == 0.0 && ease(1.0) == 1.0 && ease(0.5) > 0.5);
    }
}

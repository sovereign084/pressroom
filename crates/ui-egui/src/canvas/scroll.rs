#[cfg(test)]
use super::view::set_zoom;
use super::*;

/// Shortest scrollbar thumb.
const MIN_THUMB: f32 = 18.0;

/// How long egui (0.37 `WheelState`) keeps a scroll action's latched modifiers after the last
/// wheel event: 150 ms, or a second while fingers rest on a trackpad. The Alt latch below ends
/// with the action, exactly as egui ends Ctrl/Cmd, so the rest of an ended motion scrolls for
/// both keys and a later plain scroll is never taken for a zoom.
const SCROLL_ACTION_TIMEOUT: f64 = 0.150;

const RESTING_FINGER_TIMEOUT: f64 = 1.0;

/// Whether the wheel motion now arriving was made with Alt down. As egui does for Ctrl/Cmd, a
/// motion keeps its key to its end: the wheel is smoothed over a few frames, and a trackpad swipe
/// coasts on after the key is let go.
pub(super) fn wheel_alt(ui: &egui::Ui) -> bool {
    let id = egui::Id::unique("canvas_wheel_alt");
    // Alt, whether a trackpad swipe is under way, and the time of the last wheel event.
    let (mut alt, mut swipe, mut last): (bool, bool, f64) = ui.data(|d| d.get_temp(id)).unwrap_or((false, false, f64::NEG_INFINITY));
    ui.input(|i| {
        for e in &i.events {
            let egui::Event::MouseWheel { phase, modifiers, source, .. } = e else { continue };
            // egui ends the action (and drops its latched modifiers) when a new event arrives
            // after this pause; the Alt latch ends there too.
            let timeout = if swipe && *source == egui::MouseWheelSource::Trackpad { RESTING_FINGER_TIMEOUT } else { SCROLL_ACTION_TIMEOUT };
            if timeout < i.time - last {
                alt = false;
                swipe = false;
            }
            last = i.time;
            (alt, swipe) = match phase {
                egui::TouchPhase::Start => (modifiers.alt, true),
                egui::TouchPhase::Move => (modifiers.alt || (swipe && alt), swipe),
                egui::TouchPhase::End | egui::TouchPhase::Cancel => (false, false),
            };
        }
    });
    ui.data_mut(|d| d.insert_temp(id, (alt, swipe, last)));
    alt
}

pub(super) fn mods(i: &egui::InputState, space: bool) -> Mods {
    Mods { shift: i.modifiers.shift, alt: i.modifiers.alt, cmd: i.modifiers.command, ctrl: i.modifiers.ctrl, space }
}

/// Held Space belongs to navigation only when neither the document nor a text field is typing.
pub(super) fn navigation_space(app: &mut DesignApp, ctx: &egui::Context) -> bool {
    ctx.input(|i| i.focused && i.key_down(egui::Key::Space)) && !app.session.wants_text() && !ctx.text_edit_focused()
}

/// Allow temporary Zoom only for Cmd+Space with no pointer press and no busy tool.
pub(super) fn spring_zoom_mode(app: &DesignApp, ctx: &egui::Context, space: bool) -> bool {
    let down = ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(("canvas_pointer_down", app.pane)))).unwrap_or(false);
    space && ctx.input(|i| i.modifiers.command) && !down && !app.session.tool_busy()
}

#[derive(Clone, Default)]
struct SpringZoom {
    tool: designcraft_tools::ZoomTool,
    down: bool,
}

/// Lend the Zoom tool the gesture; the selected tool and document selection stay intact.
pub(super) fn spring_zoom_input(app: &mut DesignApp, ui: &egui::Ui, resp: &egui::Response, xf: &Xf, active: bool) -> bool {
    let id = egui::Id::unique(("canvas_spring_zoom", app.pane));
    let mut zoom: SpringZoom = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    if !ui.input(|i| i.focused) {
        ui.data_mut(|d| d.remove::<SpringZoom>(id));
        return false;
    }
    if !zoom.down && (!active || ui.input(|i| i.pointer.middle_down())) {
        return false;
    }
    let (pointer, m) = ui.input(|i| (i.pointer.clone(), mods(i, true)));
    let vi = app.view_info();
    let mut events = Vec::new();
    let canvas = if xf.rot == 0 { app.canvas_rect } else { Some(resp.rect) };
    if pointer.primary_pressed()
        && resp.hovered()
        && let Some(p) = pointer.press_origin().filter(|p| canvas.is_some_and(|r| r.contains(*p)))
    {
        events.push(PointerEvent { kind: PointerKind::Down, pos: xf.to_canvas(p), mods: m, clicks: 1 });
        zoom.down = true;
        resp.request_focus();
    }
    if zoom.down
        && let Some(p) = pointer.interact_pos()
    {
        if resp.dragged() && resp.drag_delta() != egui::Vec2::ZERO {
            events.push(PointerEvent { kind: PointerKind::Drag, pos: xf.to_canvas(p), mods: m, clicks: 1 });
        }
        if pointer.primary_released() {
            events.push(PointerEvent { kind: PointerKind::Up, pos: xf.to_canvas(p), mods: m, clicks: 1 });
            zoom.down = false;
        }
    }
    for e in events {
        if let Err(err) = app.session.pointer_with_tool(&mut zoom.tool, &e, vi) {
            app.status(err.to_string());
        }
    }
    app.after_engine();
    // Keep ownership until pointer release even if the shortcut was released first, so an Up
    // cannot leak into the previous tool and edit the document.
    ui.data_mut(|d| d.insert_temp(id, zoom));
    true
}

/// Move the view's content by a screen-space distance `d` (rotated views move in screen axes).
pub(super) fn pan(app: &mut DesignApp, xf: &Xf, d: egui::Vec2) {
    let d = xf.unrotate_delta(d);
    if let Some(v) = app.view_mut() {
        v.origin = Point::new(v.origin.x - d.x as f64 / v.zoom, v.origin.y - d.y as f64 / v.zoom);
    }
}

/// How far the pointer must leave the auto-scroll mark before the view moves, and how fast it then
/// goes: pixels a second for each pixel beyond that, up to a limit.
const AUTO_SCROLL_DEAD: f32 = 12.0;
const AUTO_SCROLL_RATE: f32 = 3.0;
const AUTO_SCROLL_MAX: f32 = 2000.0;

/// Auto-scroll's state: where the wheel was clicked, or the press that ended it, still held.
#[derive(Clone, Copy)]
enum AutoScroll {
    From(Pos2),
    Ending,
}

fn auto_scroll_id(app: &DesignApp) -> egui::Id {
    egui::Id::unique(("canvas_auto_scroll", app.pane))
}

/// Whether the view is gliding after a click of the wheel.
pub(super) fn auto_scrolling(app: &DesignApp, ui: &egui::Ui) -> bool {
    matches!(ui.data(|d| d.get_temp(auto_scroll_id(app))), Some(AutoScroll::From(_)))
}

/// Auto-scroll, as in Office: click the wheel (press and let go without moving) and the view glides
/// the way the pointer is from where it was clicked, faster the further away it is, in any direction.
/// Any click or Escape ends it, and that click does nothing else. True while it has the pointer.
pub(super) fn auto_scroll(app: &mut DesignApp, ui: &egui::Ui, resp: &egui::Response, xf: &Xf, blocked: bool) -> bool {
    let id = auto_scroll_id(app);
    let state: Option<AutoScroll> = ui.data(|d| d.get_temp(id));
    let (pressed, any_down, escape, pointer, dt, focused) = ui
        .input(|i| (i.pointer.any_pressed(), i.pointer.any_down(), i.key_pressed(egui::Key::Escape), i.pointer.latest_pos(), i.stable_dt, i.focused));
    match state {
        None => {
            if blocked || !resp.clicked_by(egui::PointerButton::Middle) {
                return false;
            }
            let Some(p) = pointer else { return false };
            ui.data_mut(|d| d.insert_temp(id, AutoScroll::From(p)));
            ui.ctx().request_repaint();
            true
        }
        Some(AutoScroll::Ending) => {
            // The click that ended it, until it is let go: it must not start it again or reach a tool.
            if !any_down {
                ui.data_mut(|d| d.remove::<AutoScroll>(id));
            }
            true
        }
        Some(AutoScroll::From(mark)) => {
            if pressed || escape || !focused || blocked {
                if pressed {
                    ui.data_mut(|d| d.insert_temp(id, AutoScroll::Ending));
                } else {
                    ui.data_mut(|d| d.remove::<AutoScroll>(id));
                }
                ui.ctx().request_repaint();
                return true;
            }
            if let Some(p) = pointer {
                let d = p - mark;
                let far = d.length();
                if far > AUTO_SCROLL_DEAD && far.is_finite() {
                    let speed = ((far - AUTO_SCROLL_DEAD) * AUTO_SCROLL_RATE).min(AUTO_SCROLL_MAX);
                    // The view goes toward the pointer, so the page goes the other way.
                    pan(app, xf, d / far * -speed * dt.clamp(0.0, 0.1));
                }
            }
            auto_scroll_mark(ui, mark);
            ui.ctx().request_repaint();
            true
        }
    }
}

/// The mark where the wheel was clicked: a round badge with an arrow each way, as Windows draws it.
fn auto_scroll_mark(ui: &egui::Ui, at: Pos2) {
    let painter = ui.painter();
    let (ink, paper) = (egui::Color32::from_gray(40), egui::Color32::from_white_alpha(230));
    painter.circle(at, 13.0, paper, egui::Stroke::new(1.0, ink));
    painter.circle_filled(at, 1.5, ink);
    for (x, y) in [(0.0f32, -1.0f32), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        let (dir, side) = (vec2(x, y), vec2(-y, x));
        let (tip, base) = (at + dir * 11.0, at + dir * 6.0);
        painter.add(egui::Shape::convex_polygon(vec![tip, base + side * 3.0, base - side * 3.0], ink, egui::Stroke::NONE));
    }
}

/// A scrollbar thumb on a track `track` pixels long, for `visible` (the view) within `content`
/// (the pasteboard), both spans along the bar's axis in screen pixels: the thumb's start and
/// length on the track, and the content pixels one thumb pixel moves (0 when nothing scrolls).
fn thumb_span(track: f32, content: (f32, f32), visible: (f32, f32)) -> (f32, f32, f32) {
    let track = track.max(0.0);
    let (lo, hi) = (content.0.min(visible.0), content.1.max(visible.1));
    let (total, shown) = (hi - lo, visible.1 - visible.0);
    let scrollable = total - shown;
    if !total.is_finite() || !shown.is_finite() || total <= 0.0 || scrollable <= 0.0 {
        return (0.0, track, 0.0);
    }
    let len = (track * shown / total).max(MIN_THUMB.min(track));
    let free = track - len;
    if free <= 0.0 {
        return (0.0, track, 0.0);
    }
    ((visible.0 - lo) / scrollable * free, len, scrollable / free)
}

/// A scrollbar's width, and the length of the arrow button at each end (0: it has none).
#[derive(Clone, Copy)]
struct BarSize {
    width: f32,
    arrow: f32,
}

fn bar_size(app: &DesignApp) -> BarSize {
    if app.ui.pressroom_shell { BarSize { width: PRESS_SCROLLBAR, arrow: PRESS_SCROLLBAR } } else { BarSize { width: SCROLLBAR, arrow: 0.0 } }
}

/// The track and thumb of the vertical (or horizontal) scrollbar of the view `screen` inside
/// `outer`, and the content pixels per thumb pixel.
fn scrollbar_rects(outer: Rect, screen: Rect, content: Rect, vertical: bool, size: BarSize) -> (Rect, Rect, f32) {
    let BarSize { width, arrow } = size;
    if vertical {
        let track = Rect::from_min_max(pos2(outer.max.x - width, screen.min.y + arrow), pos2(outer.max.x, screen.max.y - arrow));
        let (start, len, k) = thumb_span(track.height(), (content.min.y, content.max.y), (screen.min.y, screen.max.y));
        (track, Rect::from_min_size(pos2(track.min.x, track.min.y + start), vec2(width, len)), k)
    } else {
        let track = Rect::from_min_max(pos2(screen.min.x + arrow, outer.max.y - width), pos2(screen.max.x - arrow, outer.max.y));
        let (start, len, k) = thumb_span(track.width(), (content.min.x, content.max.x), (screen.min.x, screen.max.x));
        (track, Rect::from_min_size(pos2(track.min.x + start, track.min.y), vec2(len, width)), k)
    }
}

/// The arrow buttons at a scrollbar's start (up or left) and end (down or right).
fn scrollbar_arrows(outer: Rect, screen: Rect, vertical: bool, size: BarSize) -> [Rect; 2] {
    let BarSize { width, arrow } = size;
    if vertical {
        let x = outer.max.x - width;
        [Rect::from_min_size(pos2(x, screen.min.y), vec2(width, arrow)), Rect::from_min_size(pos2(x, screen.max.y - arrow), vec2(width, arrow))]
    } else {
        let y = outer.max.y - width;
        [Rect::from_min_size(pos2(screen.min.x, y), vec2(arrow, width)), Rect::from_min_size(pos2(screen.max.x - arrow, y), vec2(arrow, width))]
    }
}

/// What a press of an arrow button moves the view: one step at once, then, held past a moment, on
/// and on, each frame's worth.
const ARROW_STEP: f32 = 40.0;
const ARROW_DELAY: f64 = 0.35;
const ARROW_SPEED: f32 = 700.0;

fn arrow_move(ui: &egui::Ui, id: egui::Id, resp: &egui::Response) -> f32 {
    let held = id.with("held");
    if !resp.is_pointer_button_down_on() {
        ui.data_mut(|d| d.remove::<f64>(held));
        return 0.0;
    }
    ui.ctx().request_repaint();
    let (now, dt) = ui.input(|i| (i.time, i.stable_dt));
    let Some(from) = ui.data(|d| d.get_temp::<f64>(held)) else {
        ui.data_mut(|d| d.insert_temp(held, now));
        return ARROW_STEP;
    };
    if now - from < ARROW_DELAY { 0.0 } else { ARROW_SPEED * dt.clamp(0.0, 0.1) }
}

/// How far a scrollbar interaction pans the view along its axis: a thumb drag follows the pointer,
/// a click on the track beside the thumb pages by one view.
fn scrollbar_move(resp: &egui::Response, grabbed: bool, vertical: bool, thumb: Rect, page: f32, k: f32) -> f32 {
    if grabbed {
        let d = resp.drag_delta();
        return -(if vertical { d.y } else { d.x }) * k;
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let (p, a, b) = if vertical { (p.y, thumb.min.y, thumb.max.y) } else { (p.x, thumb.min.x, thumb.max.x) };
        if p < a {
            return page;
        } else if p > b {
            return -page;
        }
    }
    0.0
}

/// The view's scrollbars, on the right and bottom edges of `outer`: they span the pasteboard;
/// dragging a thumb pans, clicking the track pages by one view, and PressRoom's arrows step.
pub(super) fn scrollbars(app: &mut DesignApp, ui: &mut egui::Ui, outer: Rect, screen: Rect, rect: Rect, t: &Tokens) {
    let content = |app: &DesignApp| {
        let st = app.session.active()?;
        let v = app.view()?;
        let xf = Xf::new(rect, v);
        Some((xf, xf.rect(CanvasLayout::new(&st.doc, st.editing_parents).pasteboard)))
    };
    let Some((xf, cb)) = content(app) else { return };
    let press = ui.input(|i| i.pointer.press_origin());
    let size = bar_size(app);
    let mut hot = [false; 2];
    for (n, vertical) in [true, false].into_iter().enumerate() {
        let (track, thumb, k) = scrollbar_rects(outer, screen, cb, vertical, size);
        let id = egui::Id::unique(("canvas_scrollbar", app.pane, vertical));
        let resp = ui.interact(track, id, Sense::click_and_drag());
        let grab_id = id.with("grab");
        if resp.drag_started() {
            let on_thumb = press.is_some_and(|p| thumb.contains(p));
            ui.data_mut(|d| d.insert_temp(grab_id, on_thumb));
        }
        let grabbed = resp.dragged() && ui.data(|d| d.get_temp::<bool>(grab_id)).unwrap_or(false);
        if !resp.dragged() {
            ui.data_mut(|d| d.remove::<bool>(grab_id));
        }
        let page = if vertical { screen.size().y } else { screen.size().x };
        let moved = scrollbar_move(&resp, grabbed, vertical, thumb, page, k);
        if moved != 0.0 && moved.is_finite() {
            pan(app, &xf, if vertical { vec2(0.0, moved) } else { vec2(moved, 0.0) });
        }
        if let Some(slot) = hot.get_mut(n) {
            *slot = grabbed || resp.hovered();
        }
        if size.arrow > 0.0 {
            let [back, on] = scrollbar_arrows(outer, screen, vertical, size);
            for (rect, sign, end) in [(back, 1.0, "back"), (on, -1.0, "on")] {
                let id = id.with(end);
                let resp = ui.interact(rect, id, Sense::click_and_drag());
                let moved = arrow_move(ui, id, &resp) * sign;
                if moved != 0.0 {
                    pan(app, &xf, if vertical { vec2(0.0, moved) } else { vec2(moved, 0.0) });
                }
            }
        }
    }
    // Painted from the view as it is after this frame's input.
    let Some((_, cb)) = content(app) else { return };
    let painter = ui.painter_at(outer);
    let right = Rect::from_min_max(pos2(outer.max.x - size.width, outer.min.y), outer.max);
    let bottom = Rect::from_min_max(pos2(outer.min.x, outer.max.y - size.width), outer.max);
    if app.ui.pressroom_shell {
        // Publisher's scroll bars: always there, a plain thumb on a track, an arrow at each end.
        // They are the way sideways that shows, for a mouse with only an up-and-down wheel.
        let p = crate::pressroom::pal();
        painter.rect_filled(right, 0.0, p.scratch);
        painter.rect_filled(bottom, 0.0, p.scratch);
        let pointer = ui.input(|i| i.pointer.hover_pos());
        for (n, vertical) in [true, false].into_iter().enumerate() {
            let (track, thumb, _) = scrollbar_rects(outer, screen, cb, vertical, size);
            painter.rect_filled(track, 0.0, p.text.gamma_multiply(0.07));
            let alpha = if hot.get(n).copied().unwrap_or(false) { 0.62 } else { 0.42 };
            painter.rect_filled(thumb.shrink(2.0), 2.0, p.text.gamma_multiply(alpha));
            for (arrow, sign) in scrollbar_arrows(outer, screen, vertical, size).into_iter().zip([-1.0f32, 1.0]) {
                if pointer.is_some_and(|q| arrow.contains(q)) {
                    painter.rect_filled(arrow, 0.0, p.text.gamma_multiply(0.14));
                }
                let (dir, side) = if vertical { (vec2(0.0, sign), vec2(1.0, 0.0)) } else { (vec2(sign, 0.0), vec2(0.0, 1.0)) };
                let c = arrow.center();
                let points = vec![c + dir * 3.5, c - dir * 2.5 + side * 4.0, c - dir * 2.5 - side * 4.0];
                painter.add(egui::Shape::convex_polygon(points, p.text.gamma_multiply(0.75), egui::Stroke::NONE));
            }
        }
        return;
    }
    painter.rect_filled(right, 0.0, t.panel_darker);
    painter.rect_filled(bottom, 0.0, t.panel_darker);
    for (n, vertical) in [true, false].into_iter().enumerate() {
        let (_, thumb, _) = scrollbar_rects(outer, screen, cb, vertical, size);
        let alpha = if hot.get(n).copied().unwrap_or(false) { 0.6 } else { 0.35 };
        painter.rect_filled(thumb.shrink(2.0), SCROLLBAR / 2.0 - 2.0, t.text.gamma_multiply(alpha));
    }
}

#[cfg(test)]
mod scrollbar_tests {
    use egui::{Event, Modifiers, PointerButton};
    use egui_kittest::Harness;

    use super::*;
    use crate::test_window::{Window, open, wheel};

    fn window() -> Harness<'static, Window> {
        let mut h = open(DesignApp::new(designcraft_engine::Session::new(), crate::Services::default()), vec2(1440.0, 900.0));
        // Zoomed in so both axes scroll.
        set_zoom(&mut h.state_mut().app, 2.0);
        h.run_steps(3);
        h
    }

    fn origin(h: &Harness<'static, Window>) -> Point {
        h.state().app.view().map(|v| v.origin).unwrap_or_default()
    }

    /// The scrollbar's track and thumb as the canvas lays them out for the current view, and
    /// the content pixels per thumb pixel.
    fn bar(h: &Harness<'static, Window>, vertical: bool) -> (Rect, Rect, f32) {
        let app = &h.state().app;
        let rect = app.canvas_rect.unwrap();
        let st = app.session.active().unwrap();
        let xf = Xf::new(rect, app.view().unwrap());
        let cb = xf.rect(CanvasLayout::new(&st.doc, st.editing_parents).pasteboard);
        let size = bar_size(app);
        let outer = Rect::from_min_max(rect.min, rect.max + vec2(size.width, size.width));
        scrollbar_rects(outer, rect, cb, vertical, size)
    }

    /// The arrow buttons of a scrollbar as the canvas lays them out.
    fn arrows(h: &Harness<'static, Window>, vertical: bool) -> [Rect; 2] {
        let app = &h.state().app;
        let rect = app.canvas_rect.unwrap();
        let size = bar_size(app);
        scrollbar_arrows(Rect::from_min_max(rect.min, rect.max + vec2(size.width, size.width)), rect, vertical, size)
    }

    /// A mouse with only an up-and-down wheel: what shows on screen has to take it sideways.
    #[test]
    fn pressrooms_scrollbars_are_wide_with_arrows_that_step_and_a_thumb_to_drag() {
        let mut h = pressroom_window();
        let rect = h.state().app.canvas_rect.unwrap();
        for vertical in [true, false] {
            let (track, thumb, k) = bar(&h, vertical);
            let r = h.ctx.read_response(egui::Id::unique(("canvas_scrollbar", 0u8, vertical))).expect("scrollbar").rect;
            assert_eq!(r, track);
            assert!(k > 0.0, "the zoomed-in view scrolls");
            if vertical {
                assert_eq!((track.width(), track.min.x), (PRESS_SCROLLBAR, rect.max.x));
                assert!(thumb.height() < track.height());
            } else {
                assert_eq!((track.height(), track.min.y), (PRESS_SCROLLBAR, rect.max.y));
                assert!(thumb.width() < track.width());
            }
        }
        // The arrow at the right end goes right, and only right; the one at the left comes back.
        let a = origin(&h);
        let [left, right] = arrows(&h, false);
        crate::test_window::click_at(&mut h, right.center());
        let b = origin(&h);
        assert!(b.x > a.x && b.y == a.y, "{a:?} -> {b:?}");
        crate::test_window::click_at(&mut h, left.center());
        let c = origin(&h);
        assert!(c.x < b.x && c.y == a.y, "{b:?} -> {c:?}");
        // Down and up.
        let [up, down] = arrows(&h, true);
        crate::test_window::click_at(&mut h, down.center());
        let d = origin(&h);
        assert!(d.y > c.y && d.x == c.x, "{c:?} -> {d:?}");
        crate::test_window::click_at(&mut h, up.center());
        assert!(origin(&h).y < d.y);
        // The bottom thumb drags sideways.
        let (_, thumb, _) = bar(&h, false);
        let e = origin(&h);
        drag(&mut h, thumb.center(), thumb.center() + vec2(40.0, 0.0), Modifiers::NONE);
        let f = origin(&h);
        assert!(f.x > e.x && f.y == e.y, "{e:?} -> {f:?}");
        // Clicking the track beside the thumb pages.
        let (track, thumb, _) = bar(&h, false);
        let beside =
            if thumb.max.x + 8.0 < track.max.x { pos2(thumb.max.x + 4.0, track.center().y) } else { pos2(thumb.min.x - 4.0, track.center().y) };
        crate::test_window::click_at(&mut h, beside);
        assert_ne!(origin(&h).x, f.x);
    }

    #[test]
    fn an_arrow_held_down_keeps_going() {
        let mut h = pressroom_window();
        let [_, right] = arrows(&h, false);
        h.hover_at(right.center());
        h.step();
        let a = origin(&h);
        h.event(Event::PointerButton { pos: right.center(), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        h.step();
        let once = origin(&h);
        assert!(once.x > a.x, "the press itself steps");
        // The test window's frames are a quarter of a second each.
        h.step();
        assert_eq!(origin(&h), once, "then a moment's pause");
        h.run_steps(4);
        let held = origin(&h);
        assert!(held.x > once.x, "then it goes on: {once:?} -> {held:?}");
        h.event(Event::PointerButton { pos: right.center(), button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        h.run_steps(2);
        let let_go = origin(&h);
        h.run_steps(8);
        assert_eq!(origin(&h), let_go, "and stops when let go");
    }

    /// A primary-button drag from `a` to `b` in a few moves, with `mods` held throughout.
    fn drag(h: &mut Harness<'static, Window>, a: Pos2, b: Pos2, mods: Modifiers) {
        h.event(Event::ModifiersChanged(mods));
        h.hover_at(a);
        h.step();
        h.event(Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: mods });
        h.step();
        for n in 1..=4 {
            h.hover_at(a + (b - a) * n as f32 / 4.0);
            h.step();
        }
        h.event(Event::PointerButton { pos: b, button: PointerButton::Primary, pressed: false, modifiers: mods });
        h.event(Event::ModifiersChanged(Modifiers::NONE));
        h.run_steps(3);
    }

    /// Scrollbars sit immediately outside the view, have non-full-size thumbs and use a positive content scale.
    #[test]
    fn document_views_have_scrollbars_beside_the_view() {
        let h = window();
        let rect = h.state().app.canvas_rect.unwrap();
        for vertical in [true, false] {
            let r = h.ctx.read_response(egui::Id::unique(("canvas_scrollbar", 0u8, vertical))).expect("scrollbar").rect;
            let (track, thumb, k) = bar(&h, vertical);
            assert_eq!(r, track);
            assert!(k > 0.0, "the zoomed-in view scrolls");
            if vertical {
                assert_eq!((r.min.x, r.width(), r.min.y, r.max.y), (rect.max.x, SCROLLBAR, rect.min.y, rect.max.y));
                assert!(thumb.height() < track.height());
            } else {
                assert_eq!((r.min.y, r.height(), r.min.x, r.max.x), (rect.max.y, SCROLLBAR, rect.min.x, rect.max.x));
                assert!(thumb.width() < track.width());
            }
        }
    }

    #[test]
    fn dragging_a_thumb_pans_and_clicking_the_track_pages() {
        let mut h = window();
        let zoom = h.state().app.view().unwrap().zoom;
        // Vertical thumb: down 40 px moves the view down by 40 thumb pixels' worth of content.
        let (_, thumb, k) = bar(&h, true);
        let before = origin(&h);
        drag(&mut h, thumb.center(), thumb.center() + vec2(0.0, 40.0), Modifiers::NONE);
        let after = origin(&h);
        let want = 40.0 * k as f64 / zoom;
        assert!((after.y - before.y - want).abs() < want * 0.2, "{before:?} → {after:?}, want +{want}");
        assert_eq!(after.x, before.x);
        // Horizontal thumb: left 30 px.
        let (_, thumb, k) = bar(&h, false);
        drag(&mut h, thumb.center(), thumb.center() - vec2(30.0, 0.0), Modifiers::NONE);
        let next = origin(&h);
        let want = 30.0 * k as f64 / zoom;
        assert!((after.x - next.x - want).abs() < want * 0.2, "{after:?} → {next:?}, want -{want}");
        assert_eq!(next.y, after.y);
        // A click on the vertical track above the thumb pages up by one view.
        let (track, thumb, _) = bar(&h, true);
        assert!(thumb.min.y - track.min.y > 4.0);
        crate::test_window::click_at(&mut h, pos2(track.center().x, track.min.y + 2.0));
        let paged = origin(&h);
        let view = h.state().app.canvas_rect.unwrap().height() as f64 / zoom;
        assert!((next.y - paged.y - view).abs() < 0.01, "{next:?} → {paged:?}, want -{view}");
    }

    /// Wheel scrolling moves both thumbs and keeps the painted scrollbar rect in sync.
    #[test]
    fn scrolling_the_view_moves_the_thumbs() {
        let mut h = window();
        let (_, v0, _) = bar(&h, true);
        let (_, h0, _) = bar(&h, false);
        let c = h.state().app.canvas_rect.unwrap().center();
        wheel(&mut h, c, vec2(-50.0, -80.0));
        let (_, v1, _) = bar(&h, true);
        let (_, h1, _) = bar(&h, false);
        assert!(v1.min.y > v0.min.y && h1.min.x > h0.min.x, "{v0:?} → {v1:?}, {h0:?} → {h1:?}");
        let r = h.ctx.read_response(egui::Id::unique(("canvas_scrollbar", 0u8, true))).expect("scrollbar");
        assert_eq!(r.rect, bar(&h, true).0);
    }

    /// PressRoom's own window: the routes must work there, ribbon and all, not only in DesignCraft's.
    fn pressroom_window() -> Harness<'static, Window> {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::Services::default());
        app.ui.pressroom_shell = true;
        let mut h = open(app, vec2(1440.0, 900.0));
        set_zoom(&mut h.state_mut().app, 2.0);
        h.run_steps(3);
        h
    }

    /// The wheel pressed and let go on the spot.
    fn wheel_click(h: &mut Harness<'static, Window>, at: Pos2) {
        h.hover_at(at);
        h.step();
        h.event(Event::PointerButton { pos: at, button: PointerButton::Middle, pressed: true, modifiers: Modifiers::NONE });
        h.step();
        h.event(Event::PointerButton { pos: at, button: PointerButton::Middle, pressed: false, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }

    #[test]
    fn clicking_the_wheel_glides_the_view_toward_the_pointer_in_any_direction() {
        for mut h in [window(), pressroom_window()] {
            let c = h.state().app.canvas_rect.unwrap().center();
            wheel_click(&mut h, c);
            assert!(auto_scrolling_in(&h), "the click starts it");
            // Near the mark, nothing moves.
            let still = origin(&h);
            h.hover_at(c + vec2(5.0, 0.0));
            h.run_steps(5);
            assert_eq!(origin(&h), still);
            // To the right of it, the view goes right; not up or down.
            h.hover_at(c + vec2(150.0, 0.0));
            h.run_steps(10);
            let right = origin(&h);
            assert!(right.x > still.x && right.y == still.y, "{still:?} -> {right:?}");
            // Further away is faster.
            h.hover_at(c + vec2(300.0, 0.0));
            h.run_steps(10);
            let faster = origin(&h);
            assert!(faster.x - right.x > right.x - still.x, "{still:?} -> {right:?} -> {faster:?}");
            // And left, and down.
            h.hover_at(c + vec2(-150.0, 120.0));
            h.run_steps(10);
            let back = origin(&h);
            assert!(back.x < faster.x && back.y > faster.y, "{faster:?} -> {back:?}");
        }
    }

    fn auto_scrolling_in(h: &Harness<'static, Window>) -> bool {
        matches!(h.ctx.data(|d| d.get_temp(auto_scroll_id(&h.state().app))), Some(AutoScroll::From(_)))
    }

    #[test]
    fn any_click_or_escape_ends_auto_scroll_and_does_nothing_else() {
        let mut h = pressroom_window();
        let c = h.state().app.canvas_rect.unwrap().center();
        // A frame under the pointer: the click that ends it mustn't select it.
        let (a, b) = {
            let app = &h.state().app;
            let xf = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap());
            (xf.to_canvas(c - vec2(60.0, 60.0)), xf.to_canvas(c + vec2(60.0, 60.0)))
        };
        h.state_mut().app.run("frame.create", json!({"rect": [a.x, a.y, b.x, b.y], "shape": "rectangle", "content": "unassigned"})).unwrap();
        h.state_mut().app.run("edit.deselectAll", json!({})).unwrap();
        h.run_steps(2);
        wheel_click(&mut h, c);
        assert!(auto_scrolling_in(&h));
        crate::test_window::click_at(&mut h, c);
        assert!(!auto_scrolling_in(&h), "a click ends it");
        assert!(h.state().app.session.doc().unwrap().selection.items.is_empty(), "and selects nothing");
        let stopped = origin(&h);
        h.hover_at(c + vec2(200.0, 0.0));
        h.run_steps(8);
        assert_eq!(origin(&h), stopped, "the view stays");
        // A second click of the wheel ends it, and doesn't start it again as it is let go.
        wheel_click(&mut h, c);
        assert!(auto_scrolling_in(&h));
        wheel_click(&mut h, c);
        h.run_steps(3);
        assert!(!auto_scrolling_in(&h), "the wheel's own click ends it");
        // Escape.
        wheel_click(&mut h, c);
        assert!(auto_scrolling_in(&h));
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
        assert!(!auto_scrolling_in(&h), "Escape ends it");
    }

    #[test]
    fn holding_the_wheel_and_dragging_is_not_taken_for_a_click() {
        let mut h = pressroom_window();
        let c = h.state().app.canvas_rect.unwrap().center();
        wheel_button_drag(&mut h, c, c + vec2(120.0, 0.0));
        assert!(!auto_scrolling_in(&h));
    }

    #[test]
    fn all_of_a_mouses_ways_of_going_sideways_work_in_pressrooms_own_window() {
        let mut h = pressroom_window();
        let c = h.state().app.canvas_rect.unwrap().center();
        let a = origin(&h);
        wheel_notch(&mut h, c, vec2(0.0, -1.0), Modifiers::SHIFT);
        let b = origin(&h);
        assert!(b.x > a.x && b.y == a.y, "Shift and the wheel: {a:?} -> {b:?}");
        wheel_notch(&mut h, c, vec2(-1.0, 0.0), Modifiers::NONE);
        let d = origin(&h);
        assert!(d.x > b.x && d.y == b.y, "a sideways wheel: {b:?} -> {d:?}");
        wheel_button_drag(&mut h, c, c + vec2(120.0, 0.0));
        let e = origin(&h);
        assert!(e.x < d.x && e.y == d.y, "holding the wheel: {d:?} -> {e:?}");
    }

    /// A mouse's wheel, notch by notch, as Windows reports it: lines, with Shift held if asked.
    fn wheel_notch(h: &mut Harness<'static, Window>, pos: Pos2, delta: egui::Vec2, modifiers: Modifiers) {
        h.hover_at(pos);
        h.event(Event::ModifiersChanged(modifiers));
        h.step();
        h.event(Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta,
            phase: egui::TouchPhase::Move,
            modifiers,
            source: egui::MouseWheelSource::Unknown,
        });
        h.run_steps(10);
        h.event(Event::ModifiersChanged(Modifiers::NONE));
        h.run_steps(2);
    }

    #[test]
    fn a_plain_mouse_moves_the_page_sideways_with_shift_and_the_wheel() {
        let mut h = window();
        let c = h.state().app.canvas_rect.unwrap().center();
        let before = origin(&h);
        wheel_notch(&mut h, c, vec2(0.0, -1.0), Modifiers::SHIFT);
        let after = origin(&h);
        assert!(after.x > before.x, "{before:?} -> {after:?}");
        assert_eq!(after.y, before.y);
    }

    #[test]
    fn a_mouse_with_a_sideways_wheel_moves_the_page_sideways() {
        let mut h = window();
        let c = h.state().app.canvas_rect.unwrap().center();
        let before = origin(&h);
        wheel_notch(&mut h, c, vec2(-1.0, 0.0), Modifiers::NONE);
        let after = origin(&h);
        assert!(after.x > before.x, "{before:?} -> {after:?}");
        assert_eq!(after.y, before.y);
    }

    /// Holding the wheel down and moving the mouse, a press at a time as a real mouse sends it.
    fn wheel_button_drag(h: &mut Harness<'static, Window>, a: Pos2, b: Pos2) {
        h.hover_at(a);
        h.step();
        h.event(Event::PointerButton { pos: a, button: PointerButton::Middle, pressed: true, modifiers: Modifiers::NONE });
        h.step();
        for n in 1..=6 {
            h.hover_at(a + (b - a) * n as f32 / 6.0);
            h.step();
        }
        h.event(Event::PointerButton { pos: b, button: PointerButton::Middle, pressed: false, modifiers: Modifiers::NONE });
        h.run_steps(3);
    }

    #[test]
    fn holding_the_wheel_down_and_moving_the_mouse_pans_with_the_page_following() {
        let mut h = window();
        let c = h.state().app.canvas_rect.unwrap().center();
        let zoom = h.state().app.view().unwrap().zoom;
        let before = origin(&h);
        wheel_button_drag(&mut h, c, c + vec2(120.0, 0.0));
        let after = origin(&h);
        let want = 120.0 / zoom;
        assert!((before.x - after.x - want).abs() < want * 0.2, "{before:?} -> {after:?}, want -{want}");
        assert_eq!(after.y, before.y);
    }

    #[test]
    fn alt_drag_while_editing_text_pans_and_keeps_the_caret() {
        let mut h = window();
        let app = &mut h.state_mut().app;
        let rect = app.canvas_rect.unwrap();
        let xf = Xf::new(rect, app.view().unwrap());
        // A text frame filling the visible part of the page, with the caret in it.
        let (a, b) = (xf.to_canvas(rect.min + vec2(20.0, 20.0)), xf.to_canvas(rect.max - vec2(20.0, 20.0)));
        let r = app.run("frame.create", json!({"rect": [a.x, a.y, b.x, b.y], "content": "text", "text": "Hello brave world"})).unwrap();
        let sid = r["story"].as_u64().unwrap();
        app.run("tool.select", json!({"tool": "type"})).unwrap();
        app.run("text.select", json!({"story": sid, "anchor": 3, "focus": 3})).unwrap();
        assert!(app.session.wants_text());
        h.run_steps(2);
        let caret = h.state().app.session.doc().unwrap().selection.text;
        let before = origin(&h);
        let zoom = h.state().app.view().unwrap().zoom;
        let from = rect.center();
        drag(&mut h, from, from + vec2(60.0, 40.0), Modifiers::ALT);
        let after = origin(&h);
        assert!(after.x < before.x && after.y < before.y, "{before:?} → {after:?}");
        assert!((before.x - after.x - 60.0 / zoom).abs() < 15.0 / zoom, "{before:?} → {after:?}");
        assert_eq!(h.state().app.session.doc().unwrap().selection.text, caret);
        assert!(h.state_mut().app.session.wants_text(), "still editing text");
        // Without Alt the same drag selects text and leaves the view alone.
        drag(&mut h, from, from + vec2(60.0, 40.0), Modifiers::NONE);
        assert_eq!(origin(&h), after);
    }
}

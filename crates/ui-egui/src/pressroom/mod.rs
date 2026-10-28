//! PressRoom: a desktop-publishing window laid out the way a Microsoft Publisher user knows it —
//! a coloured title bar with the Quick Access Toolbar, the File screen, the ribbon tabs (Home,
//! Insert, Page Design, Mailings, Review, View, and Format when a box is selected), the Page
//! Navigation pane, and a status bar with the page count and zoom slider — around DesignCraft's
//! layout engine. All art is original; nothing is taken from Microsoft's product.

mod art;
mod backstage;
mod keys;
mod nav;
mod ribbon;
mod templates;

use designcraft_doc::Content;
use egui::{Color32, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::DesignApp;

pub use backstage::Backstage;
pub use keys::shortcuts;

/// The window's colours: Publisher's family teal on the bars, light grey ribbon, mid-grey scratch
/// area.
#[allow(non_snake_case)]
pub mod PR {
    use egui::Color32;
    pub const TEAL: Color32 = Color32::from_rgb(7, 117, 104);
    pub const TEAL_DARK: Color32 = Color32::from_rgb(5, 92, 82);
    pub const TEAL_HOVER: Color32 = Color32::from_rgb(38, 138, 125);
    pub const RIBBON: Color32 = Color32::from_rgb(243, 243, 243);
    pub const RIBBON_EDGE: Color32 = Color32::from_rgb(213, 213, 213);
    pub const GROUP_SEP: Color32 = Color32::from_rgb(218, 218, 218);
    pub const GROUP_LABEL: Color32 = Color32::from_rgb(96, 96, 96);
    pub const TEXT: Color32 = Color32::from_rgb(38, 38, 38);
    pub const DISABLED: Color32 = Color32::from_rgb(166, 166, 166);
    pub const HOVER: Color32 = Color32::from_rgb(220, 220, 220);
    pub const HOVER_BORDER: Color32 = Color32::from_rgb(198, 198, 198);
    pub const PRESSED: Color32 = Color32::from_rgb(196, 196, 196);
    pub const CHECKED: Color32 = Color32::from_rgb(206, 206, 206);
    pub const SCRATCH: Color32 = Color32::from_rgb(171, 171, 171);
    pub const PANE: Color32 = Color32::from_rgb(243, 243, 243);
    pub const CONTEXT: Color32 = Color32::from_rgb(232, 184, 76);
    pub const SELECT: Color32 = Color32::from_rgb(242, 164, 64);
}

const TITLE_H: f32 = 31.0;
const TABS_H: f32 = 25.0;
const STATUS_H: f32 = 23.0;
const CAPTION_W: f32 = 46.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    File,
    #[default]
    Home,
    Insert,
    PageDesign,
    Mailings,
    Review,
    View,
    Format,
}

/// What the selection is, which picks the contextual Format tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Context {
    #[default]
    None,
    Text,
    Picture,
    Drawing,
    Table,
}

impl Context {
    fn title(self) -> &'static str {
        match self {
            Context::None => "",
            Context::Text => "Text Box Tools",
            Context::Picture => "Picture Tools",
            Context::Drawing => "Drawing Tools",
            Context::Table => "Table Tools",
        }
    }
}

/// The window's own state between frames.
#[derive(Clone, Debug, Default)]
struct State {
    tab: Tab,
    /// The ribbon shows tab names only.
    collapsed: bool,
    nav_hidden: bool,
    nav_narrow: bool,
    two_page: bool,
    backstage: Option<Backstage>,
    context: Context,
    /// Create Link is waiting for the next text box to be clicked: the box the text comes from.
    link_from: Option<u64>,
    /// The canvas width last frame (Page Width zoom).
    canvas_w: f32,
}

fn state_id() -> egui::Id {
    egui::Id::new("pressroom_state")
}

fn state(ctx: &egui::Context) -> State {
    ctx.data(|d| d.get_temp(state_id())).unwrap_or_default()
}

fn update(ctx: &egui::Context, f: impl FnOnce(&mut State)) {
    let mut s = state(ctx);
    f(&mut s);
    ctx.data_mut(|d| d.insert_temp(state_id(), s));
}

pub fn open_backstage(ctx: &egui::Context, page: Backstage) {
    update(ctx, |s| s.backstage = Some(page));
}

pub fn close_backstage(ctx: &egui::Context) {
    update(ctx, |s| s.backstage = None);
}

fn nav_hidden(ctx: &egui::Context) -> bool {
    state(ctx).nav_hidden
}

fn set_nav_hidden(ctx: &egui::Context, hidden: bool) {
    update(ctx, |s| s.nav_hidden = hidden);
}

fn two_page(ctx: &egui::Context) -> bool {
    state(ctx).two_page
}

/// Two-Page Spread shows facing pages side by side: the document's facing-pages setting.
fn set_two_page(app: &mut DesignApp, ctx: &egui::Context, on: bool) {
    update(ctx, |s| s.two_page = on);
    let facing = app.session.active().is_some_and(|d| d.doc.settings.facing_pages);
    if facing != on {
        ribbon::act_with(app, "layout.documentSetup", json!({"facingPages": on}));
    }
}

fn master_view(app: &DesignApp) -> bool {
    app.session.active().is_some_and(|d| d.editing_parents)
}

fn set_master_view(app: &mut DesignApp, on: bool) {
    ribbon::act_with(app, "layout.parents.edit", json!({"on": on}));
}

/// The kind of thing selected.
fn context(app: &DesignApp) -> Context {
    let Some(st) = app.session.active() else { return Context::None };
    if st.selection.cells.is_some() {
        return Context::Table;
    }
    if st.selection.text.is_some() {
        return Context::Text;
    }
    let Some(first) = st.selection.items.first().and_then(|id| st.doc.item(*id)) else { return Context::None };
    match &first.content {
        Content::Text(_) => Context::Text,
        Content::Graphic(_) => Context::Picture,
        // An empty picture box (a graphic frame waiting for its picture).
        Content::Unassigned if first.object_style == st.doc.styles.default_graphic_frame => Context::Picture,
        _ => Context::Drawing,
    }
}

/// The selected text frame (or the frame of the text being edited).
fn selected_text_frame(app: &DesignApp) -> Option<designcraft_doc::ItemId> {
    let st = app.session.active()?;
    if let Some(t) = &st.selection.text {
        if let Some(f) = t.frame {
            return Some(f);
        }
        let story = st.doc.story(t.story)?;
        return story.frames.last().copied();
    }
    let id = *st.selection.items.first()?;
    matches!(st.doc.item(id)?.content, Content::Text(_)).then_some(id)
}

/// The selected text boxes as command `ids`: the box being typed in counts, so Text Box Tools
/// work with the text cursor in the box as well as with the box selected.
fn text_frame_ids(app: &DesignApp) -> serde_json::Value {
    let Some(st) = app.session.active() else { return json!([]) };
    if st.selection.text.is_some() {
        return json!(selected_text_frame(app).map(|id| vec![id.0]).unwrap_or_default());
    }
    json!(st.selection.items.iter().map(|i| i.0).collect::<Vec<_>>())
}

// ───────────────────────────── actions shared by the ribbon ─────────────────────────────

/// Arm a drawing tool: the next drag on the page draws the box (Draw Text Box, Shapes…).
fn start_draw(app: &mut DesignApp, tool: &str) {
    app.select_tool(tool);
    app.status(match tool {
        "type" => "Click and drag on the page to draw a text box.",
        "rectangleFrame" => "Click and drag on the page to draw a picture placeholder.",
        _ => "Click and drag on the page to draw the shape.",
    });
}

/// A rectangle in the middle of the page in view, in spread coordinates: (spread index, rect).
fn centre_rect(app: &DesignApp, wf: f64, hf: f64) -> Option<(usize, [f64; 4])> {
    let st = app.session.active()?;
    let abs = crate::canvas::current_page(app).unwrap_or(0);
    let (si, pi) = st.doc.page_loc(abs)?;
    let pg = st.doc.spreads.get(si)?.pages.get(pi)?;
    let (w, h) = (pg.width * wf, pg.height * hf);
    let (cx, cy) = (pg.x + pg.width / 2.0, pg.height / 2.0);
    Some((si, [cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0]))
}

/// Draw Table / Table: a table needs a text box to live in; make one in the middle of the page
/// when no text is being edited, then ask for the rows and columns.
fn insert_table(app: &mut DesignApp) {
    let editing = app.session.active().is_some_and(|d| d.selection.text.is_some());
    if !editing && let Some((si, rect)) = centre_rect(app, 0.7, 0.3) {
        let _ = app.run("frame.create", json!({"spread": si, "rect": rect, "content": "text", "caret": true}));
    }
    ribbon::act(app, "app.insertTableDialog");
}

/// Pictures: choose a picture file. It fills the selected picture box (cropped to fill, as a
/// Publisher placeholder does), or lands in the middle of the page in view, ready to move.
fn insert_picture(app: &mut DesignApp) {
    let Some(pick) = app.services.pick_open.as_mut() else {
        // No synchronous picker (web): the place cursor.
        ribbon::act(app, "app.placeDialog");
        return;
    };
    let Some(path) = pick("place") else { return };
    let target = app.session.active().and_then(|st| {
        let id = *st.selection.items.first()?;
        let it = st.doc.item(id)?;
        (st.selection.items.len() == 1 && matches!(it.content, Content::Graphic(_) | Content::Unassigned)).then_some(id)
    });
    let placed = match target {
        Some(id) => app.open_file("file.place", json!({"path": path, "frame": id.0})).map(|_| {
            let _ = app.run("selection.set", json!({"ids": [id.0]}));
            let _ = app.run("fit.fillFrameProportionally", json!({}));
        }),
        None => {
            let Some((si, rect)) = centre_rect(app, 0.6, 0.4) else { return };
            app.open_file("file.place", json!({"path": path, "spread": si, "x": rect[0], "y": rect[1], "width": rect[2] - rect[0]})).map(|_| ())
        }
    };
    if let Err(e) = placed {
        app.status(format!("Couldn't insert the picture: {e}"));
    }
}

fn duplicate_page(app: &mut DesignApp, abs: usize) {
    let Some(si) = app.session.active().and_then(|d| d.doc.page_loc(abs)).map(|l| l.0) else { return };
    ribbon::act_with(app, "layout.pages.duplicateSpread", json!({"spread": si}));
}

fn delete_page(app: &mut DesignApp, abs: usize) {
    ribbon::act_with(app, "layout.pages.delete", json!({"pages": [abs]}));
}

/// Today's date as Publisher's default long format ("October 28, 2026").
fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let name = MONTHS.get((m - 1).clamp(0, 11) as usize).copied().unwrap_or("January");
    format!("{name} {d}, {y}")
}

fn insert_date(app: &mut DesignApp) {
    let editing = app.session.active().is_some_and(|d| d.selection.text.is_some());
    if editing {
        let _ = app.run("text.insert", json!({"text": today()}));
    } else if let Some((si, rect)) = centre_rect(app, 0.4, 0.06) {
        let _ = app.run("frame.create", json!({"spread": si, "rect": rect, "content": "text", "text": today()}));
    }
}

/// Header / Footer: open the master page and put a text box in its top (or bottom) margin.
fn edit_header_footer(app: &mut DesignApp, header: bool) {
    set_master_view(app, true);
    let Some(st) = app.session.active() else { return };
    let Some(parent) = st.doc.parents.first() else { return };
    let Some(pg) = parent.pages.first() else { return };
    let (x0, x1) = (pg.x + 36.0, pg.x + pg.width - 36.0);
    let (y0, y1) = if header { (18.0, 50.0) } else { (pg.height - 50.0, pg.height - 18.0) };
    let label = if header { "pressroom-header" } else { "pressroom-footer" };
    let existing = app.run("object.findByLabel", json!({"label": label})).ok().and_then(|v| v.as_array().and_then(|a| a.first().cloned()));
    if let Some(id) = existing {
        let _ = app.run("selection.set", json!({"ids": [id]}));
        return;
    }
    if app.run("frame.create", json!({"spread": {"kind": "parent", "index": 0}, "rect": [x0, y0, x1, y1], "content": "text", "caret": true})).is_ok() {
        let _ = app.run("object.label", json!({"label": label}));
    }
}

/// Page Background: a filled box behind everything on the master page, or none.
fn set_background(app: &mut DesignApp, swatch: Option<String>) {
    let label = "pressroom-background";
    let ids = app.run("object.findByLabel", json!({"label": label})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if !ids.is_empty() {
        let _ = app.run("selection.set", json!({"ids": ids}));
        let _ = app.run("edit.clear", json!({}));
    }
    let Some(swatch) = swatch else { return };
    let Some(st) = app.session.active() else { return };
    let Some(parent) = st.doc.parents.first() else { return };
    let rects: Vec<[f64; 4]> = parent.pages.iter().map(|p| [p.x, 0.0, p.x + p.width, p.height]).collect();
    for rect in rects {
        if app.run("frame.create", json!({"spread": {"kind": "parent", "index": 0}, "rect": rect, "content": "unassigned"})).is_ok() {
            let _ = app.run("object.fill", json!({"swatch": swatch}));
            let _ = app.run("object.stroke", json!({"swatch": "[None]"}));
            let _ = app.run("object.sendToBack", json!({}));
            let _ = app.run("object.label", json!({"label": label}));
        }
    }
    let _ = app.run("edit.deselectAll", json!({}));
}

/// Create Link: the next text box clicked continues this one's text.
fn start_link(app: &mut DesignApp) {
    let Some(from) = selected_text_frame(app) else {
        app.status("Select the text box whose text should continue, then click Create Link.");
        return;
    };
    if let Some(ctx) = app.egui_ctx.clone() {
        update(&ctx, |s| s.link_from = Some(from.0));
    }
    let _ = app.run("tool.select", json!({"tool": "selection"}));
    app.status("Click the empty text box the text should continue in (Esc cancels).");
}

/// After Create Link: the first other text box selected receives the overflow.
fn finish_link(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(from) = state(ctx).link_from else { return };
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        update(ctx, |s| s.link_from = None);
        app.status("");
        return;
    }
    // The box clicked: selected, or the one the click put the text cursor in.
    let Some(to) = selected_text_frame(app).or_else(|| app.session.active().and_then(|d| d.selection.items.first().copied())) else { return };
    if to.0 == from {
        return;
    }
    // The clicked box takes the text from the start: no stray cursor left in it.
    let _ = app.run("selection.set", json!({"ids": [to.0]}));
    update(ctx, |s| s.link_from = None);
    // As in Publisher, text only flows into an empty text box.
    let has_text = app.session.active().is_some_and(|st| {
        st.doc.item(to).and_then(|it| it.text_frame()).and_then(|tf| st.doc.story(tf.story)).is_some_and(|s| !s.text.trim().is_empty())
    });
    if has_text {
        app.status("The text box you link to must be empty. Delete its text, or draw a new text box, then click Create Link again.");
        return;
    }
    match app.run("text.thread", json!({"from": from, "to": to.0})) {
        Ok(_) => app.status("Text boxes linked: the text continues in this box."),
        Err(e) => app.status(format!("Couldn't link the text boxes: {e}")),
    }
}

/// Previous / Next: select the box before or after this one in its chain of linked text boxes.
fn follow_link(app: &mut DesignApp, next: bool) {
    let Some(st) = app.session.active() else { return };
    let Some(id) = selected_text_frame(app) else { return };
    let Some(Content::Text(tf)) = st.doc.item(id).map(|i| &i.content) else { return };
    let Some(story) = st.doc.story(tf.story) else { return };
    let Some(k) = story.frames.iter().position(|f| *f == id) else { return };
    let target = if next { story.frames.get(k + 1) } else { k.checked_sub(1).and_then(|k| story.frames.get(k)) };
    if let Some(t) = target.copied() {
        let _ = app.run("selection.set", json!({"ids": [t.0]}));
        let _ = app.run("view.fitSelection", json!({}));
    }
}

fn fit_page_width(app: &mut DesignApp) {
    let Some(ctx) = app.egui_ctx.clone() else { return };
    let w = f64::from(state(&ctx).canvas_w);
    let page_w = app.session.active().and_then(|d| d.doc.page(crate::canvas::current_page(app).unwrap_or(0)).map(|p| p.width)).unwrap_or(612.0);
    if w > 40.0 && page_w > 0.0 {
        ribbon::act_with(app, "view.zoom", json!({"zoom": (w - 40.0) / page_w}));
    }
}

// ───────────────────────────── the window ─────────────────────────────

/// The bars around the document area: title bar, tabs and ribbon on top, status bar below.
pub fn chrome(app: &mut DesignApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    // Publisher's light look: dialogs, menus and the canvas chrome follow it.
    if app.ui.brightness != crate::theme::Brightness::Light {
        app.ui.brightness = crate::theme::Brightness::Light;
        app.restyle = true;
    }
    // Publisher has no floating bar over the selection: the ribbon's Format tab is where its
    // controls live.
    app.ui.task_bar = false;
    let mut s = state(&ctx);
    // No document: the File screen is the start screen.
    if app.session.active().is_none() && s.backstage.is_none() {
        s.backstage = Some(Backstage::New);
    }
    let c = context(app);
    if c != s.context {
        // A new kind of selection brings its Format tab forward, as Publisher does when a box is
        // inserted; losing it goes back to Home.
        if c == Context::None && s.tab == Tab::Format {
            s.tab = Tab::Home;
        }
        s.context = c;
    }
    ctx.data_mut(|d| d.insert_temp(state_id(), s.clone()));
    finish_link(app, &ctx);
    title_bar(app, ui, &s);
    if s.backstage.is_none() {
        tabs_and_ribbon(app, ui, &s);
        status_bar(app, ui);
    }
}

/// The area below the ribbon: the File screen, or the Page Navigation pane and the canvas.
pub fn body(app: &mut DesignApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let s = state(&ctx);
    if let Some(page) = s.backstage {
        backstage::show(app, ui, page);
        return;
    }
    if app.session.active().is_none() {
        backstage::show(app, ui, Backstage::New);
        return;
    }
    if !s.nav_hidden {
        nav::pane(app, ui, s.nav_narrow);
    }
    let w = ui.available_width();
    if (w - s.canvas_w).abs() > 0.5 {
        update(&ctx, |s| s.canvas_w = w);
    }
    egui::Frame::NONE.fill(PR::SCRATCH).show(ui, |ui| {
        app.canvas_area(ui, false);
    });
}

fn title_bar(app: &mut DesignApp, ui: &mut Ui, s: &State) {
    egui::Panel::top("pr_title").exact_size(TITLE_H).frame(egui::Frame::NONE.fill(PR::TEAL)).show(ui, |ui| {
        let full = ui.max_rect();
        let caption_w = if app.custom_titlebar { CAPTION_W * 3.0 } else { 0.0 };
        if app.custom_titlebar {
            let drag = Rect::from_min_max(full.min, pos2(full.max.x - caption_w, full.max.y));
            let resp = ui.interact(drag, egui::Id::new("pr_title_drag"), Sense::click_and_drag());
            if resp.double_clicked() {
                let on = !ui.input(|i| i.viewport().maximized.unwrap_or(false));
                let _ = app.run("window.maximize", json!({"on": on}));
            } else if resp.drag_started_by(egui::PointerButton::Primary) {
                let _ = app.run("window.drag", json!({}));
            }
            app.flush_window_commands(ui.ctx());
        }
        // App mark and the Quick Access Toolbar.
        let mut x = full.min.x + 8.0;
        let mark = Rect::from_min_size(pos2(x, full.center().y - 9.0), vec2(18.0, 18.0));
        ui.painter().rect_filled(mark, 2.0, Color32::WHITE);
        ui.painter().text(mark.center(), egui::Align2::CENTER_CENTER, "P", crate::theme::semibold(13.0), PR::TEAL);
        ui.painter().line_segment([mark.center_bottom() + vec2(-6.0, -3.0), mark.center_bottom() + vec2(6.0, -3.0)], Stroke::new(1.0, PR::TEAL));
        x += 30.0;
        let doc = app.session.active().is_some();
        for (icon, tip, cmd) in [("save", "Save (Ctrl+S)", "app.save"), ("undo", "Undo (Ctrl+Z)", "edit.undo"), ("redo", "Redo (Ctrl+Y)", "edit.redo")] {
            let r = Rect::from_min_size(pos2(x, full.min.y + 3.0), vec2(26.0, TITLE_H - 6.0));
            let enabled = doc && ribbon::on(app, cmd);
            let resp = ui.interact(r, egui::Id::new(("pr_qat", cmd)), if enabled { Sense::click() } else { Sense::hover() });
            if resp.hovered() && enabled {
                ui.painter().rect_filled(r, 0.0, PR::TEAL_HOVER);
            }
            art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, enabled);
            if resp.on_hover_text(tip).clicked() {
                ribbon::act(app, cmd);
            }
            x += 27.0;
        }
        // Title, centred.
        let title = match app.session.active() {
            Some(d) => format!("{} - PressRoom", d.title()),
            None => "PressRoom".to_string(),
        };
        ui.painter().text(full.center(), egui::Align2::CENTER_CENTER, title, FontId::proportional(12.0), Color32::WHITE);
        // The contextual tab's tool name sits over its tab.
        if s.context != Context::None && s.backstage.is_none() {
            let tab_x: Option<(f32, f32)> = ui.data(|d| d.get_temp(egui::Id::new("pr_format_tab_x")));
            if let Some((x0, x1)) = tab_x {
                let r = Rect::from_min_max(pos2(x0, full.min.y + 4.0), pos2(x1.max(x0 + 100.0), full.max.y));
                ui.painter().rect_filled(r, 0.0, PR::TEAL_DARK);
                ui.painter().rect_filled(Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 3.0)), 0.0, PR::CONTEXT);
                ui.painter().text(r.center() + vec2(0.0, 1.5), egui::Align2::CENTER_CENTER, s.context.title(), FontId::proportional(11.5), Color32::WHITE);
            }
        }
        if app.custom_titlebar {
            caption_buttons(app, ui, Rect::from_min_max(pos2(full.max.x - caption_w, full.min.y), full.max));
        }
    });
}

fn caption_buttons(app: &mut DesignApp, ui: &mut Ui, area: Rect) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    for (k, (command, label)) in
        [("window.minimize", "Minimize"), ("window.maximize", if maximized { "Restore Down" } else { "Maximize" }), ("window.close", "Close")]
            .into_iter()
            .enumerate()
    {
        let r = Rect::from_min_size(pos2(area.min.x + k as f32 * CAPTION_W, area.min.y), vec2(CAPTION_W, area.height()));
        let resp = ui.interact(r, egui::Id::new(("pr_caption", command)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, if command == "window.close" { Color32::from_rgb(232, 17, 35) } else { PR::TEAL_HOVER });
        }
        let s = Stroke::new(1.0, Color32::WHITE);
        let i = Rect::from_center_size(r.center(), vec2(10.0, 10.0));
        match command {
            "window.close" => {
                ui.painter().line_segment([i.left_top(), i.right_bottom()], s);
                ui.painter().line_segment([i.right_top(), i.left_bottom()], s);
            }
            "window.maximize" if maximized => {
                let back = i.translate(vec2(2.0, -2.0));
                ui.painter().line_segment([back.left_top(), back.right_top()], s);
                ui.painter().line_segment([back.right_top(), back.right_bottom()], s);
                ui.painter().rect_stroke(i, 0.0, s, egui::StrokeKind::Inside);
            }
            "window.maximize" => {
                ui.painter().rect_stroke(i, 0.0, s, egui::StrokeKind::Inside);
            }
            _ => {
                ui.painter().line_segment([i.left_center(), i.right_center()], s);
            }
        }
        if resp.on_hover_text(label).clicked() {
            let params = if command == "window.maximize" { json!({"on": !maximized}) } else { json!({}) };
            let _ = app.run(command, params);
            app.flush_window_commands(ui.ctx());
        }
    }
}

fn tabs_and_ribbon(app: &mut DesignApp, ui: &mut Ui, s: &State) {
    let height = TABS_H + if s.collapsed { 0.0 } else { ribbon::BODY_H };
    egui::Panel::top("pr_ribbon").exact_size(height).frame(egui::Frame::NONE.fill(PR::RIBBON)).show(ui, |ui| {
        let full = ui.max_rect();
        let strip = Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + TABS_H));
        ui.painter().rect_filled(strip, 0.0, PR::TEAL);
        let mut tabs = vec![
            (Tab::File, "File"),
            (Tab::Home, "Home"),
            (Tab::Insert, "Insert"),
            (Tab::PageDesign, "Page Design"),
            (Tab::Mailings, "Mailings"),
            (Tab::Review, "Review"),
            (Tab::View, "View"),
        ];
        if s.context != Context::None {
            tabs.push((Tab::Format, if s.context == Context::Table { "Layout" } else { "Format" }));
        }
        let font = FontId::proportional(12.0);
        let mut x = strip.min.x + 2.0;
        let mut chosen = None;
        let mut toggle = false;
        for (tab, name) in tabs {
            let w = ui.painter().layout_no_wrap(name.to_string(), font.clone(), Color32::WHITE).size().x + if tab == Tab::File { 34.0 } else { 22.0 };
            let r = Rect::from_min_size(pos2(x, strip.min.y), vec2(w, TABS_H));
            let resp = ui.interact(r, egui::Id::new(("pr_tab", name)), Sense::click());
            let current = s.tab == tab && !s.collapsed;
            if tab == Tab::File {
                ui.painter().rect_filled(r, 0.0, if resp.hovered() { PR::TEAL_HOVER } else { PR::TEAL_DARK });
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, name, font.clone(), Color32::WHITE);
            } else if current {
                ui.painter().rect_filled(r.shrink2(vec2(0.0, 0.0)).with_min_y(r.min.y + 2.0), 0.0, PR::RIBBON);
                ui.painter().text(r.center() + vec2(0.0, 1.0), egui::Align2::CENTER_CENTER, name, font.clone(), PR::TEAL);
            } else {
                if resp.hovered() {
                    ui.painter().rect_filled(r.with_min_y(r.min.y + 2.0), 0.0, PR::TEAL_HOVER);
                }
                ui.painter().text(r.center() + vec2(0.0, 1.0), egui::Align2::CENTER_CENTER, name, font.clone(), Color32::WHITE);
            }
            if tab == Tab::Format {
                ui.data_mut(|d| d.insert_temp(egui::Id::new("pr_format_tab_x"), (r.min.x, r.max.x)));
                ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(2.0, PR::CONTEXT));
            }
            if resp.double_clicked() && tab != Tab::File {
                toggle = true;
            } else if resp.clicked() {
                chosen = Some(tab);
            }
            x += w + 1.0;
        }
        // "Tell me what you want to do" lives here in Publisher; PressRoom's search is the same idea.
        if let Some(tab) = chosen {
            if tab == Tab::File {
                update(ui.ctx(), |st| st.backstage = Some(Backstage::Info));
            } else {
                update(ui.ctx(), |st| {
                    st.tab = tab;
                    st.collapsed = false;
                });
            }
        }
        if toggle {
            update(ui.ctx(), |st| st.collapsed = !st.collapsed);
        }
        if s.collapsed {
            return;
        }
        let body = Rect::from_min_max(pos2(full.min.x, strip.max.y), full.max);
        ui.painter().line_segment([body.left_bottom() - vec2(0.0, 0.5), body.right_bottom() - vec2(0.0, 0.5)], Stroke::new(1.0, PR::RIBBON_EDGE));
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(body.shrink2(vec2(0.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Min)));
        inner.style_mut().visuals = egui::Visuals::light();
        egui::ScrollArea::horizontal().id_salt("pr_ribbon_scroll").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
            &mut inner,
            |ui| {
                let tab = if s.tab == Tab::Format && s.context == Context::None { Tab::Home } else { s.tab };
                ribbon::body(app, ui, tab, s.context);
            },
        );
        // Collapse the Ribbon (Ctrl+F1).
        let cr = Rect::from_min_size(pos2(full.max.x - 24.0, full.max.y - 20.0), vec2(18.0, 16.0));
        let resp = ui.interact(cr, egui::Id::new("pr_collapse"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(cr, 0.0, PR::HOVER);
        }
        let c = cr.center();
        ui.painter().line_segment([c + vec2(-4.0, 2.0), c + vec2(0.0, -2.0)], Stroke::new(1.2, PR::GROUP_LABEL));
        ui.painter().line_segment([c + vec2(0.0, -2.0), c + vec2(4.0, 2.0)], Stroke::new(1.2, PR::GROUP_LABEL));
        if resp.on_hover_text("Collapse the Ribbon (Ctrl+F1)").clicked() {
            update(ui.ctx(), |st| st.collapsed = true);
        }
    });
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F1)) {
        update(ui.ctx(), |st| st.collapsed = !st.collapsed);
    }
}

fn status_bar(app: &mut DesignApp, ui: &mut Ui) {
    egui::Panel::bottom("pr_status").exact_size(STATUS_H).frame(egui::Frame::NONE.fill(PR::RIBBON)).show(ui, |ui| {
        let full = ui.max_rect();
        ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, PR::RIBBON_EDGE));
        let font = FontId::proportional(11.5);
        let Some(n) = app.session.active().map(|d| d.doc.page_count()) else { return };
        let cur = crate::canvas::current_page(app).unwrap_or(0);
        // Page: 1 of 4
        let pr = Rect::from_min_size(pos2(full.min.x + 8.0, full.min.y + 2.0), vec2(96.0, STATUS_H - 4.0));
        let resp = ui.interact(pr, egui::Id::new("pr_status_page"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(pr, 0.0, PR::HOVER);
        }
        art::paint(ui.painter(), Rect::from_min_size(pr.min + vec2(2.0, 1.5), vec2(15.0, 15.0)), "single-page", true);
        ui.painter().text(pr.min + vec2(20.0, pr.height() / 2.0), egui::Align2::LEFT_CENTER, format!("Page: {} of {}", cur + 1, n), font.clone(), PR::TEXT);
        if resp.on_hover_text("Page Navigation").clicked() {
            let hidden = nav_hidden(ui.ctx());
            set_nav_hidden(ui.ctx(), !hidden);
        }
        if !app.ui.status.is_empty() {
            ui.painter().text(pos2(pr.max.x + 18.0, full.center().y), egui::Align2::LEFT_CENTER, &app.ui.status, font.clone(), PR::GROUP_LABEL);
        }
        // Right: view buttons, zoom slider, percentage.
        let zoom = app.view().map(|v| v.zoom).unwrap_or(1.0);
        let pct_r = Rect::from_min_size(pos2(full.max.x - 52.0, full.min.y + 2.0), vec2(46.0, STATUS_H - 4.0));
        let resp = ui.interact(pct_r, egui::Id::new("pr_zoom_pct"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(pct_r, 0.0, PR::HOVER);
        }
        ui.painter().text(pct_r.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", zoom * 100.0), font.clone(), PR::TEXT);
        egui::Popup::menu(&resp).show(|ui| {
            for p in [400.0, 200.0, 150.0, 100.0, 75.0, 50.0, 33.0, 25.0] {
                if ui.button(format!("{p}%")).clicked() {
                    ribbon::act_with(app, "view.zoom", json!({"zoom": p / 100.0}));
                    ui.close();
                }
            }
            if ui.button("Whole Page").clicked() {
                ribbon::act(app, "view.fitPage");
                ui.close();
            }
        });
        // Slider: 10%–400% on a log scale, 100% in the middle.
        let sr = Rect::from_min_max(pos2(pct_r.min.x - 140.0, full.min.y), pos2(pct_r.min.x - 22.0, full.max.y));
        let to_t = |z: f64| ((z.max(0.1).ln() - 0.1f64.ln()) / (4.0f64.ln() - 0.1f64.ln())).clamp(0.0, 1.0) as f32;
        let from_t = |t: f32| (0.1f64.ln() + f64::from(t) * (4.0f64.ln() - 0.1f64.ln())).exp();
        let track = Rect::from_min_max(pos2(sr.min.x, sr.center().y - 0.5), pos2(sr.max.x, sr.center().y + 0.5));
        ui.painter().rect_filled(track, 0.0, PR::GROUP_LABEL);
        ui.painter().line_segment([pos2(sr.center().x, sr.center().y - 4.0), pos2(sr.center().x, sr.center().y + 4.0)], Stroke::new(1.0, PR::GROUP_LABEL));
        let resp = ui.interact(sr, egui::Id::new("pr_zoom_slider"), Sense::click_and_drag());
        let t = to_t(zoom);
        let knob = Rect::from_center_size(pos2(sr.min.x + t * sr.width(), sr.center().y), vec2(4.0, 12.0));
        ui.painter().rect_filled(knob, 0.0, PR::TEXT);
        if (resp.dragged() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let t = ((p.x - sr.min.x) / sr.width()).clamp(0.0, 1.0);
            let z = from_t(t);
            let z = if (z - 1.0).abs() < 0.06 { 1.0 } else { z };
            let _ = app.run("view.zoom", json!({"zoom": z}));
        }
        for (dx, minus) in [(-12.0, true), (sr.width() + 12.0, false)] {
            let c = pos2(sr.min.x + dx, sr.center().y);
            let br = Rect::from_center_size(c, vec2(16.0, 16.0));
            let b = ui.interact(br, egui::Id::new(("pr_zoom_step", minus)), Sense::click());
            if b.hovered() {
                ui.painter().rect_filled(br, 0.0, PR::HOVER);
            }
            ui.painter().line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.2, PR::TEXT));
            if !minus {
                ui.painter().line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.2, PR::TEXT));
            }
            if b.clicked() {
                ribbon::act(app, if minus { "view.zoomOut" } else { "view.zoomIn" });
            }
        }
        // Single Page / Two-Page Spread.
        let spread = two_page(ui.ctx());
        for (k, (icon, tip, on)) in [("single-page", "Single Page", !spread), ("two-page", "Two-Page Spread", spread)].into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(sr.min.x - 76.0 + k as f32 * 26.0, full.min.y + 2.0), vec2(24.0, STATUS_H - 4.0));
            let b = ui.interact(r, egui::Id::new(("pr_viewbtn", icon)), Sense::click());
            if on {
                ui.painter().rect_filled(r, 0.0, PR::CHECKED);
            } else if b.hovered() {
                ui.painter().rect_filled(r, 0.0, PR::HOVER);
            }
            art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(15.0, 15.0)), icon, true);
            if b.on_hover_text(tip).clicked() {
                let ctx = ui.ctx().clone();
                set_two_page(app, &ctx, k == 1);
            }
        }
    });
}

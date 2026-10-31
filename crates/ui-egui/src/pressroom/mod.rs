//! PressRoom: a desktop-publishing window laid out the way a Microsoft Publisher user knows it —
//! a coloured title bar with the Quick Access Toolbar, the File screen, the ribbon tabs (Home,
//! Insert, Page Design, Mailings, Review, View, and Format when a box is selected), the Page
//! Navigation pane, and a status bar with the page count and zoom slider — around DesignCraft's
//! layout engine. All art is original; nothing is taken from Microsoft's product.

mod art;
mod backstage;
mod blocks;
mod fontnames;
mod keys;
mod mica;
mod nav;
pub(crate) mod pictures;
pub mod pubimport;
mod ribbon;
mod scheme;
mod splash;
mod templates;
#[cfg(test)]
mod tests;
mod tips;
pub mod update;
mod wordart;

use designcraft_doc::Content;
use egui::{Color32, FontId, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::DesignApp;

pub use backstage::Backstage;
pub use keys::shortcuts;

/// Colours that are the same in every Office Theme: Publisher's teal accent and its shades.
#[allow(non_snake_case)]
pub mod PR {
    use egui::Color32;
    pub const TEAL: Color32 = Color32::from_rgb(7, 117, 104);
    pub const TEAL_DARK: Color32 = Color32::from_rgb(5, 92, 82);
    pub const TEAL_HOVER: Color32 = Color32::from_rgb(38, 138, 125);
    pub const CONTEXT: Color32 = Color32::from_rgb(232, 184, 76);
    /// Rounding of buttons' hover and pressed highlights.
    pub const ROUND: f32 = 4.0;
}

/// The window's colours in one Office Theme (File › Account), as Publisher's current (Microsoft
/// 365) look: a flat top, the ribbon on a rounded panel, Publisher's teal as the accent. White and
/// Colorful are light; Dark Gray and Black are Office's dark themes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pal {
    pub dark: bool,
    /// The top (title bar and tabs): its fill, text, hover and the selected tab's underline.
    pub top: Color32,
    pub top_text: Color32,
    pub top_hover: Color32,
    pub top_accent: Color32,
    /// The top is Publisher's teal (the Colorful theme).
    pub colorful: bool,
    /// The ribbon's surround.
    pub ribbon: Color32,
    /// The ribbon's rounded panel and its edge.
    pub card: Color32,
    pub card_edge: Color32,
    pub ribbon_edge: Color32,
    pub group_sep: Color32,
    pub group_label: Color32,
    pub text: Color32,
    pub disabled: Color32,
    pub hover: Color32,
    pub hover_border: Color32,
    pub pressed: Color32,
    /// A button that's on (Bold, the alignment in force): a tint of the accent.
    pub checked: Color32,
    /// Around the page.
    pub scratch: Color32,
    /// The Pages pane.
    pub pane: Color32,
    pub status: Color32,
    /// The page shown in the Pages pane, and other selections.
    pub select: Color32,
    /// The File screen's page.
    pub page: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub const WHITE: Pal = Pal {
    dark: false,
    top: rgb(243, 243, 243),
    top_text: rgb(36, 36, 36),
    top_hover: rgb(229, 229, 229),
    top_accent: PR::TEAL,
    colorful: false,
    ribbon: rgb(243, 243, 243),
    card: Color32::WHITE,
    card_edge: rgb(226, 226, 226),
    ribbon_edge: rgb(224, 224, 224),
    group_sep: rgb(225, 225, 225),
    group_label: rgb(97, 97, 97),
    text: rgb(36, 36, 36),
    disabled: rgb(168, 168, 168),
    hover: rgb(235, 235, 235),
    hover_border: rgb(209, 209, 209),
    pressed: rgb(220, 220, 220),
    checked: rgb(220, 236, 233),
    scratch: rgb(224, 224, 224),
    pane: rgb(243, 243, 243),
    status: rgb(243, 243, 243),
    select: PR::TEAL,
    page: Color32::WHITE,
};

pub const COLORFUL: Pal =
    Pal { top: PR::TEAL, top_text: Color32::WHITE, top_hover: PR::TEAL_HOVER, top_accent: Color32::WHITE, colorful: true, ..WHITE };

pub const DARK_GRAY: Pal = Pal {
    dark: true,
    top: rgb(51, 51, 51),
    top_text: rgb(245, 245, 245),
    top_hover: rgb(78, 78, 78),
    top_accent: rgb(96, 190, 176),
    colorful: false,
    ribbon: rgb(51, 51, 51),
    card: rgb(68, 68, 68),
    card_edge: rgb(86, 86, 86),
    ribbon_edge: rgb(75, 75, 75),
    group_sep: rgb(90, 90, 90),
    group_label: rgb(208, 208, 208),
    text: rgb(245, 245, 245),
    disabled: rgb(140, 140, 140),
    hover: rgb(86, 86, 86),
    hover_border: rgb(110, 110, 110),
    pressed: rgb(100, 100, 100),
    checked: rgb(40, 92, 84),
    scratch: rgb(96, 96, 96),
    pane: rgb(60, 60, 60),
    status: rgb(45, 45, 45),
    select: rgb(64, 160, 146),
    page: rgb(58, 58, 58),
};

/// As Publisher's Black theme measures: a near-black top, the ribbon on a dark grey panel.
pub const BLACK: Pal = Pal {
    dark: true,
    top: rgb(10, 12, 14),
    top_text: Color32::WHITE,
    top_hover: rgb(45, 45, 45),
    top_accent: rgb(92, 184, 170),
    colorful: false,
    ribbon: rgb(10, 12, 14),
    card: rgb(41, 41, 41),
    card_edge: rgb(56, 56, 56),
    ribbon_edge: rgb(50, 50, 50),
    group_sep: rgb(64, 64, 64),
    group_label: rgb(214, 214, 214),
    text: Color32::WHITE,
    disabled: rgb(120, 120, 120),
    hover: rgb(61, 61, 61),
    hover_border: rgb(90, 90, 90),
    pressed: rgb(77, 77, 77),
    checked: rgb(35, 72, 66),
    scratch: rgb(38, 38, 38),
    pane: rgb(38, 38, 38),
    status: rgb(20, 20, 20),
    select: rgb(53, 126, 117),
    page: rgb(32, 32, 32),
};

/// The theme the window is drawn in this frame: 0 White, 1 Colorful, 2 Dark Gray, 3 Black.
static THEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The current Office Theme's colours.
pub fn pal() -> &'static Pal {
    match THEME.load(std::sync::atomic::Ordering::Relaxed) {
        1 => &COLORFUL,
        2 => &DARK_GRAY,
        3 => &BLACK,
        _ => &WHITE,
    }
}

/// The Office Theme setting ("system", "white", "colorful", "darkgray", "black") as the theme to
/// draw: "Use system setting" (Office's default) is White on a light Windows and Black on a dark one.
pub fn theme_index(setting: &str, system_dark: bool) -> u8 {
    match setting {
        "white" => 0,
        "colorful" => 1,
        "darkgray" => 2,
        "black" => 3,
        _ if system_dark => 3,
        _ => 0,
    }
}

/// Draw this frame in the chosen theme. Whether it changed (menus and dialogs are restyled then).
fn set_theme(app: &DesignApp, ctx: &egui::Context) -> bool {
    let i = theme_index(&app.ui.pressroom_theme, ctx.system_theme() == Some(egui::Theme::Dark));
    THEME.swap(i, std::sync::atomic::Ordering::Relaxed) != i
}

/// Menus, dropdowns and dialogs as current Office draws them, on top of DesignCraft's theme
/// (`theme::apply`, InDesign's look): rounded corners and soft shadows, roomier rows and larger
/// text, filled highlights instead of outlines, plain bordered buttons, and the accent's tint for
/// what's selected.
pub fn style(ctx: &egui::Context) {
    let p = *pal();
    let mut v = ctx.global_style().visuals.clone();
    let round = egui::CornerRadius::same(4);
    v.window_corner_radius = egui::CornerRadius::same(8);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.card_edge);
    v.popup_shadow = egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(if p.dark { 110 } else { 40 }) };
    v.window_shadow = egui::epaint::Shadow { offset: [0, 10], blur: 32, spread: 0, color: Color32::from_black_alpha(if p.dark { 120 } else { 48 }) };
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = round;
    }
    let button = if p.dark { rgb(58, 58, 58) } else { rgb(253, 253, 253) };
    v.widgets.inactive.weak_bg_fill = button;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.hover_border);
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.hover_border);
    v.widgets.active.weak_bg_fill = p.pressed;
    v.widgets.active.bg_fill = p.pressed;
    v.widgets.open.weak_bg_fill = p.hover;
    v.extreme_bg_color = if p.dark { rgb(30, 30, 30) } else { Color32::WHITE };
    v.selection.bg_fill = p.checked;
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.override_text_color = Some(p.text);
    v.hyperlink_color = if p.dark { p.top_accent } else { PR::TEAL };
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(10.5));
        s.spacing.button_padding = vec2(10.0, 4.0);
        s.spacing.interact_size = vec2(24.0, 24.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.item_spacing = vec2(8.0, 4.0);
        // Gentle motion: highlights and panels ease in rather than snap.
        s.animation_time = 0.12;
        // Windows 11's scroll bars: a slim line over the edge that widens when pointed at.
        s.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 8.0,
            floating_width: 3.0,
            bar_inner_margin: 2.0,
            bar_outer_margin: 1.0,
            handle_min_length: 20.0,
            dormant_handle_opacity: 0.45,
            active_handle_opacity: 0.6,
            interact_handle_opacity: 0.9,
            dormant_background_opacity: 0.0,
            active_background_opacity: 0.0,
            interact_background_opacity: 0.35,
            foreground_color: false,
            ..egui::style::ScrollStyle::floating()
        };
    });
}

/// The top of the window (title bar and tabs) in the chosen Office Theme.
#[derive(Clone, Copy)]
struct Top {
    bg: Color32,
    text: Color32,
    hover: Color32,
    /// The selected tab's underline.
    accent: Color32,
    colorful: bool,
}

fn top(_app: &DesignApp) -> Top {
    let p = pal();
    // Hover as a see-through shade, so it darkens (or lightens) the Mica wash under it evenly.
    let hover = if p.colorful { p.top_hover } else { shade(p.top, p.top_hover) };
    Top { bg: p.top, text: p.top_text, hover, accent: p.top_accent, colorful: p.colorful }
}

/// The see-through black or white that turns `base` into `to` when laid over it.
fn shade(base: Color32, to: Color32) -> Color32 {
    let (b, t) = (f32::from(base.r()), f32::from(to.r()));
    if t >= b {
        let room = (255.0 - b).max(1.0);
        Color32::from_white_alpha(((t - b) / room * 255.0).round().clamp(0.0, 255.0) as u8)
    } else {
        Color32::from_black_alpha(((b - t) / b.max(1.0) * 255.0).round().clamp(0.0, 255.0) as u8)
    }
}

const TITLE_H: f32 = 31.0;
const TABS_H: f32 = 30.0;
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
    Help,
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
    /// The publication in front last frame (its session uid).
    shown_doc: Option<u64>,
}

fn state_id() -> egui::Id {
    egui::Id::unique("pressroom_state")
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
    // Selected cells, or the text cursor in one: Table Tools, as in Publisher.
    if st.selection.cells.is_some() || st.selection.text.as_ref().is_some_and(|t| t.cell.is_some()) {
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

/// Edit WordArt Text (Insert › WordArt), drawn and confirmed by the dialogs module.
pub const WORDART: &str = wordart::DIALOG;

pub(crate) fn wordart_dialog(app: &DesignApp, ui: &mut Ui, d: &mut crate::dialogs::Dialog) {
    wordart::dialog(app, ui, d);
}

pub(crate) fn wordart_insert(app: &mut DesignApp, d: &crate::dialogs::Dialog) -> Result<Value, String> {
    wordart::insert(app, d)
}

/// Character formatting chosen in the Font group with nothing selected: Publisher keeps the group
/// working then, and the next text box drawn takes it.
static NEXT_TEXT: std::sync::Mutex<Option<serde_json::Map<String, Value>>> = std::sync::Mutex::new(None);

/// Adds `attrs` to the formatting the next text box takes.
pub(super) fn set_next_text(attrs: &Value) {
    let mut g = NEXT_TEXT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let m = g.get_or_insert_with(serde_json::Map::new);
    if let Some(o) = attrs.as_object() {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
}

/// The formatting waiting for the next text box.
pub(super) fn next_text() -> Option<serde_json::Map<String, Value>> {
    NEXT_TEXT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// A caret in an empty text box (one just drawn): it takes the waiting formatting, once.
fn apply_next_text(app: &mut DesignApp, ctx: &egui::Context) {
    let Some(attrs) = next_text() else { return };
    // Not while the box is still being drawn: the drag's own end places the caret afresh.
    if ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    let empty_caret = app.session.active().is_some_and(|st| {
        st.interaction.is_none()
            && st.selection.text.as_ref().is_some_and(|t| t.cell.is_none() && st.doc.story(t.story).is_some_and(|s| s.text.is_empty()))
    });
    if empty_caret {
        *NEXT_TEXT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        let _ = app.run("type.char", json!({"attrs": Value::Object(attrs)}));
    }
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
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
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
    if app.run("frame.create", json!({"spread": {"kind": "parent", "index": 0}, "rect": [x0, y0, x1, y1], "content": "text", "caret": true})).is_ok()
    {
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
    let changed = set_theme(app, &ctx);
    // Dialogs, menus and the canvas chrome follow the Office Theme: light, or dark for Dark Gray
    // and Black.
    let brightness = if pal().dark { crate::theme::Brightness::Dark } else { crate::theme::Brightness::Light };
    if app.ui.brightness != brightness || changed || !app.pressroom_styled {
        app.ui.brightness = brightness;
        app.restyle = true;
        app.pressroom_styled = true;
    }
    // Publisher has no floating bar over the selection: the ribbon's Format tab is where its
    // controls live.
    app.ui.task_bar = false;
    let mut s = state(&ctx);
    // A publication that comes to the front shows itself, however it was opened (double-clicked
    // in Explorer, Open with, a recent file): the File screen steps aside.
    let front = app.session.active().map(|d| d.uid);
    if front != s.shown_doc {
        if front.is_some() {
            s.backstage = None;
        }
        s.shown_doc = front;
    }
    // No document: the File screen is the start screen.
    if front.is_none() && s.backstage.is_none() {
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
    pubimport::poll(app, &ctx);
    apply_next_text(app, &ctx);
    title_bar(app, ui, &s);
    if s.backstage.is_none() {
        tabs_and_ribbon(app, ui, &s);
    }
    // An update to offer: under the ribbon, or under the title bar on the File screen.
    update::bar(app, ui);
    if s.backstage.is_none() {
        status_bar(app, ui);
    }
    splash::show(app, &ctx);
}

/// The area below the ribbon: the File screen, or the Page Navigation pane and the canvas.
pub fn body(app: &mut DesignApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let s = state(&ctx);
    // A Publisher file on its way in: say so over the window.
    if let Some(name) = pubimport::busy() {
        let r = ui.max_rect();
        let card = Rect::from_center_size(r.center(), vec2(420.0, 90.0));
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("pr_pubimport")));
        painter.add(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(50) }.as_shape(card, 8.0));
        painter.rect(card, 8.0, pal().card, Stroke::new(1.0, pal().card_edge), egui::StrokeKind::Inside);
        painter.text(
            card.center() - vec2(0.0, 14.0),
            egui::Align2::CENTER_CENTER,
            format!("Bringing in \u{201c}{name}\u{201d}"),
            FontId::proportional(15.0),
            pal().text,
        );
        painter.text(
            card.center() + vec2(0.0, 14.0),
            egui::Align2::CENTER_CENTER,
            match pubimport::progress() {
                Some((n, of)) => format!("Publisher is reading file {n} of {of}. Each can take a minute."),
                None => "Publisher is reading the file. This can take a minute.".to_string(),
            },
            FontId::proportional(12.0),
            pal().group_label,
        );
    }
    let shown = s.backstage.or(app.session.active().is_none().then_some(Backstage::New));
    let opened = egui::Id::unique("pr_backstage_in");
    if let Some(page) = shown {
        // The File screen glides in from the left and fades up, as Office's does.
        let t = ctx.animate_bool_with_time(opened, true, 0.16);
        let rect = ui.max_rect().translate(vec2(-(1.0 - t) * 36.0, 0.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.multiply_opacity(0.4 + 0.6 * t);
        backstage::show(app, &mut child, page);
        return;
    }
    // Closed: the next opening glides in again.
    ctx.animate_bool_with_time(opened, false, 0.0);
    if !s.nav_hidden {
        nav::pane(app, ui, s.nav_narrow);
    }
    let w = ui.available_width();
    if (w - s.canvas_w).abs() > 0.5 {
        update(&ctx, |s| s.canvas_w = w);
    }
    egui::Frame::NONE.fill(pal().scratch).show(ui, |ui| {
        app.canvas_area(ui, false);
    });
}

/// A Quick Access Toolbar command: its key (as saved), its name in the ▾ menu, tooltip, the icons
/// on the coloured and on the light top, and the command it runs.
struct Qat {
    key: &'static str,
    label: &'static str,
    tip: &'static str,
    icon: &'static str,
    icon_dark: &'static str,
    cmd: &'static str,
}

const QAT: [Qat; 9] = [
    Qat { key: "new", label: "New", tip: "New (Ctrl+N)", icon: "new", icon_dark: "new", cmd: "" },
    Qat { key: "open", label: "Open", tip: "Open (Ctrl+O)", icon: "open", icon_dark: "open", cmd: "app.openDialog" },
    Qat { key: "save", label: "Save", tip: "Save (Ctrl+S)", icon: "save", icon_dark: "save-dark", cmd: "app.save" },
    Qat {
        key: "quickPrint",
        label: "Quick Print",
        tip: "Quick Print: the whole publication on the printer",
        icon: "print",
        icon_dark: "print",
        cmd: "",
    },
    Qat {
        key: "printPreview",
        label: "Print Preview and Print",
        tip: "Print Preview and Print (Ctrl+P)",
        icon: "print",
        icon_dark: "print",
        cmd: "",
    },
    Qat { key: "spelling", label: "Spelling", tip: "Spelling (F7)", icon: "spelling", icon_dark: "spelling", cmd: "spelling.check" },
    Qat { key: "undo", label: "Undo", tip: "Undo (Ctrl+Z)", icon: "undo", icon_dark: "undo-ink", cmd: "edit.undo" },
    Qat { key: "redo", label: "Redo", tip: "Redo (Ctrl+Y)", icon: "redo", icon_dark: "redo-ink", cmd: "edit.redo" },
    Qat { key: "textBox", label: "Draw Text Box", tip: "Draw Text Box", icon: "textbox", icon_dark: "textbox", cmd: "" },
];

fn qat_run(app: &mut DesignApp, ctx: &egui::Context, key: &str) {
    match key {
        "new" => open_backstage(ctx, Backstage::New),
        "printPreview" => open_backstage(ctx, Backstage::Print),
        "quickPrint" => match app.run("file.print", json!({"copies": 1})) {
            Ok(_) => app.status("Sent to the printer."),
            Err(e) => app.status(format!("Couldn't print: {e}")),
        },
        "textBox" => start_draw(app, "type"),
        _ => {
            if let Some(q) = QAT.iter().find(|q| q.key == key) {
                ribbon::act(app, q.cmd);
            }
        }
    }
}

fn title_bar(app: &mut DesignApp, ui: &mut Ui, s: &State) {
    let top = top(app);
    egui::Panel::top("pr_title").exact_size(TITLE_H).frame(egui::Frame::NONE.fill(top.bg)).show(ui, |ui| {
        let full = ui.max_rect();
        mica::paint(ui.ctx(), ui.painter(), full, top.bg);
        let caption_w = if app.custom_titlebar { CAPTION_W * 3.0 } else { 0.0 };
        if app.custom_titlebar {
            let drag = Rect::from_min_max(full.min, pos2(full.max.x - caption_w, full.max.y));
            let resp = ui.interact(drag, egui::Id::unique("pr_title_drag"), Sense::click_and_drag());
            if resp.double_clicked() {
                let on = !ui.input(|i| i.viewport().maximized.unwrap_or(false));
                let _ = app.run("window.maximize", json!({"on": on}));
            } else if resp.drag_started_by(egui::PointerButton::Primary) {
                let _ = app.run("window.drag", json!({}));
            }
            app.flush_window_commands(ui.ctx());
        }
        // App mark and the Quick Access Toolbar.
        let mut x = full.min.x + 10.0;
        // PressRoom's mark: an open booklet on a rounded tile (teal, or white on the Colorful top).
        let mark = Rect::from_min_size(pos2(x, full.center().y - 9.0), vec2(18.0, 18.0));
        let (tile, page, leaf) =
            if top.colorful { (Color32::WHITE, PR::TEAL, PR::TEAL_DARK) } else { (PR::TEAL, Color32::WHITE, Color32::from_rgb(214, 232, 228)) };
        ui.painter().rect_filled(mark, 4.0, tile);
        let (c, l, r) = (mark.center().x, mark.min.x + 3.0, mark.max.x - 3.0);
        let (t, b) = (mark.min.y + 4.5, mark.max.y - 4.0);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![pos2(l, t), pos2(c - 0.5, t + 1.2), pos2(c - 0.5, b), pos2(l, b - 1.2)],
            leaf,
            Stroke::NONE,
        ));
        ui.painter().add(egui::Shape::convex_polygon(
            vec![pos2(c + 0.5, t + 1.2), pos2(r, t), pos2(r, b - 1.2), pos2(c + 0.5, b)],
            page,
            Stroke::NONE,
        ));
        x += 30.0;
        let doc = app.session.active().is_some();
        let shown = app.ui.pressroom_qat.clone();
        for q in QAT.iter().filter(|q| shown.iter().any(|s| s == q.key)) {
            let r = Rect::from_min_size(pos2(x, full.min.y + 3.0), vec2(28.0, TITLE_H - 6.0));
            let enabled = match q.key {
                "new" | "open" => true,
                "undo" | "redo" | "save" => doc && ribbon::on(app, q.cmd),
                _ => doc,
            };
            let resp = ui.interact(r, egui::Id::unique(("pr_qat", q.key)), if enabled { Sense::click() } else { Sense::hover() });
            if resp.hovered() && enabled {
                ui.painter().rect_filled(r, PR::ROUND, top.hover);
            }
            let icon = if top.colorful { q.icon } else { q.icon_dark };
            art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(18.0, 18.0)), icon, enabled);
            if resp.on_hover_text(q.tip).clicked() {
                qat_run(app, ui.ctx(), q.key);
            }
            x += 29.0;
        }
        // Customize Quick Access Toolbar ▾: which commands it shows.
        let r = Rect::from_min_size(pos2(x, full.min.y + 3.0), vec2(16.0, TITLE_H - 6.0));
        let resp = ui.interact(r, egui::Id::unique("pr_qat_more"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, PR::ROUND, top.hover);
        }
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "\u{25be}", FontId::proportional(11.0), top.text);
        let resp = resp.on_hover_text("Customize Quick Access Toolbar");
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(220.0);
            ui.label(egui::RichText::new("Customize Quick Access Toolbar").strong());
            ui.separator();
            for q in QAT.iter() {
                let mut on = app.ui.pressroom_qat.iter().any(|s| s == q.key);
                if ui.checkbox(&mut on, q.label).changed() {
                    if on {
                        app.ui.pressroom_qat.push(q.key.to_string());
                    } else {
                        app.ui.pressroom_qat.retain(|s| s != q.key);
                    }
                }
            }
        });
        x += 18.0;
        // Title, after the Quick Access Toolbar, as Publisher has it.
        let title = match app.session.active() {
            Some(d) => format!("{} - PressRoom", d.title()),
            None => "PressRoom".to_string(),
        };
        ui.painter().text(pos2(x + 14.0, full.center().y), egui::Align2::LEFT_CENTER, title, FontId::proportional(12.0), top.text);
        let _ = s;
        if app.custom_titlebar {
            caption_buttons(app, ui, Rect::from_min_max(pos2(full.max.x - caption_w, full.min.y), full.max), top);
        }
    });
}

fn caption_buttons(app: &mut DesignApp, ui: &mut Ui, area: Rect, top: Top) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    for (k, (command, label)) in
        [("window.minimize", "Minimize"), ("window.maximize", if maximized { "Restore Down" } else { "Maximize" }), ("window.close", "Close")]
            .into_iter()
            .enumerate()
    {
        let r = Rect::from_min_size(pos2(area.min.x + k as f32 * CAPTION_W, area.min.y), vec2(CAPTION_W, area.height()));
        let resp = ui.interact(r, egui::Id::unique(("pr_caption", command)), Sense::click());
        let close_hot = resp.hovered() && command == "window.close";
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, if close_hot { Color32::from_rgb(196, 43, 28) } else { top.hover });
        }
        let s = Stroke::new(1.0, if close_hot { Color32::WHITE } else { top.text });
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
    let top = top(app);
    let height = TABS_H + if s.collapsed { 0.0 } else { ribbon::BODY_H };
    egui::Panel::top("pr_ribbon").exact_size(height).frame(egui::Frame::NONE.fill(pal().ribbon)).show(ui, |ui| {
        let full = ui.max_rect();
        let strip = Rect::from_min_max(full.min, pos2(full.max.x, full.min.y + TABS_H));
        ui.painter().rect_filled(strip, 0.0, top.bg);
        // Windows 11's Mica behind the tabs and round the ribbon, continuing the title bar's.
        mica::paint(ui.ctx(), ui.painter(), strip, top.bg);
        let band = Rect::from_min_max(pos2(full.min.x, strip.max.y), full.max);
        if band.height() > 0.0 {
            mica::paint(ui.ctx(), ui.painter(), band, pal().ribbon);
        }
        let mut tabs = vec![
            (Tab::File, "File"),
            (Tab::Home, "Home"),
            (Tab::Insert, "Insert"),
            (Tab::PageDesign, "Page Design"),
            (Tab::Mailings, "Mailings"),
            (Tab::Review, "Review"),
            (Tab::View, "View"),
            (Tab::Help, "Help"),
        ];
        if s.context != Context::None {
            tabs.push((Tab::Format, if s.context == Context::Table { "Layout" } else { "Format" }));
        }
        let font = FontId::proportional(13.0);
        let mut x = strip.min.x + 6.0;
        let mut chosen = None;
        let mut toggle = false;
        let mut underline = None;
        for (tab, name) in tabs {
            let tw = ui.painter().layout_no_wrap(name.to_string(), font.clone(), top.text).size().x;
            let w = tw + 22.0;
            let r = Rect::from_min_size(pos2(x, strip.min.y + 3.0), vec2(w, TABS_H - 6.0));
            let resp = ui.interact(r, egui::Id::unique(("pr_tab", name)), Sense::click());
            let current = s.tab == tab && !s.collapsed && tab != Tab::File;
            // Named for screen readers (and tests): a tab, selected while its ribbon shows.
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, current, name));
            let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.1);
            if hover > 0.0 {
                ui.painter().rect_filled(r, PR::ROUND, top.hover.gamma_multiply(hover));
            }
            // The contextual tab is in the accent colour, as Office shows it.
            let color = if tab == Tab::Format && !top.colorful { PR::TEAL } else { top.text };
            let tab_font = if current { crate::theme::semibold(13.0) } else { font.clone() };
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, name, tab_font, color);
            let resp = if tab == Tab::Format { resp.on_hover_text(s.context.title()) } else { resp };
            if current {
                underline = Some((r.center().x, tw.max(16.0), r.max.y - 1.5));
            }
            if tab == Tab::Format {
                ui.data_mut(|d| d.insert_temp(egui::Id::unique("pr_format_tab_x"), (r.min.x, r.max.x)));
            }
            if resp.double_clicked() && tab != Tab::File {
                toggle = true;
            } else if resp.clicked() {
                chosen = Some(tab);
            }
            x += w + 2.0;
        }
        // The selected tab's underline, in the accent colour; it slides to a newly chosen tab.
        if let Some((cx, w, y)) = underline {
            let cx = ui.ctx().animate_value_with_time(egui::Id::unique("pr_tab_underline_x"), cx, 0.18);
            let w = ui.ctx().animate_value_with_time(egui::Id::unique("pr_tab_underline_w"), w, 0.18);
            ui.painter().rect_filled(Rect::from_center_size(pos2(cx, y), vec2(w, 3.0)), 1.5, top.accent);
        }
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
        // The ribbon's groups on a white rounded panel with a soft shadow.
        let body = Rect::from_min_max(pos2(full.min.x, strip.max.y), full.max);
        let card = Rect::from_min_max(body.min + vec2(6.0, 4.0), body.max - vec2(6.0, 5.0));
        let shadow = egui::epaint::Shadow { offset: [0, 1], blur: 4, spread: 0, color: Color32::from_black_alpha(22) };
        ui.painter().add(shadow.as_shape(card, 8.0));
        ui.painter().rect(card, 8.0, pal().card, Stroke::new(1.0, pal().card_edge), egui::StrokeKind::Inside);
        let mut inner =
            ui.new_child(egui::UiBuilder::new().max_rect(card.shrink2(vec2(2.0, 2.0))).layout(egui::Layout::left_to_right(egui::Align::Min)));
        inner.style_mut().visuals = inner.ctx().global_style().visuals.clone();
        inner.set_clip_rect(card.shrink(1.0));
        egui::ScrollArea::horizontal().id_salt("pr_ribbon_scroll").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
            &mut inner,
            |ui| {
                let tab = if s.tab == Tab::Format && s.context == Context::None { Tab::Home } else { s.tab };
                ribbon::body(app, ui, tab, s.context);
            },
        );
        // Collapse the Ribbon (Ctrl+F1), at the panel's bottom right.
        let cr = Rect::from_min_size(pos2(card.max.x - 26.0, card.max.y - 22.0), vec2(20.0, 18.0));
        let resp = ui.interact(cr, egui::Id::unique("pr_collapse"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(cr, PR::ROUND, pal().hover);
        }
        let c = cr.center();
        ui.painter().line_segment([c + vec2(-4.0, 2.0), c + vec2(0.0, -2.0)], Stroke::new(1.2, pal().group_label));
        ui.painter().line_segment([c + vec2(0.0, -2.0), c + vec2(4.0, 2.0)], Stroke::new(1.2, pal().group_label));
        if resp.on_hover_text("Collapse the Ribbon (Ctrl+F1)").clicked() {
            update(ui.ctx(), |st| st.collapsed = true);
        }
    });
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F1)) {
        update(ui.ctx(), |st| st.collapsed = !st.collapsed);
    }
}

fn status_bar(app: &mut DesignApp, ui: &mut Ui) {
    egui::Panel::bottom("pr_status").exact_size(STATUS_H).frame(egui::Frame::NONE.fill(pal().status)).show(ui, |ui| {
        let full = ui.max_rect();
        ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, pal().ribbon_edge));
        let font = FontId::proportional(11.5);
        let Some(n) = app.session.active().map(|d| d.doc.page_count()) else { return };
        let cur = crate::canvas::current_page(app).unwrap_or(0);
        // Page: 1 of 4
        let pr = Rect::from_min_size(pos2(full.min.x + 8.0, full.min.y + 2.0), vec2(96.0, STATUS_H - 4.0));
        let resp = ui.interact(pr, egui::Id::unique("pr_status_page"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(pr, PR::ROUND, pal().hover);
        }
        art::paint(ui.painter(), Rect::from_min_size(pr.min + vec2(2.0, 1.5), vec2(15.0, 15.0)), "single-page", true);
        ui.painter().text(
            pr.min + vec2(20.0, pr.height() / 2.0),
            egui::Align2::LEFT_CENTER,
            format!("Page: {} of {}", cur + 1, n),
            font.clone(),
            pal().text,
        );
        if resp.on_hover_text("Page Navigation").clicked() {
            let hidden = nav_hidden(ui.ctx());
            set_nav_hidden(ui.ctx(), !hidden);
        }
        if !app.ui.status.is_empty() {
            ui.painter().text(pos2(pr.max.x + 18.0, full.center().y), egui::Align2::LEFT_CENTER, &app.ui.status, font.clone(), pal().group_label);
        }
        // Right: view buttons, zoom slider, percentage, and Fit Page to Window.
        let zoom = app.view().map(|v| v.zoom).unwrap_or(1.0);
        let fit_r = Rect::from_min_size(pos2(full.max.x - 32.0, full.min.y + 2.0), vec2(26.0, STATUS_H - 4.0));
        let fit = ui.interact(fit_r, egui::Id::unique("pr_zoom_fit"), Sense::click());
        if fit.hovered() {
            ui.painter().rect_filled(fit_r, PR::ROUND, pal().hover);
        }
        let (fo, fi) = (Rect::from_center_size(fit_r.center(), vec2(14.0, 11.0)), Rect::from_center_size(fit_r.center(), vec2(6.0, 4.0)));
        ui.painter().rect_stroke(fo, 1.5, Stroke::new(1.1, pal().text), egui::StrokeKind::Inside);
        ui.painter().rect_filled(fi, 1.0, PR::TEAL);
        if fit.on_hover_text("Fit Page to Window").clicked() {
            ribbon::act(app, "view.fitPage");
        }
        let pct_r = Rect::from_min_size(pos2(fit_r.min.x - 48.0, full.min.y + 2.0), vec2(46.0, STATUS_H - 4.0));
        let resp = ui.interact(pct_r, egui::Id::unique("pr_zoom_pct"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(pct_r, PR::ROUND, pal().hover);
        }
        ui.painter().text(pct_r.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", zoom * 100.0), font.clone(), pal().text);
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
        ui.painter().rect_filled(track, 0.5, pal().group_label);
        ui.painter()
            .line_segment([pos2(sr.center().x, sr.center().y - 4.0), pos2(sr.center().x, sr.center().y + 4.0)], Stroke::new(1.0, pal().group_label));
        let resp = ui.interact(sr, egui::Id::unique("pr_zoom_slider"), Sense::click_and_drag());
        let t = to_t(zoom);
        let knob = Rect::from_center_size(pos2(sr.min.x + t * sr.width(), sr.center().y), vec2(5.0, 13.0));
        ui.painter().rect_filled(knob, 2.5, pal().text);
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
            let b = ui.interact(br, egui::Id::unique(("pr_zoom_step", minus)), Sense::click());
            if b.hovered() {
                ui.painter().rect_filled(br, PR::ROUND, pal().hover);
            }
            ui.painter().line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.2, pal().text));
            if !minus {
                ui.painter().line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.2, pal().text));
            }
            if b.clicked() {
                ribbon::act(app, if minus { "view.zoomOut" } else { "view.zoomIn" });
            }
        }
        // Single Page / Two-Page Spread.
        let spread = two_page(ui.ctx());
        for (k, (icon, tip, on)) in [("single-page", "Single Page", !spread), ("two-page", "Two-Page Spread", spread)].into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(sr.min.x - 76.0 + k as f32 * 26.0, full.min.y + 2.0), vec2(24.0, STATUS_H - 4.0));
            let b = ui.interact(r, egui::Id::unique(("pr_viewbtn", icon)), Sense::click());
            if on {
                ui.painter().rect_filled(r, PR::ROUND, pal().checked);
            } else if b.hovered() {
                ui.painter().rect_filled(r, PR::ROUND, pal().hover);
            }
            art::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(15.0, 15.0)), icon, true);
            if b.on_hover_text(tip).clicked() {
                let ctx = ui.ctx().clone();
                set_two_page(app, &ctx, k == 1);
            }
        }
    });
}

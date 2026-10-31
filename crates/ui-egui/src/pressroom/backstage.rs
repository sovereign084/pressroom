//! The File screen: a coloured column of commands on the left (back arrow, Info, New, Open, Save,
//! Save As, Print, Share, Export, Close, Account, Options) and the chosen page on the right. With no
//! publication open it is the start screen, on New.

use egui::{Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

use super::{PR, art, ribbon};
use crate::DesignApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backstage {
    Info,
    New,
    Open,
    Print,
    Share,
    Export,
    Account,
}

const COLUMN_W: f32 = 168.0;

/// A blank publication or one of PressRoom's built-in starting points.
struct Kind {
    name: &'static str,
    /// Inches.
    w: f64,
    h: f64,
    pages: u32,
    facing: bool,
    columns: u32,
    template: Option<&'static str>,
}

const BLANKS: &[Kind] = &[
    Kind { name: "8.5 x 11\"", w: 8.5, h: 11.0, pages: 1, facing: false, columns: 1, template: None },
    Kind { name: "11 x 8.5\"", w: 11.0, h: 8.5, pages: 1, facing: false, columns: 1, template: None },
    Kind { name: "Half Letter Booklet\n5.5 x 8.5\"", w: 5.5, h: 8.5, pages: 4, facing: true, columns: 1, template: None },
    Kind { name: "8.5 x 14\"", w: 8.5, h: 14.0, pages: 1, facing: false, columns: 1, template: None },
];

const BUILT_IN: &[Kind] = &[
    Kind { name: "Church Bulletin\n(folded, 4 pages)", w: 5.5, h: 8.5, pages: 4, facing: true, columns: 1, template: Some("bulletin") },
    Kind { name: "Newsletter\n(4 pages)", w: 8.5, h: 11.0, pages: 4, facing: true, columns: 2, template: Some("newsletter") },
    Kind { name: "Flyer", w: 8.5, h: 11.0, pages: 1, facing: false, columns: 1, template: Some("flyer") },
    Kind { name: "Greeting Card\n(half fold)", w: 5.5, h: 8.5, pages: 4, facing: true, columns: 1, template: Some("card") },
];

pub(super) fn show(app: &mut DesignApp, ui: &mut Ui, page: Backstage) {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, super::pal().page);
    let column = Rect::from_min_max(full.min, pos2(full.min.x + COLUMN_W, full.max.y));
    ui.painter().rect_filled(column, 0.0, PR::TEAL);
    let has_doc = app.session.active().is_some();
    let mut y = column.min.y + 14.0;
    // Back arrow.
    if has_doc {
        let c = pos2(column.min.x + 30.0, y + 16.0);
        let br = Rect::from_center_size(c, vec2(34.0, 34.0));
        let resp = ui.interact(br, egui::Id::unique("pr_bs_back"), Sense::click());
        let col = if resp.hovered() { Color32::from_rgb(200, 230, 225) } else { Color32::WHITE };
        ui.painter().circle_stroke(c, 15.0, Stroke::new(1.5, col));
        ui.painter().line_segment([c + vec2(-7.0, 0.0), c + vec2(7.0, 0.0)], Stroke::new(1.8, col));
        ui.painter().line_segment([c + vec2(-7.0, 0.0), c + vec2(-1.5, -5.5)], Stroke::new(1.8, col));
        ui.painter().line_segment([c + vec2(-7.0, 0.0), c + vec2(-1.5, 5.5)], Stroke::new(1.8, col));
        if resp.on_hover_text("Back").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            super::close_backstage(ui.ctx());
        }
    }
    y += 52.0;
    let items: [(&str, Option<Backstage>, &str, bool); 10] = [
        ("Info", Some(Backstage::Info), "", has_doc),
        ("New", Some(Backstage::New), "", true),
        ("Open", Some(Backstage::Open), "", true),
        ("Save", None, "app.save", has_doc),
        ("Save As", None, "app.saveDialog", has_doc),
        ("Print", Some(Backstage::Print), "", has_doc),
        ("Share", Some(Backstage::Share), "", has_doc),
        ("Export", Some(Backstage::Export), "", has_doc),
        ("Close", None, "file.close", has_doc),
        ("Account", Some(Backstage::Account), "", true),
    ];
    for (label, target, cmd, enabled) in items {
        let r = Rect::from_min_size(pos2(column.min.x, y), vec2(COLUMN_W, 38.0));
        let resp = ui.interact(r, egui::Id::unique(("pr_bs_item", label)), if enabled { Sense::click() } else { Sense::hover() });
        let current = target == Some(page);
        if current {
            ui.painter().rect_filled(r, 0.0, super::PR::TEAL_DARK);
        } else if resp.hovered() && enabled {
            ui.painter().rect_filled(r, 0.0, PR::TEAL_HOVER);
        }
        let color = if enabled { Color32::WHITE } else { Color32::from_rgb(130, 180, 172) };
        ui.painter().text(r.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(15.0), color);
        if resp.clicked() {
            match target {
                Some(t) => super::open_backstage(ui.ctx(), t),
                None => {
                    super::close_backstage(ui.ctx());
                    ribbon::act(app, cmd);
                }
            }
        }
        y += 38.0;
        if label == "Close" {
            y += 12.0;
            ui.painter().line_segment(
                [pos2(column.min.x + 20.0, y - 6.0), pos2(column.max.x - 20.0, y - 6.0)],
                Stroke::new(1.0, Color32::from_rgb(70, 150, 140)),
            );
        }
    }
    let r = Rect::from_min_size(pos2(column.min.x, y), vec2(COLUMN_W, 38.0));
    let resp = ui.interact(r, egui::Id::unique("pr_bs_options"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, PR::TEAL_HOVER);
    }
    ui.painter().text(r.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, "Options", FontId::proportional(15.0), Color32::WHITE);
    if resp.clicked() {
        super::close_backstage(ui.ctx());
        ribbon::act(app, "app.preferences");
    }
    // The page.
    let content = Rect::from_min_max(pos2(column.max.x + 40.0, full.min.y + 24.0), pos2(full.max.x - 40.0, full.max.y - 16.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
    ui.style_mut().visuals = ui.ctx().global_style().visuals.clone();
    let title = match page {
        Backstage::Info => "Info",
        Backstage::New => "New",
        Backstage::Open => "Open",
        Backstage::Print => "Print",
        Backstage::Share => "Share",
        Backstage::Export => "Export",
        Backstage::Account => "Account",
    };
    ui.label(egui::RichText::new(title).font(FontId::proportional(30.0)).color(super::pal().text));
    ui.add_space(18.0);
    egui::ScrollArea::vertical().id_salt("pr_bs_scroll").auto_shrink([false, false]).show(&mut ui, |ui| match page {
        Backstage::Info => info(app, ui),
        Backstage::New => new_page(app, ui),
        Backstage::Open => open_page(app, ui),
        Backstage::Print => print_page(app, ui),
        Backstage::Share => share_page(app, ui),
        Backstage::Export => export_page(app, ui),
        Backstage::Account => account(app, ui),
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(s).font(FontId::proportional(16.0)).color(PR::TEAL));
    ui.add_space(6.0);
}

/// A large tile with an icon and a caption under it.
fn tile(ui: &mut Ui, id: &str, size: egui::Vec2, caption: &str, draw: impl FnOnce(&egui::Painter, Rect)) -> bool {
    let (r, resp) = ui.allocate_exact_size(size + vec2(0.0, 36.0), Sense::click());
    // A rounded card that lifts when pointed at and settles back when pressed, as the cards on
    // Office's start screen do.
    let lift = ui.ctx().animate_bool_with_time(resp.id.with("lift"), resp.hovered(), 0.12);
    let pressed = resp.is_pointer_button_down_on();
    let card = Rect::from_min_size(r.min, size).translate(vec2(0.0, -3.0 * lift)).shrink(if pressed { 1.5 } else { 0.0 });
    let p = super::pal();
    let shadow = egui::epaint::Shadow {
        offset: [0, (2.0 + 4.0 * lift) as i8],
        blur: (6.0 + 12.0 * lift) as u8,
        spread: 0,
        color: Color32::from_black_alpha(((if p.dark { 90.0 } else { 18.0 }) + 30.0 * lift) as u8),
    };
    ui.painter().add(shadow.as_shape(card, 8.0));
    let edge = p.card_edge.lerp_to_gamma(p.select, lift);
    ui.painter().rect(card, 8.0, p.card, Stroke::new(1.0 + 0.5 * lift, edge), StrokeKind::Inside);
    draw(ui.painter(), card);
    for (i, line) in caption.split('\n').enumerate() {
        ui.painter().text(
            pos2(r.center().x, card.max.y + 12.0 + i as f32 * 14.0),
            egui::Align2::CENTER_CENTER,
            line,
            FontId::proportional(12.0),
            super::pal().text,
        );
    }
    let _ = id;
    resp.on_hover_text(caption.replace('\n', " ")).clicked()
}

/// A page shape for the New tiles, its proportions those of the publication.
fn paper(p: &egui::Painter, card: Rect, w: f64, h: f64) -> Rect {
    let s = (card.width() - 16.0).min(card.height() - 16.0);
    let (pw, ph) = if w >= h { (s, s * (h / w) as f32) } else { (s * (w / h) as f32, s) };
    let r = Rect::from_center_size(card.center(), vec2(pw, ph));
    // Paper with a soft shadow under it.
    p.add(egui::epaint::Shadow { offset: [0, 1], blur: 6, spread: 0, color: Color32::from_black_alpha(45) }.as_shape(r, 2.0));
    p.rect(r, 2.0, Color32::WHITE, Stroke::new(1.0, Color32::from_gray(205)), StrokeKind::Inside);
    r
}

fn new_page(app: &mut DesignApp, ui: &mut Ui) {
    heading(ui, "Blank page sizes");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(22.0, 18.0);
        for k in BLANKS {
            if tile(ui, k.name, vec2(130.0, 130.0), k.name, |p, card| {
                paper(p, card, k.w, k.h);
            }) {
                create(app, ui.ctx(), k);
            }
        }
    });
    ui.add_space(24.0);
    heading(ui, "Built-in");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(22.0, 18.0);
        for k in BUILT_IN {
            if tile(ui, k.name, vec2(130.0, 130.0), k.name, |p, card| {
                let r = paper(p, card, k.w, k.h);
                preview(p, r, k.template.unwrap_or(""));
            }) {
                create(app, ui.ctx(), k);
            }
        }
    });
    ui.add_space(24.0);
    if ui.button(egui::RichText::new("  Open an existing publication…  ").font(FontId::proportional(13.0))).clicked() {
        super::close_backstage(ui.ctx());
        ribbon::act(app, "app.openDialog");
    }
}

/// A little sketch of what a built-in starting point looks like.
fn preview(p: &egui::Painter, r: Rect, kind: &str) {
    let line = |y: f32, x0: f32, x1: f32, c: Color32| {
        p.line_segment(
            [pos2(r.min.x + x0 * r.width(), r.min.y + y * r.height()), pos2(r.min.x + x1 * r.width(), r.min.y + y * r.height())],
            Stroke::new(1.5, c),
        );
    };
    let block = |x0: f32, y0: f32, x1: f32, y1: f32, c: Color32| {
        p.rect_filled(
            Rect::from_min_max(pos2(r.min.x + x0 * r.width(), r.min.y + y0 * r.height()), pos2(r.min.x + x1 * r.width(), r.min.y + y1 * r.height())),
            0.0,
            c,
        );
    };
    let grey = Color32::from_gray(185);
    match kind {
        "bulletin" => {
            block(0.12, 0.08, 0.88, 0.2, super::PR::TEAL);
            p.text(r.center() - vec2(0.0, r.height() * 0.08), egui::Align2::CENTER_CENTER, "✝", FontId::proportional(r.height() * 0.18), art::GOLD);
            for y in [0.55, 0.62, 0.69, 0.76, 0.83] {
                line(y, 0.15, 0.85, grey);
            }
        }
        "newsletter" => {
            block(0.08, 0.06, 0.92, 0.18, super::PR::TEAL);
            block(0.08, 0.24, 0.46, 0.46, art::SKY);
            for y in [0.27, 0.32, 0.37, 0.42, 0.52, 0.57, 0.62, 0.67, 0.72, 0.77, 0.82, 0.87] {
                line(y, 0.54, 0.92, grey);
            }
            for y in [0.52, 0.57, 0.62, 0.67, 0.72, 0.77, 0.82, 0.87] {
                line(y, 0.08, 0.46, grey);
            }
        }
        "flyer" => {
            block(0.1, 0.08, 0.9, 0.28, art::GOLD);
            block(0.15, 0.34, 0.85, 0.66, art::SKY);
            for y in [0.74, 0.8, 0.86] {
                line(y, 0.2, 0.8, grey);
            }
        }
        _ => {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "♥", FontId::proportional(r.height() * 0.3), art::RED);
        }
    }
}

/// Make the new publication (and its built-in content), then leave the File screen.
fn create(app: &mut DesignApp, ctx: &egui::Context, k: &Kind) {
    let params = json!({
        "units": "inches",
        "width": k.w * 72.0,
        "height": k.h * 72.0,
        "pages": k.pages,
        "facingPages": k.facing,
        "columns": k.columns,
        "gutter": 18.0,
        "margins": 36.0,
        "intent": "print",
    });
    if let Err(e) = app.run("file.new", params) {
        app.status(format!("Couldn't create the publication: {e}"));
        return;
    }
    super::update(ctx, |s| {
        s.backstage = None;
        s.two_page = k.facing;
        s.tab = super::Tab::Home;
    });
    // As in Publisher: only objects in front of a text box wrap its text, and characters a font
    // lacks come from another font.
    let _ = app.run("document.preferences", json!({"textWrapOnlyBeneath": true, "glyphFallback": true}));
    // Publisher's default text: Calibri 10 pt, where Windows has it.
    if let Some(style) = app.session.active().map(|d| d.doc.styles.default_paragraph.clone()) {
        let _ = app.run("style.paragraph.edit", json!({"name": style, "chars": {"fontFamily": "Calibri", "fontStyle": "Regular", "size": 10.0}}));
    }
    if let Some(t) = k.template {
        super::templates::fill(app, t);
    }
    ribbon::act(app, "view.fitPage");
}

fn open_page(app: &mut DesignApp, ui: &mut Ui) {
    if ui.button(egui::RichText::new("   Browse…   ").font(FontId::proportional(14.0))).clicked() {
        super::close_backstage(ui.ctx());
        ribbon::act(app, "app.openDialog");
    }
    ui.add_space(10.0);
    // Her old Publisher files, all at once, while Publisher is still on this computer.
    let busy = super::pubimport::busy().is_some();
    if ui
        .add_enabled(!busy, egui::Button::new(egui::RichText::new("   Convert a Folder of Publisher Files\u{2026}   ").font(FontId::proportional(13.0))))
        .on_hover_text("Turns every Publisher file in a folder (and its folders) into a PressRoom publication beside it. Publisher must still be installed on this computer.")
        .clicked()
        && let Some(folder) = app.services.pick_open.as_mut().and_then(|pick| pick("folder"))
    {
        super::pubimport::start_folder(app, &folder);
    }
    ui.add_space(18.0);
    heading(ui, "Recent Publications");
    let recent = app.run("file.recentFiles", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if recent.is_empty() {
        ui.label(egui::RichText::new("Publications you open will show up here.").color(super::pal().group_label));
    }
    for (i, f) in recent.iter().enumerate() {
        let name = f["name"].as_str().unwrap_or("");
        let path = f["path"].as_str().unwrap_or("");
        let exists = f["exists"].as_bool().unwrap_or(true);
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(640.0), 44.0), if exists { Sense::click() } else { Sense::hover() });
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.1);
        if hover > 0.0 {
            ui.painter().rect_filled(r, 6.0, super::pal().checked.gamma_multiply(hover));
        }
        art::paint(ui.painter(), Rect::from_min_size(r.min + vec2(6.0, 8.0), vec2(28.0, 28.0)), "single-page", exists);
        ui.painter().text(
            r.min + vec2(44.0, 14.0),
            egui::Align2::LEFT_CENTER,
            name,
            FontId::proportional(13.5),
            if exists { super::pal().text } else { super::pal().disabled },
        );
        ui.painter().text(r.min + vec2(44.0, 31.0), egui::Align2::LEFT_CENTER, path, FontId::proportional(11.0), super::pal().group_label);
        if resp.on_hover_text(path).clicked() {
            super::close_backstage(ui.ctx());
            ribbon::act_with(app, "file.openRecent", json!({"index": i}));
        }
    }
}

fn info(app: &mut DesignApp, ui: &mut Ui) {
    let Some(st) = app.session.active() else { return };
    let title = st.title();
    let pages = st.doc.page_count();
    let size = st.doc.page(0).map(|p| (p.width / 72.0, p.height / 72.0));
    let path = st.path.clone().unwrap_or_else(|| "Not saved yet".into());
    heading(ui, &title);
    ui.label(egui::RichText::new(path).color(super::pal().group_label));
    ui.add_space(14.0);
    egui::Grid::new("pr_info").num_columns(2).spacing(vec2(24.0, 8.0)).show(ui, |ui| {
        ui.label("Pages");
        ui.label(pages.to_string());
        ui.end_row();
        if let Some((w, h)) = size {
            ui.label("Page size");
            ui.label(format!("{w:.2} x {h:.2}\""));
            ui.end_row();
        }
    });
    // Fonts this computer doesn't have. Opening the publication doesn't stop to say so (Publisher
    // doesn't either): they are listed here.
    let missing = missing_fonts(app);
    if !missing.is_empty() {
        ui.add_space(18.0);
        ui.label(egui::RichText::new("Fonts not on this computer").strong());
        ui.label(
            egui::RichText::new("A similar font shows in their place. The publication keeps them, so they come back on a computer that has them.")
                .color(super::pal().group_label),
        );
        ui.add_space(4.0);
        egui::Grid::new("pr_missing_fonts").num_columns(2).spacing(vec2(24.0, 4.0)).show(ui, |ui| {
            for (family, shown_as) in &missing {
                ui.label(family);
                ui.label(egui::RichText::new(format!("shows as {shown_as}")).color(super::pal().group_label));
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if ui.button("  Replace Fonts…  ").clicked() {
            super::close_backstage(ui.ctx());
            let _ = app.run("app.findFontDialog", json!({}));
        }
    }
    ui.add_space(18.0);
    if ui.button("  Run Design Checker  ").clicked() {
        super::close_backstage(ui.ctx());
        let _ = app.run("window.panel.activate", json!({"panel": "preflight"}));
    }
}

/// The document's fonts that aren't installed, each once, with the family shown in its place.
fn missing_fonts(app: &mut DesignApp) -> Vec<(String, String)> {
    let Ok(fonts) = app.session.execute("font.list", &json!({})) else { return vec![] };
    let mut out: Vec<(String, String)> = Vec::new();
    for f in fonts.as_array().into_iter().flatten().filter(|f| f["missing"] == true) {
        let family = f["family"].as_str().unwrap_or_default().to_string();
        if family.is_empty() || out.iter().any(|(name, _)| *name == family) {
            continue;
        }
        let shown_as = f["replacementFamily"].as_str().unwrap_or("a similar font").to_string();
        out.push((family, shown_as));
    }
    out
}

/// The Print page's choices, kept while the File screen is open.
#[derive(Clone, Debug)]
struct PrintSettings {
    copies: u32,
    printer: Option<String>,
    /// 0 all pages, 1 current page, 2 the pages typed in `pages`.
    which: u8,
    pages: String,
    booklet: bool,
    /// "one", "longEdge", "shortEdge".
    sides: &'static str,
    grayscale: bool,
    /// The sheet or page shown in the preview.
    preview: usize,
    /// Printers, read once per visit (on a thread: see [`look_for_printers`]).
    printers: Option<(Vec<String>, Option<String>)>,
    /// Whether this visit has started looking for them.
    looked: bool,
    /// The document the defaults were chosen for.
    doc: u64,
}

/// The printers last found, and whether a search is under way. They are looked for on a thread:
/// asking a printer that can't be reached (an office printer away from the office) can take
/// minutes, and the window mustn't wait on it.
static PRINTERS: std::sync::Mutex<Option<(Vec<String>, Option<String>)>> = std::sync::Mutex::new(None);
static LOOKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Starts looking for the printers, unless a search is under way.
fn look_for_printers(ctx: &egui::Context) {
    use std::sync::atomic::Ordering;
    if LOOKING.swap(true, Ordering::SeqCst) {
        return;
    }
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let r = designcraft_engine::cmd::printer_list();
        let names: Vec<String> =
            r["printers"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let default = r["default"].as_str().map(str::to_string);
        *PRINTERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((names, default));
        LOOKING.store(false, Ordering::SeqCst);
        ctx.request_repaint();
    });
}

fn print_settings_id() -> egui::Id {
    egui::Id::unique("pr_print_settings")
}

fn print_page(app: &mut DesignApp, ui: &mut Ui) {
    let Some((uid, n, folded)) = app.session.active().map(|d| {
        let p = d.doc.page(0);
        // Half-letter facing pages: a folded bulletin or card.
        let folded = d.doc.settings.facing_pages && p.is_some_and(|p| (p.width - 396.0).abs() < 2.0 && (p.height - 612.0).abs() < 2.0);
        (d.uid, d.doc.page_count(), folded)
    }) else {
        return;
    };
    let ctx = ui.ctx().clone();
    let mut s: PrintSettings = ctx.data(|d| d.get_temp(print_settings_id())).filter(|s: &PrintSettings| s.doc == uid).unwrap_or(PrintSettings {
        copies: 1,
        printer: None,
        which: 0,
        pages: String::new(),
        booklet: folded,
        sides: if folded { "shortEdge" } else { "one" },
        grayscale: false,
        preview: 0,
        printers: None,
        looked: false,
        doc: uid,
    });
    if s.printers.is_none() {
        // The last list at once (none the first time), and a fresh look for each visit.
        let found = PRINTERS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        if !s.looked {
            look_for_printers(&ctx);
            s.looked = true;
        }
        if let Some((names, default)) = found
            && !LOOKING.load(std::sync::atomic::Ordering::SeqCst)
        {
            if s.printer.is_none() {
                s.printer = default.clone().or_else(|| names.first().cloned());
            }
            s.printers = Some((names, default));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
    }
    let mut print_now = false;
    ui.horizontal_top(|ui| {
        // Settings column.
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(88.0, 88.0), Sense::click());
                ui.painter().rect(
                    r,
                    0.0,
                    if resp.hovered() { super::pal().checked } else { super::pal().card },
                    Stroke::new(1.0, super::pal().ribbon_edge),
                    StrokeKind::Inside,
                );
                art::paint(ui.painter(), Rect::from_center_size(r.center() - vec2(0.0, 10.0), vec2(40.0, 40.0)), "print", true);
                ui.painter().text(r.center() + vec2(0.0, 28.0), egui::Align2::CENTER_CENTER, "Print", FontId::proportional(13.0), super::pal().text);
                print_now = resp.clicked();
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add_space(24.0);
                    ui.label("Copies of print job:");
                    ui.add(egui::DragValue::new(&mut s.copies).range(1..=999).speed(0.1));
                });
            });
            ui.add_space(14.0);
            heading(ui, "Printer");
            let (names, default) = s.printers.clone().unwrap_or_default();
            let shown = match (&s.printer, &s.printers) {
                (Some(p), _) => p.clone(),
                (None, None) => "Looking for printers\u{2026}".into(),
                (None, Some(_)) => "No printer found".into(),
            };
            craft_ui::ComboBox::from_id_salt("pr_printer").width(290.0).selected_text(shown).show_ui(ui, |ui| {
                for n in &names {
                    let label = if Some(n) == default.as_ref() { format!("{n} (default)") } else { n.clone() };
                    ui.selectable_value(&mut s.printer, Some(n.clone()), label);
                }
            });
            ui.add_space(14.0);
            heading(ui, "Settings");
            let which = ["Print All Pages", "Print Current Page", "Custom Print"];
            craft_ui::ComboBox::from_id_salt("pr_which").width(290.0).selected_text(which[usize::from(s.which.min(2))]).show_ui(ui, |ui| {
                for (k, label) in which.iter().enumerate() {
                    ui.selectable_value(&mut s.which, k as u8, *label);
                }
            });
            if s.which == 2 {
                ui.horizontal(|ui| {
                    ui.label("Pages:");
                    ui.add(egui::TextEdit::singleline(&mut s.pages).hint_text("e.g. 1-3, 5").desired_width(200.0));
                });
            }
            ui.add_space(6.0);
            let layout = if s.booklet { "Booklet, side-fold" } else { "One page per sheet" };
            craft_ui::ComboBox::from_id_salt("pr_layout").width(290.0).selected_text(layout).show_ui(ui, |ui| {
                if ui.selectable_label(!s.booklet, "One page per sheet").clicked() {
                    s.booklet = false;
                    s.preview = 0;
                }
                if ui.selectable_label(s.booklet, "Booklet, side-fold").clicked() {
                    s.booklet = true;
                    s.sides = "shortEdge";
                    s.preview = 0;
                }
            });
            ui.add_space(6.0);
            let sides = [
                ("one", "Print One Sided"),
                ("longEdge", "Print on Both Sides (flip sheets on long edge)"),
                ("shortEdge", "Print on Both Sides (flip sheets on short edge)"),
            ];
            let cur = sides.iter().find(|(k, _)| *k == s.sides).map_or("Print One Sided", |(_, l)| *l);
            craft_ui::ComboBox::from_id_salt("pr_sides").width(290.0).selected_text(cur).show_ui(ui, |ui| {
                for (k, label) in sides {
                    ui.selectable_value(&mut s.sides, k, label);
                }
            });
            ui.add_space(6.0);
            craft_ui::ComboBox::from_id_salt("pr_color").width(290.0).selected_text(if s.grayscale { "Grayscale" } else { "Color" }).show_ui(
                ui,
                |ui| {
                    ui.selectable_value(&mut s.grayscale, false, "Color");
                    ui.selectable_value(&mut s.grayscale, true, "Grayscale");
                },
            );
            if s.booklet {
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Each sheet holds two pages; fold the printed stack in half.").color(super::pal().group_label));
                if n % 4 != 0 {
                    ui.label(
                        egui::RichText::new(format!("{n} pages: blank pages fill the booklet out to {}.", n.div_ceil(4) * 4))
                            .color(super::pal().group_label),
                    );
                }
            }
        });
        ui.add_space(30.0);
        // Preview, with its page arrows under it.
        ui.vertical(|ui| print_preview(app, ui, &mut s, n));
    });
    let cur = crate::canvas::current_page(app).unwrap_or(0);
    ctx.data_mut(|d| d.insert_temp(print_settings_id(), s.clone()));
    if print_now {
        let mut p = json!({"copies": s.copies, "sides": s.sides, "grayscale": s.grayscale});
        if let Some(printer) = &s.printer {
            p["printer"] = json!(printer);
        }
        if s.booklet {
            p["layout"] = json!("booklet");
        } else if s.which == 1 {
            p["pages"] = json!((cur + 1).to_string());
        } else if s.which == 2 && !s.pages.trim().is_empty() {
            p["pages"] = json!(s.pages.replace(' ', ""));
        }
        match app.run("file.print", p) {
            Ok(_) => {
                app.status(format!("Sent to {}.", s.printer.clone().unwrap_or_else(|| "the printer".into())));
                super::close_backstage(&ctx);
            }
            Err(e) => app.status(format!("Couldn't print: {e}")),
        }
    }
}

/// The preview: one page, or a booklet sheet side (two pages in folding order).
fn print_preview(app: &mut DesignApp, ui: &mut Ui, s: &mut PrintSettings, n: usize) {
    let ctx = ui.ctx().clone();
    let padded = n.div_ceil(4) * 4;
    let count = if s.booklet { padded / 2 } else { n };
    s.preview = s.preview.min(count.saturating_sub(1));
    let avail = ui.available_size();
    let area = vec2(avail.x.max(200.0), (avail.y - 40.0).clamp(200.0, 620.0));
    let (r, _) = ui.allocate_exact_size(area, Sense::hover());
    ui.painter().rect_filled(r, 0.0, Color32::from_gray(225));
    let pages: Vec<Option<usize>> = if s.booklet {
        let i = s.preview;
        let (l, rt) = if i.is_multiple_of(2) { (padded - 1 - i, i) } else { (i, padded - 1 - i) };
        vec![(l < n).then_some(l), (rt < n).then_some(rt)]
    } else {
        vec![Some(s.preview)]
    };
    let (pw, ph) = app.session.active().and_then(|d| d.doc.page(0).map(|p| (p.width as f32, p.height as f32))).unwrap_or((612.0, 792.0));
    let sheet_w = pw * pages.len() as f32;
    let scale = ((r.width() - 40.0) / sheet_w).min((r.height() - 40.0) / ph);
    let size = vec2(pw * scale, ph * scale);
    let total = Rect::from_center_size(r.center(), vec2(size.x * pages.len() as f32, size.y));
    ui.painter().rect_filled(total.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(40));
    for (k, page) in pages.iter().enumerate() {
        let pr = Rect::from_min_size(total.min + vec2(size.x * k as f32, 0.0), size);
        ui.painter().rect_filled(pr, 0.0, Color32::WHITE);
        if let Some(abs) = page
            && let Some(tex) = crate::panels::pages::thumb(app, &ctx, crate::panels::pages::ThumbOf::Page(*abs), size.y)
        {
            ui.painter().image(tex.id(), pr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }
    if s.booklet {
        ui.painter().line_segment([total.center_top(), total.center_bottom()], Stroke::new(1.0, Color32::from_gray(200)));
    }
    ui.painter().rect_stroke(total, 0.0, Stroke::new(1.0, Color32::from_gray(160)), StrokeKind::Outside);
    ui.horizontal(|ui| {
        ui.add_space(r.width() / 2.0 - 90.0);
        if ui.add_enabled(s.preview > 0, egui::Button::new("◀")).clicked() {
            s.preview -= 1;
        }
        let what = if s.booklet {
            format!("Sheet {} {} of {}", s.preview / 2 + 1, if s.preview.is_multiple_of(2) { "front" } else { "back" }, count.div_ceil(2))
        } else {
            format!("{} of {}", s.preview + 1, count)
        };
        ui.label(egui::RichText::new(what).font(FontId::proportional(13.0)));
        if ui.add_enabled(s.preview + 1 < count, egui::Button::new("▶")).clicked() {
            s.preview += 1;
        }
    });
}

fn share_page(app: &mut DesignApp, ui: &mut Ui) {
    heading(ui, "Send it as a PDF");
    ui.label("Save a PDF to attach to an email or post online. Anyone can open it.");
    ui.add_space(8.0);
    if ui.button("  Create PDF…  ").clicked() {
        super::close_backstage(ui.ctx());
        ribbon::act(app, "app.exportPdf");
    }
}

fn export_page(app: &mut DesignApp, ui: &mut Ui) {
    for (label, detail, cmd) in [
        ("Create PDF Document", "A PDF looks the same on every computer and prints well.", "app.exportPdf"),
        ("Save as Picture", "Save the page as a PNG picture.", "app.exportPng"),
        ("Save for Word / PowerPoint", "Export the pages as a PowerPoint file.", "app.exportPptx"),
        ("Save as a Web Page", "One HTML page with the text and pictures.", "app.exportHtml"),
    ] {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 56.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, super::pal().checked);
        }
        art::paint(ui.painter(), Rect::from_min_size(r.min + vec2(8.0, 12.0), vec2(32.0, 32.0)), "pdf", true);
        ui.painter().text(r.min + vec2(52.0, 18.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), super::pal().text);
        ui.painter().text(r.min + vec2(52.0, 38.0), egui::Align2::LEFT_CENTER, detail, FontId::proportional(11.5), super::pal().group_label);
        if resp.clicked() {
            super::close_backstage(ui.ctx());
            ribbon::act(app, cmd);
        }
    }
}

fn account(app: &mut DesignApp, ui: &mut Ui) {
    heading(ui, "PressRoom");
    ui.label("Free desktop publishing for bulletins, newsletters, flyers and cards.");
    ui.label(egui::RichText::new("Built on DesignCraft (Apache-2.0 / MIT). No account or subscription needed.").color(super::pal().group_label));
    // Office Theme, as in Publisher's Account page. "Use system setting" (Office's default) is White
    // on a light Windows and Black on a dark one; Colorful has the top in Publisher's teal.
    ui.add_space(18.0);
    ui.label(egui::RichText::new("Office Theme").size(15.0).color(super::pal().text));
    let themes = [("system", "Use system setting"), ("white", "White"), ("colorful", "Colorful"), ("darkgray", "Dark Gray"), ("black", "Black")];
    let shown = themes.iter().find(|(v, _)| *v == app.ui.pressroom_theme).map_or("Use system setting", |(_, l)| *l);
    craft_ui::ComboBox::from_id_salt("pr_office_theme").selected_text(shown).width(180.0).show_ui(ui, |ui| {
        for (value, label) in themes {
            if ui.selectable_label(app.ui.pressroom_theme == value, label).clicked() {
                app.ui.pressroom_theme = value.into();
            }
        }
    });
    // Update Options, as in Publisher's Account page.
    ui.add_space(18.0);
    ui.label(egui::RichText::new("Product Information").size(15.0).color(super::pal().text));
    let version = super::update::VERSION.map_or_else(|| "Development build".to_string(), |v| format!("Version {v}"));
    ui.label(egui::RichText::new(version).color(super::pal().group_label));
    ui.add_space(6.0);
    if ui.button("Check for Updates").on_hover_text("Look for a newer version of PressRoom").clicked() {
        app.updates_rt.pressroom.check(ui.ctx(), true);
    }
    let _ = Value::Null;
}

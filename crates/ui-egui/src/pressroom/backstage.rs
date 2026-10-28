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
    ui.painter().rect_filled(full, 0.0, Color32::WHITE);
    let column = Rect::from_min_max(full.min, pos2(full.min.x + COLUMN_W, full.max.y));
    ui.painter().rect_filled(column, 0.0, PR::TEAL);
    let has_doc = app.session.active().is_some();
    let mut y = column.min.y + 14.0;
    // Back arrow.
    if has_doc {
        let c = pos2(column.min.x + 30.0, y + 16.0);
        let br = Rect::from_center_size(c, vec2(34.0, 34.0));
        let resp = ui.interact(br, egui::Id::new("pr_bs_back"), Sense::click());
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
        let resp = ui.interact(r, egui::Id::new(("pr_bs_item", label)), if enabled { Sense::click() } else { Sense::hover() });
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
            ui.painter().line_segment([pos2(column.min.x + 20.0, y - 6.0), pos2(column.max.x - 20.0, y - 6.0)], Stroke::new(1.0, Color32::from_rgb(70, 150, 140)));
        }
    }
    let r = Rect::from_min_size(pos2(column.min.x, y), vec2(COLUMN_W, 38.0));
    let resp = ui.interact(r, egui::Id::new("pr_bs_options"), Sense::click());
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
    ui.style_mut().visuals = egui::Visuals::light();
    let title = match page {
        Backstage::Info => "Info",
        Backstage::New => "New",
        Backstage::Open => "Open",
        Backstage::Print => "Print",
        Backstage::Share => "Share",
        Backstage::Export => "Export",
        Backstage::Account => "Account",
    };
    ui.label(egui::RichText::new(title).font(FontId::proportional(30.0)).color(PR::TEXT));
    ui.add_space(18.0);
    egui::ScrollArea::vertical().id_salt("pr_bs_scroll").auto_shrink([false, false]).show(&mut ui, |ui| match page {
        Backstage::Info => info(app, ui),
        Backstage::New => new_page(app, ui),
        Backstage::Open => open_page(app, ui),
        Backstage::Print => print_page(app, ui),
        Backstage::Share => share_page(app, ui),
        Backstage::Export => export_page(app, ui),
        Backstage::Account => account(ui),
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(s).font(FontId::proportional(16.0)).color(PR::TEAL));
    ui.add_space(6.0);
}

/// A large tile with an icon and a caption under it.
fn tile(ui: &mut Ui, id: &str, size: egui::Vec2, caption: &str, draw: impl FnOnce(&egui::Painter, Rect)) -> bool {
    let (r, resp) = ui.allocate_exact_size(size + vec2(0.0, 36.0), Sense::click());
    let card = Rect::from_min_size(r.min, size);
    if resp.hovered() {
        ui.painter().rect_filled(r.expand(4.0), 0.0, Color32::from_rgb(222, 236, 233));
    }
    draw(ui.painter(), card);
    for (i, line) in caption.split('\n').enumerate() {
        ui.painter().text(pos2(r.center().x, card.max.y + 12.0 + i as f32 * 14.0), egui::Align2::CENTER_CENTER, line, FontId::proportional(12.0), PR::TEXT);
    }
    let _ = id;
    resp.on_hover_text(caption.replace('\n', " ")).clicked()
}

/// A page shape for the New tiles, its proportions those of the publication.
fn paper(p: &egui::Painter, card: Rect, w: f64, h: f64) -> Rect {
    let s = (card.width() - 16.0).min(card.height() - 16.0);
    let (pw, ph) = if w >= h { (s, s * (h / w) as f32) } else { (s * (w / h) as f32, s) };
    let r = Rect::from_center_size(card.center(), vec2(pw, ph));
    p.rect_filled(r.translate(vec2(2.0, 2.0)), 0.0, Color32::from_black_alpha(30));
    p.rect(r, 0.0, Color32::WHITE, Stroke::new(1.0, Color32::from_gray(170)), StrokeKind::Inside);
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
        p.line_segment([pos2(r.min.x + x0 * r.width(), r.min.y + y * r.height()), pos2(r.min.x + x1 * r.width(), r.min.y + y * r.height())], Stroke::new(1.5, c));
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
        "margins": if k.w <= 6.0 { 36.0 } else { 36.0 },
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
    ui.add_space(18.0);
    heading(ui, "Recent Publications");
    let recent = app.run("file.recentFiles", json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    if recent.is_empty() {
        ui.label(egui::RichText::new("Publications you open will show up here.").color(PR::GROUP_LABEL));
    }
    for (i, f) in recent.iter().enumerate() {
        let name = f["name"].as_str().unwrap_or("");
        let path = f["path"].as_str().unwrap_or("");
        let exists = f["exists"].as_bool().unwrap_or(true);
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(640.0), 44.0), if exists { Sense::click() } else { Sense::hover() });
        if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, Color32::from_rgb(222, 236, 233));
        }
        art::paint(ui.painter(), Rect::from_min_size(r.min + vec2(6.0, 8.0), vec2(28.0, 28.0)), "single-page", exists);
        ui.painter().text(r.min + vec2(44.0, 14.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(13.5), if exists { PR::TEXT } else { PR::DISABLED });
        ui.painter().text(r.min + vec2(44.0, 31.0), egui::Align2::LEFT_CENTER, path, FontId::proportional(11.0), PR::GROUP_LABEL);
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
    ui.label(egui::RichText::new(path).color(PR::GROUP_LABEL));
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
    ui.add_space(18.0);
    if ui.button("  Run Design Checker  ").clicked() {
        super::close_backstage(ui.ctx());
        let _ = app.run("window.panel.activate", json!({"panel": "preflight"}));
    }
}

fn print_page(app: &mut DesignApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(96.0, 96.0), Sense::click());
        ui.painter().rect(r, 0.0, if resp.hovered() { Color32::from_rgb(222, 236, 233) } else { Color32::WHITE }, Stroke::new(1.0, PR::RIBBON_EDGE), StrokeKind::Inside);
        art::paint(ui.painter(), Rect::from_center_size(r.center() - vec2(0.0, 10.0), vec2(40.0, 40.0)), "print", true);
        ui.painter().text(r.center() + vec2(0.0, 28.0), egui::Align2::CENTER_CENTER, "Print", FontId::proportional(13.0), PR::TEXT);
        if resp.clicked() {
            super::close_backstage(ui.ctx());
            ribbon::act(app, "file.print");
        }
    });
    ui.add_space(18.0);
    heading(ui, "Booklet");
    ui.label("For a folded bulletin or card: prints the pages in the right order to fold.");
    ui.add_space(6.0);
    if ui.button("  Print as a Booklet…  ").clicked() {
        super::close_backstage(ui.ctx());
        ribbon::act(app, "app.printBooklet");
    }
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
            ui.painter().rect_filled(r, 0.0, Color32::from_rgb(222, 236, 233));
        }
        art::paint(ui.painter(), Rect::from_min_size(r.min + vec2(8.0, 12.0), vec2(32.0, 32.0)), "pdf", true);
        ui.painter().text(r.min + vec2(52.0, 18.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), PR::TEXT);
        ui.painter().text(r.min + vec2(52.0, 38.0), egui::Align2::LEFT_CENTER, detail, FontId::proportional(11.5), PR::GROUP_LABEL);
        if resp.clicked() {
            super::close_backstage(ui.ctx());
            ribbon::act(app, cmd);
        }
    }
}

fn account(ui: &mut Ui) {
    heading(ui, "PressRoom");
    ui.label("Free desktop publishing for bulletins, newsletters, flyers and cards.");
    ui.label(egui::RichText::new("Built on DesignCraft (Apache-2.0 / MIT). No account or subscription needed.").color(PR::GROUP_LABEL));
    let _ = Value::Null;
}

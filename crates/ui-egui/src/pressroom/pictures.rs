//! Picture Tools › Format: the Corrections and Recolor galleries, drawn from the selected picture
//! itself as Publisher's are, and the Compress Pictures dialog. The work is the engine's
//! `picture.adjust`, `picture.reset` and `picture.compress`.

use std::collections::HashMap;
use std::sync::Mutex;

use designcraft_doc::AssetId;
use designcraft_images::adjust::{self, Recolor, RgbaImage};
use egui::{Color32, Rect, Sense, Stroke, TextureHandle, Ui, vec2};
use serde_json::{Value, json};

use crate::DesignApp;
use crate::dialogs::Dialog;

/// The brightness and contrast steps of the Corrections gallery, in percent.
const STEPS: [i32; 5] = [-40, -20, 0, 20, 40];

/// The colours of the Dark and Light Variations: PressRoom's own, picked for church bulletins.
pub const TINTS: [(&str, [u8; 3]); 6] = [
    ("Teal", [0x1f, 0x5f, 0x7a]),
    ("Blue", [0x2f, 0x55, 0x97]),
    ("Green", [0x4e, 0x7d, 0x3a]),
    ("Burgundy", [0x8b, 0x23, 0x32]),
    ("Gold", [0xb8, 0x86, 0x0b]),
    ("Purple", [0x5b, 0x3a, 0x82]),
];

/// Thumbnails of the selected picture: its original, small, and each variation as a texture.
struct Thumbs {
    source: AssetId,
    small: Option<RgbaImage>,
    textures: HashMap<String, TextureHandle>,
}

static THUMBS: Mutex<Option<Thumbs>> = Mutex::new(None);

/// The selected picture: its original asset and the corrections it shows.
fn selected(app: &DesignApp) -> Option<(AssetId, designcraft_doc::Adjusted)> {
    let st = app.session.active()?;
    let g = st.selection.items.iter().find_map(|id| st.doc.item(*id)?.graphic().cloned())?;
    match st.doc.assets.get(&g.asset)?.adjusted.clone() {
        Some(a) => Some((a.source, a)),
        None => Some((g.asset, designcraft_doc::Adjusted { source: g.asset, ..Default::default() })),
    }
}

/// The variation `key` of the selected picture (made with `make`), as a texture.
fn thumb(app: &DesignApp, ctx: &egui::Context, key: &str, make: impl FnOnce(&mut RgbaImage)) -> Option<TextureHandle> {
    let (source, _) = selected(app)?;
    let mut guard = THUMBS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if guard.as_ref().is_none_or(|t| t.source != source) {
        let small = app
            .session
            .active()
            .and_then(|st| st.doc.assets.get(&source).cloned())
            .and_then(|a| designcraft_images::decode_rgba(&a.data))
            .map(|img| adjust::thumbnail(&img, 96));
        *guard = Some(Thumbs { source, small, textures: HashMap::new() });
    }
    let t = guard.as_mut()?;
    if let Some(tex) = t.textures.get(key) {
        return Some(tex.clone());
    }
    let mut img = t.small.clone()?;
    make(&mut img);
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let tex = ctx.load_texture(format!("pr_picture_{}_{key}", source.0), color, egui::TextureOptions::LINEAR);
    t.textures.insert(key.to_string(), tex.clone());
    Some(tex)
}

/// One gallery cell: the picture fitted into it, outlined when it's the current choice.
fn cell(ui: &mut Ui, tex: Option<&TextureHandle>, current: bool, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(64.0, 52.0), Sense::click());
    let pal = super::pal();
    if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, pal.hover);
    }
    let inner = r.shrink(4.0);
    match tex {
        Some(tex) => {
            let [w, h] = tex.size();
            let k = (inner.width() / w.max(1) as f32).min(inner.height() / h.max(1) as f32);
            let shown = Rect::from_center_size(inner.center(), vec2(w as f32 * k, h as f32 * k));
            ui.painter().image(tex.id(), shown, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            ui.painter().rect_filled(inner, 2.0, pal.card_edge);
        }
    }
    if current {
        ui.painter().rect_stroke(r.shrink(1.0), 3.0, Stroke::new(2.0, pal.select), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_stroke(r.shrink(1.0), 3.0, Stroke::new(1.0, pal.hover_border), egui::StrokeKind::Inside);
    }
    let resp = resp.on_hover_text(tip);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current, tip));
    resp
}

fn heading(ui: &mut Ui, s: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(s).strong().size(12.0));
    ui.add_space(2.0);
}

fn signed(v: i32) -> String {
    if v > 0 { format!("+{v}%") } else { format!("{v}%") }
}

/// Corrections ▾: brightness across, contrast down, each from −40% to +40%.
pub(super) fn corrections_gallery(app: &mut DesignApp, ui: &mut Ui) {
    let Some((_, now)) = selected(app) else { return };
    let ctx = ui.ctx().clone();
    heading(ui, "Brightness/Contrast");
    let mut pick = None;
    egui::Grid::new("pr_corrections").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for c in STEPS {
            for b in STEPS {
                let tex = thumb(app, &ctx, &format!("b{b}c{c}"), |img| adjust::adjust(img, b, c, Recolor::None));
                let tip = format!("Brightness: {} Contrast: {}", signed(b), signed(c));
                if cell(ui, tex.as_ref(), now.brightness == b && now.contrast == c, &tip).clicked() {
                    pick = Some((b, c));
                }
            }
            ui.end_row();
        }
    });
    if let Some((b, c)) = pick {
        let _ = app.run("picture.adjust", json!({"brightness": b, "contrast": c}));
        ui.close();
    }
}

/// Recolor ▾: the colour modes, then dark and light variations in PressRoom's colours.
pub(super) fn recolor_gallery(app: &mut DesignApp, ui: &mut Ui) {
    let Some((_, now)) = selected(app) else { return };
    let ctx = ui.ctx().clone();
    let current = Recolor::from_name(&now.recolor, now.tint.unwrap_or([0, 0, 0])).unwrap_or_default();
    let mut pick = None;
    let mut row = |ui: &mut Ui, choices: Vec<(Recolor, String)>| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            for (r, tip) in choices {
                let key = format!("r{}{:?}", r.name(), r.tint());
                let tex = thumb(app, &ctx, &key, |img| adjust::adjust(img, 0, 0, r));
                if cell(ui, tex.as_ref(), current == r, &tip).clicked() {
                    pick = Some(r);
                }
            }
        });
    };
    heading(ui, "Color Modes");
    row(
        ui,
        vec![
            (Recolor::None, "No Recolor".into()),
            (Recolor::Grayscale, "Grayscale".into()),
            (Recolor::Sepia, "Sepia".into()),
            (Recolor::Washout, "Washout".into()),
            (Recolor::BlackWhite, "Black and White".into()),
        ],
    );
    heading(ui, "Dark Variations");
    row(ui, TINTS.iter().map(|(n, c)| (Recolor::Dark(*c), format!("{n}, Dark"))).collect());
    heading(ui, "Light Variations");
    row(ui, TINTS.iter().map(|(n, c)| (Recolor::Light(*c), format!("{n}, Light"))).collect());
    if let Some(r) = pick {
        let mut p = json!({"recolor": r.name()});
        if let Some([cr, cg, cb]) = r.tint() {
            p["tint"] = json!(format!("#{cr:02x}{cg:02x}{cb:02x}"));
        }
        let _ = app.run("picture.adjust", p);
        ui.close();
    }
}

pub const COMPRESS: &str = "compressPictures";

/// The bytes of every picture in the active publication.
fn pictures_size(app: &DesignApp) -> usize {
    // The ones in use: corrected copies taken off again go when the publication is saved.
    app.session.active().map_or(0, |st| st.doc.used_assets().iter().filter_map(|id| st.doc.assets.get(id)).map(|a| a.data.len()).sum())
}

fn megabytes(b: usize) -> String {
    format!("{:.1} MB", b as f64 / 1_048_576.0)
}

/// Compress Pictures…: opens its dialog.
pub(super) fn open_compress(app: &mut DesignApp) {
    let any_selected =
        app.session.active().is_some_and(|st| st.selection.items.iter().any(|id| st.doc.item(*id).is_some_and(|i| i.graphic().is_some())));
    app.ui.dialog = Some(Dialog::new(COMPRESS, json!({"ppi": 220, "deleteCropped": true, "all": !any_selected})));
}

/// The Compress Pictures dialog's body.
pub(crate) fn compress_dialog(app: &DesignApp, ui: &mut Ui, d: &mut Dialog) {
    ui.label(format!("Pictures in this publication: {}", megabytes(pictures_size(app))));
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Compression options").strong());
    let mut cropped = d.b("deleteCropped");
    if ui.checkbox(&mut cropped, "Delete cropped areas of pictures").changed() {
        d.fields.insert("deleteCropped".into(), Value::Bool(cropped));
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Resample pictures to").strong());
    let ppi = d.n("ppi").unwrap_or(220.0) as i64;
    for (v, label) in [(300, "Commercial Printing (300 ppi)"), (220, "Desktop Printing (220 ppi)"), (96, "Web (96 ppi)")] {
        if ui.radio(ppi == v, label).clicked() {
            d.fields.insert("ppi".into(), json!(v));
        }
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Apply compression settings now").strong());
    let all = d.b("all");
    if ui.radio(all, "Apply to all pictures in the publication").clicked() {
        d.fields.insert("all".into(), Value::Bool(true));
    }
    if ui.radio(!all, "Apply to selected pictures only").clicked() {
        d.fields.insert("all".into(), Value::Bool(false));
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Photos become JPEG pictures; transparent pictures stay PNG.").size(11.0).color(super::pal().group_label));
}

/// OK in Compress Pictures: compresses and says what it saved.
pub(crate) fn compress_apply(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let r = app.run("picture.compress", json!({"ppi": d.n("ppi").unwrap_or(220.0), "all": d.b("all"), "deleteCropped": d.b("deleteCropped")}))?;
    let n = r["pictures"].as_u64().unwrap_or(0);
    let (before, after) = (r["before"].as_u64().unwrap_or(0) as usize, r["after"].as_u64().unwrap_or(0) as usize);
    app.status(match n {
        0 => "The pictures are already as small as these settings make them.".to_string(),
        1 => format!("Compressed 1 picture: {} → {}.", megabytes(before), megabytes(after)),
        n => format!("Compressed {n} pictures: {} → {}.", megabytes(before), megabytes(after)),
    });
    Ok(r)
}

//! Fonts under their Windows names, as Publisher lists them: "Arial Black" and "Segoe UI Semibold"
//! are fonts of their own there, where the layout engine knows them as a family and a style (Arial
//! in Black, Segoe UI in Semibold). Only Regular, Bold and Italic stay inside a family: the Bold
//! and Italic buttons choose them.
//!
//! The Home tab's font box ([`picker`]) lists every font this way, each name in its own face, with
//! the fonts used lately first.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use egui::{Color32, Rect, Sense, Ui, vec2};

use crate::DesignApp;

/// The Windows name of `family` in `style`: the family with the style's words other than Regular,
/// Bold and Italic ("Arial" + "Black" → "Arial Black"; "Arial" + "Bold Italic" → "Arial").
pub fn windows_name(family: &str, style: &str) -> String {
    let words: Vec<&str> = style.split_whitespace().collect();
    let mut keep: Vec<&str> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        match w.to_ascii_lowercase().as_str() {
            "regular" | "normal" | "roman" | "italic" | "oblique" | "bolditalic" => {}
            "bold" => {
                // "Extra Bold", "Semi Bold": a weight's name, not the Bold button's.
                let prev = i.checked_sub(1).and_then(|j| words.get(j)).map(|p| p.to_ascii_lowercase());
                if prev.is_some_and(|p| ["extra", "ultra", "semi", "demi"].contains(&p.as_str())) {
                    keep.push(w);
                }
            }
            _ => keep.push(w),
        }
    }
    if keep.is_empty() { family.to_string() } else { format!("{family} {}", keep.join(" ")) }
}

/// Whether `style` is bold the way the Bold button sets it.
pub fn is_bold(style: &str) -> bool {
    let words: Vec<String> = style.split_whitespace().map(str::to_ascii_lowercase).collect();
    words.iter().enumerate().any(|(i, w)| {
        w == "bolditalic"
            || (w == "bold" && i.checked_sub(1).and_then(|j| words.get(j)).is_none_or(|p| !["extra", "ultra", "semi", "demi"].contains(&p.as_str())))
    })
}

/// Whether `style` is italic.
pub fn is_italic(style: &str) -> bool {
    style.split_whitespace().any(|w| matches!(w.to_ascii_lowercase().as_str(), "italic" | "oblique" | "bolditalic"))
}

/// One Windows font: the family the engine knows it by, and its styles.
#[derive(Clone)]
struct Entry {
    family: String,
    styles: Vec<String>,
}

type Table = Arc<BTreeMap<String, Entry>>;

/// The fonts by Windows name (case-insensitive key → display name, entry), read once per font scope
/// and family count.
static TABLE: Mutex<Option<(u32, usize, Table, Arc<Vec<String>>)>> = Mutex::new(None);

fn table(app: &DesignApp) -> (Table, Arc<Vec<String>>) {
    let fonts = crate::panels::fonts(app);
    let scope = crate::panels::font_scope(app);
    let families: Vec<String> = fonts.families().into_iter().filter(|f| !f.starts_with('.')).collect();
    let mut g = TABLE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((s, n, t, names)) = g.as_ref()
        && *s == scope
        && *n == families.len()
    {
        return (Arc::clone(t), Arc::clone(names));
    }
    let mut map: BTreeMap<String, Entry> = BTreeMap::new();
    let mut shown: BTreeMap<String, String> = BTreeMap::new();
    for family in &families {
        for style in fonts.styles(family) {
            if style.contains('{') {
                continue;
            }
            let name = windows_name(family, &style);
            let key = name.to_lowercase();
            shown.entry(key.clone()).or_insert(name);
            let e = map.entry(key).or_insert_with(|| Entry { family: family.clone(), styles: Vec::new() });
            if e.family == *family {
                e.styles.push(style);
            }
        }
    }
    let names: Vec<String> = shown.into_values().collect();
    let (t, n) = (Arc::new(map), Arc::new(names));
    *g = Some((scope, families.len(), Arc::clone(&t), Arc::clone(&n)));
    (t, n)
}

/// Every font's Windows name, in alphabetical order.
pub fn names(app: &DesignApp) -> Vec<String> {
    table(app).1.as_ref().clone()
}

/// The family and style for the font Windows calls `name`, bold and italic as asked where it has them.
pub fn resolve(app: &DesignApp, name: &str, bold: bool, italic: bool) -> (String, String) {
    let fonts = crate::panels::fonts(app);
    let (t, _) = table(app);
    let Some(e) = t.get(&name.to_lowercase()) else {
        // Not installed here: the name as a family.
        let style = match (bold, italic) {
            (true, true) => "Bold Italic",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (false, false) => "Regular",
        };
        return (name.to_string(), style.to_string());
    };
    let plain = e.styles.iter().find(|s| !is_bold(s) && !is_italic(s)).or(e.styles.first()).cloned().unwrap_or_else(|| "Regular".into());
    let linked = fonts.linked_style(&e.family, &plain, Some(bold), Some(italic)).filter(|s| e.styles.contains(s));
    let style = linked
        .or_else(|| e.styles.iter().find(|s| is_bold(s) == bold && is_italic(s) == italic).cloned())
        .or_else(|| e.styles.iter().find(|s| is_italic(s) == italic).cloned())
        .unwrap_or(plain);
    (e.family.clone(), style)
}

/// Previews of font names in their own faces, kept for reuse (at most [`CAP`]).
#[derive(Default)]
struct Previews {
    map: HashMap<(String, String, String, u32, [u8; 4]), egui::TextureHandle>,
    order: VecDeque<(String, String, String, u32, [u8; 4])>,
}

const CAP: usize = 400;
static PREVIEWS: Mutex<Option<Previews>> = Mutex::new(None);

/// `text` in `family`'s `style` at `size` points, as a texture.
fn preview(app: &DesignApp, ctx: &egui::Context, family: &str, style: &str, text: &str, size: f32, color: Color32) -> egui::TextureHandle {
    let px = (size * 1.4 * ctx.pixels_per_point()).round().max(4.0) as u32;
    let key = (family.to_string(), style.to_string(), text.to_string(), px, color.to_array());
    let mut g = PREVIEWS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let p = g.get_or_insert_with(Previews::default);
    if let Some(t) = p.map.get(&key) {
        return t.clone();
    }
    let img = designcraft_render::glyphs::text_line(&crate::panels::fonts(app), family, style, text, px, color.to_array());
    let ci = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
    let t = crate::widgets::load_texture(ctx, format!("pr_font_{family}_{style}"), ci, egui::TextureOptions::LINEAR);
    p.map.insert(key.clone(), t.clone());
    p.order.push_back(key);
    while p.order.len() > CAP {
        if let Some(old) = p.order.pop_front() {
            p.map.remove(&old);
        }
    }
    t
}

/// Draws `text` in `family`'s `style`, its top left at `at`, `size` points high.
pub fn paint_sample(app: &DesignApp, ui: &Ui, at: egui::Pos2, family: &str, style: &str, text: &str, size: f32) {
    let t = preview(app, ui.ctx(), family, style, text, size, super::pal().text);
    let [w, h] = t.size();
    let ppp = ui.ctx().pixels_per_point();
    let r = Rect::from_min_size(at, vec2(w as f32 / ppp, h as f32 / ppp));
    ui.painter().image(t.id(), r, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
}

/// Fonts picked lately, newest first.
static RECENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn remember(name: &str) {
    let mut r = RECENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    r.retain(|n| !n.eq_ignore_ascii_case(name));
    r.insert(0, name.to_string());
    r.truncate(6);
}

/// One row of the list: the name in its own font. True when clicked.
fn row(app: &DesignApp, ui: &mut Ui, name: &str, current: bool, width: f32) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    let pal = super::pal();
    if current {
        ui.painter().rect_filled(r, 3.0, pal.checked);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, pal.hover);
    }
    let (family, style) = resolve(app, name, false, false);
    let t = preview(app, ui.ctx(), &family, &style, name, 11.0, pal.text);
    let [w, h] = t.size();
    let ppp = ui.ctx().pixels_per_point();
    let size = vec2((w as f32 / ppp).min(width - 12.0), h as f32 / ppp);
    let img = Rect::from_min_size(egui::pos2(r.min.x + 6.0, r.center().y - size.y / 2.0), size);
    let uv_w = (size.x * ppp / w.max(1) as f32).min(1.0);
    ui.painter().image(t.id(), img, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(uv_w, 1.0)), Color32::WHITE);
    let resp = resp.on_hover_text(name);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current, name));
    resp.clicked()
}

fn section(ui: &mut Ui, s: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(s).strong().size(11.5).color(super::pal().group_label));
    ui.add_space(2.0);
}

/// The font box: the current font's Windows name; its list picks another. Returns the name picked.
pub fn picker(app: &DesignApp, ui: &mut Ui, current: &str, width: f32, enabled: bool) -> Option<String> {
    let field = ui.add_enabled_ui(enabled, |ui| crate::widgets::dropdown(ui, current, width)).inner;
    field.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::Role::ComboBox);
        info.enabled = enabled;
        info.current_text_value = Some(current.to_string());
        info.label = Some("Font".to_owned());
        info
    });
    let mut pick = None;
    let filter_id = egui::Id::unique("pr_font_filter");
    let open_id = egui::Id::unique("pr_font_list_open");
    let shown_now = egui::Popup::menu(&field).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let list_w = 260.0;
        ui.set_width(list_w);
        let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
        let edit = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Find a font").desired_width(list_w - 8.0));
        // Typing goes straight to the search, as typing in Publisher's font box does.
        if !ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false)) {
            edit.request_focus();
            ui.data_mut(|d| d.insert_temp(open_id, true));
        }
        if edit.changed() {
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
        }
        let all = names(app);
        let wanted = filter.trim().to_lowercase();
        let shown: Vec<&String> = all.iter().filter(|n| wanted.is_empty() || n.to_lowercase().contains(&wanted)).collect();
        // Enter takes the first match.
        if (edit.has_focus() || edit.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            pick = shown.first().map(|n| (*n).clone());
        }
        let recent: Vec<String> = RECENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        if wanted.is_empty() && !recent.is_empty() {
            section(ui, "Recently Used Fonts");
            for n in &recent {
                if row(app, ui, n, n.eq_ignore_ascii_case(current), list_w - 8.0) {
                    pick = Some(n.clone());
                }
            }
            ui.separator();
        }
        section(ui, "All Fonts");
        egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show_rows(ui, 24.0, shown.len(), |ui, range| {
            for n in shown.get(range).unwrap_or_default() {
                if row(app, ui, n, n.eq_ignore_ascii_case(current), list_w - 20.0) {
                    pick = Some((*n).clone());
                }
            }
        });
        if pick.is_some() {
            ui.data_mut(|d| d.remove::<String>(filter_id));
            ui.close();
        }
    });
    if shown_now.is_none() {
        ui.data_mut(|d| d.insert_temp(open_id, false));
    }
    if let Some(n) = &pick {
        remember(n);
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_names_keep_the_style_words_publisher_lists_as_fonts_of_their_own() {
        assert_eq!(windows_name("Arial", "Regular"), "Arial");
        assert_eq!(windows_name("Arial", "Bold Italic"), "Arial");
        assert_eq!(windows_name("Arial", "Black"), "Arial Black");
        assert_eq!(windows_name("Segoe UI", "Semibold Italic"), "Segoe UI Semibold");
        assert_eq!(windows_name("Montserrat", "Extra Bold"), "Montserrat Extra Bold");
        assert_eq!(windows_name("Montserrat", "ExtraBold Italic"), "Montserrat ExtraBold");
        assert_eq!(windows_name("Bahnschrift", "Light SemiCondensed"), "Bahnschrift Light SemiCondensed");
    }

    #[test]
    fn bold_and_italic_are_the_buttons_not_the_weights() {
        assert!(is_bold("Bold") && is_bold("Bold Italic") && is_bold("BoldItalic"));
        assert!(!is_bold("Semi Bold") && !is_bold("Black") && !is_bold("Regular"));
        assert!(is_italic("Italic") && is_italic("Light Oblique") && !is_italic("Regular"));
    }
}

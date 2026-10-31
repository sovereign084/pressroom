//! Insert › WordArt: a gallery of PressRoom's own WordArt styles, then Edit WordArt Text, as in
//! Publisher. The WordArt is a text box that sizes itself to its text, in a gradient or colour
//! with an outline behind the letters and a shadow or glow, so its text can be typed over in
//! place. It goes in as one undo step (`script.run`).

use egui::{Color32, FontId, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::DesignApp;
use crate::dialogs::Dialog;

/// A WordArt fill: a named colour, or a named gradient (stops: location 0–100, colour).
#[derive(Clone, Copy)]
enum Paint {
    Color(&'static str, &'static str),
    Gradient(&'static str, &'static [(u8, &'static str)]),
}

impl Paint {
    fn swatch(self) -> String {
        match self {
            Paint::Color(n, _) => format!("WordArt {n}"),
            Paint::Gradient(n, _) => format!("WordArt {n} Gradient"),
        }
    }

    /// The colour the gallery shows it in.
    fn shown(self) -> Color32 {
        match self {
            Paint::Color(_, hex) => hex_color(hex),
            Paint::Gradient(_, stops) => stops.get(stops.len() / 2).map_or(Color32::GRAY, |(_, c)| hex_color(c)),
        }
    }

    fn create(self) -> Value {
        match self {
            Paint::Color(_, hex) => json!({"name": self.swatch(), "color": hex}),
            Paint::Gradient(_, stops) => {
                let stops: Vec<Value> = stops.iter().map(|(l, c)| json!({"location": l, "color": c})).collect();
                json!({"name": self.swatch(), "gradient": {"kind": "linear", "stops": stops}})
            }
        }
    }
}

/// A WordArt style: fill, the outline behind the letters (colour, weight), a shadow, a glow.
pub struct Style {
    name: &'static str,
    fill: Paint,
    outline: Option<(Paint, f64)>,
    shadow: bool,
    glow: Option<&'static str>,
}

const GOLD: &[(u8, &str)] = &[(0, "#ffe27a"), (100, "#c47a00")];
const SKY: &[(u8, &str)] = &[(0, "#7fc4f2"), (100, "#1d4f91")];
const PURPLE: &[(u8, &str)] = &[(0, "#b58ae0"), (100, "#4b2a7b")];
const SILVER: &[(u8, &str)] = &[(0, "#f2f2f2"), (50, "#a8a8a8"), (100, "#5a5a5a")];
const RAINBOW: &[(u8, &str)] = &[(0, "#d7263d"), (25, "#f49d37"), (50, "#e8c547"), (75, "#3fa34d"), (100, "#2e5eaa")];
const SUNSET: &[(u8, &str)] = &[(0, "#f9c74f"), (50, "#f3722c"), (100, "#9b2226")];

pub const STYLES: [Style; 12] = [
    Style { name: "Fill: Black", fill: Paint::Color("Black", "#1a1a1a"), outline: None, shadow: false, glow: None },
    Style { name: "Fill: Teal; Shadow", fill: Paint::Color("Teal", "#1f5f7a"), outline: None, shadow: true, glow: None },
    Style {
        name: "Fill: White; Outline: Teal",
        fill: Paint::Color("White", "#ffffff"),
        outline: Some((Paint::Color("Teal", "#1f5f7a"), 2.5)),
        shadow: false,
        glow: None,
    },
    Style {
        name: "Fill: Navy; Outline: Gold",
        fill: Paint::Color("Navy", "#1b2a4a"),
        outline: Some((Paint::Color("Gold", "#d4a017"), 2.0)),
        shadow: false,
        glow: None,
    },
    Style {
        name: "Gold Gradient; Outline: Brown; Shadow",
        fill: Paint::Gradient("Gold", GOLD),
        outline: Some((Paint::Color("Brown", "#6b3d00"), 2.0)),
        shadow: true,
        glow: None,
    },
    Style { name: "Blue Gradient; Shadow", fill: Paint::Gradient("Sky", SKY), outline: None, shadow: true, glow: None },
    Style { name: "Fill: Burgundy; Glow: Gold", fill: Paint::Color("Burgundy", "#8b2332"), outline: None, shadow: false, glow: Some("#f2c94c") },
    Style {
        name: "Purple Gradient; Outline: White; Shadow",
        fill: Paint::Gradient("Purple", PURPLE),
        outline: Some((Paint::Color("White", "#ffffff"), 2.0)),
        shadow: true,
        glow: None,
    },
    Style {
        name: "Fill: Green; Outline: Dark Green",
        fill: Paint::Color("Green", "#5aa04a"),
        outline: Some((Paint::Color("Dark Green", "#24461c"), 2.0)),
        shadow: false,
        glow: None,
    },
    Style { name: "Rainbow Gradient", fill: Paint::Gradient("Rainbow", RAINBOW), outline: None, shadow: false, glow: None },
    Style {
        name: "Silver Gradient; Outline: Gray",
        fill: Paint::Gradient("Silver", SILVER),
        outline: Some((Paint::Color("Gray", "#4a4a4a"), 1.5)),
        shadow: true,
        glow: None,
    },
    Style {
        name: "Sunset Gradient; Outline: Dark Red",
        fill: Paint::Gradient("Sunset", SUNSET),
        outline: Some((Paint::Color("Dark Red", "#5c0f12"), 2.0)),
        shadow: false,
        glow: None,
    },
];

fn hex_color(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    let b = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0);
    Color32::from_rgb(b(0), b(2), b(4))
}

/// One gallery cell: an "A" in the style.
fn cell(ui: &mut Ui, s: &Style) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(58.0, 58.0), Sense::click());
    let pal = super::pal();
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, pal.hover);
        ui.painter().rect_stroke(r.shrink(0.5), 4.0, Stroke::new(1.0, pal.hover_border), egui::StrokeKind::Inside);
    }
    // A light tile, so white letters show.
    ui.painter().rect_filled(r.shrink(5.0), 3.0, if pal.dark { Color32::from_gray(70) } else { Color32::from_gray(238) });
    let font = FontId::proportional(34.0);
    let c = r.center();
    let text = |dx: f32, dy: f32, color: Color32| {
        ui.painter().text(c + vec2(dx, dy), egui::Align2::CENTER_CENTER, "A", font.clone(), color);
    };
    if let Some(g) = s.glow {
        let g = hex_color(g).gamma_multiply(0.45);
        for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0), (-1.5, -1.5), (1.5, 1.5), (-1.5, 1.5), (1.5, -1.5)] {
            text(dx, dy, g);
        }
    }
    if s.shadow {
        text(2.0, 2.0, Color32::from_black_alpha(90));
    }
    if let Some((p, w)) = s.outline {
        let d = (w as f32 * 0.6).max(1.0);
        for (dx, dy) in [(-d, 0.0), (d, 0.0), (0.0, -d), (0.0, d), (-d, -d), (d, d), (-d, d), (d, -d)] {
            text(dx, dy, p.shown());
        }
    }
    // The letter a little heavier, as WordArt's are.
    for (dx, dy) in [(0.0, 0.0), (0.6, 0.0), (0.0, 0.5), (0.6, 0.5)] {
        text(dx, dy, s.fill.shown());
    }
    resp.on_hover_text(s.name)
}

/// WordArt ▾: the styles; a click opens Edit WordArt Text.
pub(super) fn gallery(app: &mut DesignApp, ui: &mut Ui) {
    ui.label(egui::RichText::new("WordArt Styles").strong().size(12.0));
    ui.add_space(2.0);
    let mut pick = None;
    egui::Grid::new("pr_wordart").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for (i, s) in STYLES.iter().enumerate() {
            if cell(ui, s).clicked() {
                pick = Some(i);
            }
            if i % 4 == 3 {
                ui.end_row();
            }
        }
    });
    if let Some(i) = pick {
        ui.close();
        app.ui.dialog = Some(Dialog::new(
            DIALOG,
            json!({"style": i, "text": "Your Text Here", "font": "Arial Black", "size": 36, "bold": false, "italic": false}),
        ));
    }
}

pub const DIALOG: &str = "wordArt";

/// The swatch a WordArt glow is drawn in.
const GLOW: &str = "WordArt Glow";

const SIZES: [f64; 12] = [18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 48.0, 54.0, 60.0, 72.0, 96.0];

/// Edit WordArt Text: font, size, bold and italic, and the text.
pub(crate) fn dialog(app: &DesignApp, ui: &mut Ui, d: &mut Dialog) {
    let names = super::fontnames::names(app);
    ui.horizontal(|ui| {
        ui.label("Font:");
        let mut font = d.s("font");
        craft_ui::ComboBox::from_id_salt("pr_wordart_font").selected_text(&font).width(200.0).show_ui(ui, |ui| {
            for n in &names {
                if ui.selectable_label(*n == font, n).clicked() {
                    font = n.clone();
                }
            }
        });
        if font != d.s("font") {
            d.fields.insert("font".into(), json!(font));
        }
        ui.add_space(10.0);
        ui.label("Size:");
        let size = d.n("size").unwrap_or(36.0);
        craft_ui::ComboBox::from_id_salt("pr_wordart_size").selected_text(format!("{size}")).width(56.0).show_ui(ui, |ui| {
            for s in SIZES {
                if ui.selectable_label((s - size).abs() < 1e-6, format!("{s}")).clicked() {
                    d.fields.insert("size".into(), json!(s));
                }
            }
        });
        ui.add_space(10.0);
        for (key, label) in [("bold", "B"), ("italic", "I")] {
            let on = d.b(key);
            let text = if key == "bold" { egui::RichText::new(label).strong() } else { egui::RichText::new(label).italics() };
            if ui
                .add(egui::Button::new(text).selected(on).min_size(vec2(26.0, 22.0)))
                .on_hover_text(if key == "bold" { "Bold" } else { "Italic" })
                .clicked()
            {
                d.fields.insert(key.into(), json!(!on));
            }
        }
    });
    ui.add_space(8.0);
    ui.label("Text:");
    let mut text = d.s("text");
    let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(4).desired_width(f32::INFINITY));
    if resp.changed() {
        d.fields.insert("text".into(), json!(text));
    }
    if !d.b("focused") {
        resp.request_focus();
        // Select the placeholder so typing replaces it, as in Publisher.
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
            let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), resp.id);
        }
        d.fields.insert("focused".into(), json!(true));
    }
}

/// OK in Edit WordArt Text: the WordArt goes in the middle of the page in view.
pub(crate) fn insert(app: &mut DesignApp, d: &Dialog) -> Result<Value, String> {
    let s = STYLES.get(d.n("style").unwrap_or(0.0).max(0.0) as usize).ok_or("no such WordArt style")?;
    let text = d.s("text");
    if text.trim().is_empty() {
        return Err("Type the WordArt's text.".into());
    }
    let (family, style) = super::fontnames::resolve(app, &d.s("font"), d.b("bold"), d.b("italic"));
    let size = d.n("size").filter(|v| v.is_finite() && *v >= 1.0).unwrap_or(36.0).min(1296.0);
    let (spread, rect) = super::centre_rect(app, 0.7, 0.14).ok_or("Open a publication first.")?;
    let doc = app.session.active().map(|st| st.doc.clone()).ok_or("Open a publication first.")?;
    let mut steps: Vec<Value> = Vec::new();
    let mut paints = vec![s.fill];
    paints.extend(s.outline.map(|(p, _)| p));
    for p in paints {
        if doc.swatch(&p.swatch()).is_none() && !steps.iter().any(|st| st["params"]["name"] == json!(p.swatch())) {
            steps.push(json!({"command": "swatch.create", "params": p.create()}));
        }
    }
    if let Some(g) = s.glow
        && doc.swatch(GLOW).is_none()
    {
        steps.push(json!({"command": "swatch.create", "params": {"name": GLOW, "color": g}}));
    }
    let frame = steps.len();
    steps.push(json!({"command": "frame.create", "params": {"spread": spread, "rect": rect, "content": "text", "text": text}}));
    steps.push(json!({"command": "selection.set", "params": {"ids": [format!("${frame}.id")]}}));
    let mut attrs = json!({"fontFamily": family, "fontStyle": style, "size": size, "fill": s.fill.swatch()});
    if let Some((p, w)) = s.outline {
        attrs["stroke"] = json!(p.swatch());
        attrs["strokeWeight"] = json!(w);
        attrs["strokeBehindFill"] = json!(true);
    }
    steps.push(json!({"command": "type.char", "params": {"attrs": attrs}}));
    steps.push(json!({"command": "type.para", "params": {"attrs": {"align": "center"}}}));
    steps.push(json!({"command": "object.textFrameOptions", "params": {"autoSize": "heightAndWidth", "autoSizeReference": "center", "noLineBreaks": true, "inset": 3}}));
    let mut effects = serde_json::Map::new();
    if s.shadow {
        effects.insert("dropShadow".into(), json!({"on": true, "distance": (size / 14.0).clamp(1.5, 8.0), "size": (size / 14.0).clamp(1.5, 8.0), "opacity": 0.45, "angle": 135, "color": "[Black]"}));
    }
    if s.glow.is_some() {
        effects
            .insert("outerGlow".into(), json!({"on": true, "color": GLOW, "opacity": 0.85, "size": (size / 7.0).clamp(3.0, 14.0), "mode": "Normal"}));
    }
    if !effects.is_empty() {
        steps.push(json!({"command": "object.effects", "params": {"effects": effects}}));
    }
    app.run("script.run", json!({"text": Value::Array(steps).to_string()}))
}

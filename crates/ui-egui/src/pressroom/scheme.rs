//! Page Design › Schemes: Color Schemes and Font Schemes, PressRoom's own.
//!
//! A colour scheme is six swatches in the publication (Scheme Main and Scheme Accent 1–5, in a
//! "Color Scheme" swatch group). Building Blocks are coloured with them, so choosing another scheme
//! recolours everything that uses them, as Publisher's schemes do. A font scheme sets the font of
//! the [Basic Paragraph] style (body text) and of a Heading style (Building Blocks' headings).

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::DesignApp;

/// The swatches of a colour scheme, in order.
pub const SWATCHES: [&str; 6] = ["Scheme Main", "Scheme Accent 1", "Scheme Accent 2", "Scheme Accent 3", "Scheme Accent 4", "Scheme Accent 5"];
pub const GROUP: &str = "Color Scheme";
/// The paragraph style Building Blocks' headings use.
pub const HEADING: &str = "Heading";

/// A colour scheme: its name and its six colours (main, then accents 1 to 5).
pub struct ColorScheme {
    pub name: &'static str,
    pub colors: [&'static str; 6],
}

pub const COLOR_SCHEMES: [ColorScheme; 12] = [
    ColorScheme { name: "Harbor", colors: ["#1a1a1a", "#1f5f7a", "#d4a017", "#8b2332", "#5aa04a", "#7fa7b8"] },
    ColorScheme { name: "Meadow", colors: ["#1e2a1e", "#4e7d3a", "#a7c957", "#bc4749", "#6a994e", "#d9c88f"] },
    ColorScheme { name: "Sunrise", colors: ["#2b2b2b", "#e76f51", "#f4a261", "#2a9d8f", "#264653", "#e9c46a"] },
    ColorScheme { name: "Lavender", colors: ["#2d2438", "#6b4e9b", "#b39cd0", "#d4a5a5", "#4b3f72", "#8e7cc3"] },
    ColorScheme { name: "Ocean", colors: ["#0b2545", "#1b4965", "#3e7cb1", "#8da9c4", "#13315c", "#5fa8d3"] },
    ColorScheme { name: "Autumn", colors: ["#3b2418", "#a0522d", "#daa520", "#8b0000", "#6b8e23", "#cd853f"] },
    ColorScheme { name: "Rose", colors: ["#3a2e2e", "#b5485d", "#e8a0a8", "#7a9e7e", "#5c3d46", "#c98b8b"] },
    ColorScheme { name: "Slate", colors: ["#222222", "#4a5568", "#2b6cb0", "#718096", "#a0aec0", "#2c7a7b"] },
    ColorScheme { name: "Cranberry", colors: ["#2a1a1f", "#9b1b30", "#3d5a80", "#d35269", "#6d2e46", "#98c1d9"] },
    ColorScheme { name: "Forest", colors: ["#1b2a20", "#2d6a4f", "#95d5b2", "#40916c", "#1b4332", "#b7a76a"] },
    ColorScheme { name: "Sky", colors: ["#1c2b39", "#2e86ab", "#f6ae2d", "#f26419", "#33658a", "#86bbd8"] },
    ColorScheme { name: "Navy and Gold", colors: ["#1b2a4a", "#1b2a4a", "#c9a227", "#8c2f39", "#7a8ba6", "#e0c56e"] },
];

/// A font scheme: its name, the heading font and the body font (family, style).
pub struct FontScheme {
    pub name: &'static str,
    pub heading: (&'static str, &'static str),
    pub body: (&'static str, &'static str),
}

pub const FONT_SCHEMES: [FontScheme; 10] = [
    FontScheme { name: "Classic", heading: ("Georgia", "Bold"), body: ("Arial", "Regular") },
    FontScheme { name: "Traditional", heading: ("Times New Roman", "Bold"), body: ("Times New Roman", "Regular") },
    FontScheme { name: "Simple", heading: ("Arial", "Bold"), body: ("Arial", "Regular") },
    FontScheme { name: "Modern", heading: ("Montserrat", "Bold"), body: ("Open Sans", "Regular") },
    FontScheme { name: "Elegant", heading: ("Playfair Display", "Bold"), body: ("Lora", "Regular") },
    FontScheme { name: "Friendly", heading: ("Varela Round", "Regular"), body: ("Nunito", "Regular") },
    FontScheme { name: "Strong", heading: ("Oswald", "Bold"), body: ("PT Sans", "Regular") },
    FontScheme { name: "Fresh", heading: ("Raleway", "Bold"), body: ("Quattrocento Sans", "Regular") },
    FontScheme { name: "Bookish", heading: ("EB Garamond", "Bold"), body: ("EB Garamond", "Regular") },
    FontScheme { name: "Handwritten", heading: ("Caveat Brush", "Regular"), body: ("Patrick Hand", "Regular") },
];

pub fn hex_color(hex: &str) -> Color32 {
    let h = hex.trim_start_matches('#');
    let b = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0);
    Color32::from_rgb(b(0), b(2), b(4))
}

/// The colour scheme the publication uses: the one whose colours its scheme swatches hold.
pub fn current_color_scheme(app: &DesignApp) -> Option<usize> {
    let st = app.session.active()?;
    let rgb = |name: &str| st.doc.resolve_color(name, 1.0).map(|c| c.to_rgb());
    let ours: Vec<[f32; 3]> = SWATCHES.iter().map(|n| rgb(n)).collect::<Option<_>>()?;
    COLOR_SCHEMES.iter().position(|s| {
        s.colors.iter().zip(&ours).all(|(hex, c)| {
            let h = hex_color(hex);
            [h.r(), h.g(), h.b()].iter().zip(c).all(|(a, b)| (f32::from(*a) / 255.0 - b).abs() < 0.02)
        })
    })
}

/// Script steps that make sure the scheme swatches exist (in the publication's scheme, else
/// Harbor) and that the Heading style exists.
pub fn ensure_steps(app: &DesignApp) -> Vec<Value> {
    let Some(st) = app.session.active() else { return vec![] };
    let mut steps = Vec::new();
    let missing: Vec<usize> = (0..SWATCHES.len()).filter(|i| SWATCHES.get(*i).is_some_and(|n| st.doc.swatch(n).is_none())).collect();
    let harbor = &COLOR_SCHEMES[0];
    for i in &missing {
        if let (Some(name), Some(hex)) = (SWATCHES.get(*i), harbor.colors.get(*i)) {
            steps.push(json!({"command": "swatch.create", "params": {"name": name, "color": hex}}));
        }
    }
    if missing.len() == SWATCHES.len() {
        steps.push(json!({"command": "swatch.newColorGroup", "params": {"name": GROUP, "swatches": SWATCHES}}));
    }
    if !st.doc.styles.paragraph.iter().any(|p| p.name == HEADING) {
        let (family, style) = FONT_SCHEMES[0].heading;
        steps.push(json!({"command": "style.paragraph.create", "params": {"name": HEADING, "chars": {"fontFamily": family, "fontStyle": style}}}));
    }
    steps
}

fn run_steps(app: &mut DesignApp, steps: Vec<Value>, done: &str) {
    match app.run("script.run", json!({"text": Value::Array(steps).to_string()})) {
        Ok(_) => app.status(done),
        Err(e) => app.status(format!("Couldn't apply the scheme: {e}")),
    }
}

/// Apply colour scheme `i`: its colours into the scheme swatches, as one undo step.
pub fn apply_color_scheme(app: &mut DesignApp, i: usize) {
    let Some(s) = COLOR_SCHEMES.get(i) else { return };
    let Some(st) = app.session.active() else { return };
    let mut steps = Vec::new();
    let mut created = Vec::new();
    for (name, hex) in SWATCHES.iter().zip(s.colors) {
        if st.doc.swatch(name).is_some() {
            steps.push(json!({"command": "swatch.options", "params": {"name": name, "color": hex}}));
        } else {
            steps.push(json!({"command": "swatch.create", "params": {"name": name, "color": hex}}));
            created.push(*name);
        }
    }
    if created.len() == SWATCHES.len() {
        steps.push(json!({"command": "swatch.newColorGroup", "params": {"name": GROUP, "swatches": SWATCHES}}));
    }
    run_steps(app, steps, &format!("Color scheme: {}.", s.name));
}

/// Apply font scheme `i`: body text's font in [Basic Paragraph], headings' in Heading.
pub fn apply_font_scheme(app: &mut DesignApp, i: usize) {
    let Some(s) = FONT_SCHEMES.get(i) else { return };
    let Some(st) = app.session.active() else { return };
    let has_heading = st.doc.styles.paragraph.iter().any(|p| p.name == HEADING);
    let (hf, hs) = s.heading;
    let (bf, bs) = s.body;
    let heading = json!({"name": HEADING, "chars": {"fontFamily": hf, "fontStyle": hs}});
    let steps = vec![
        json!({"command": "style.paragraph.edit", "params": {"name": designcraft_doc::BASIC_PARAGRAPH, "chars": {"fontFamily": bf, "fontStyle": bs}}}),
        json!({"command": if has_heading { "style.paragraph.edit" } else { "style.paragraph.create" }, "params": heading}),
    ];
    run_steps(app, steps, &format!("Font scheme: {}.", s.name));
}

/// The font scheme the publication uses: the one whose fonts its two styles have.
pub fn current_font_scheme(app: &DesignApp) -> Option<usize> {
    let st = app.session.active()?;
    let font = |name: &str| {
        let p = st.doc.styles.paragraph.iter().find(|p| p.name == name)?;
        Some((p.chars.font_family.clone()?, p.chars.font_style.clone().unwrap_or_default()))
    };
    let (body, heading) = (font(designcraft_doc::BASIC_PARAGRAPH)?, font(HEADING)?);
    FONT_SCHEMES.iter().position(|s| body.0 == s.body.0 && heading.0 == s.heading.0)
}

/// Color Schemes ▾: each scheme as its strip of colours.
pub(super) fn color_gallery(app: &mut DesignApp, ui: &mut Ui) {
    ui.label(egui::RichText::new("Color Schemes").strong().size(12.0));
    ui.add_space(4.0);
    let current = current_color_scheme(app);
    let pal = super::pal();
    let mut pick = None;
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        for (i, s) in COLOR_SCHEMES.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(230.0, 30.0), Sense::click());
            if resp.hovered() || current == Some(i) {
                ui.painter().rect_filled(r, 4.0, if current == Some(i) { pal.checked } else { pal.hover });
            }
            let strip = Rect::from_min_size(r.min + vec2(6.0, 6.0), vec2(108.0, 18.0));
            let w = strip.width() / 6.0;
            for (k, hex) in s.colors.iter().enumerate() {
                let cell = Rect::from_min_size(strip.min + vec2(w * k as f32, 0.0), vec2(w, strip.height()));
                ui.painter().rect_filled(cell, 0.0, hex_color(hex));
            }
            ui.painter().rect_stroke(strip, 2.0, Stroke::new(1.0, pal.card_edge), egui::StrokeKind::Outside);
            ui.painter().text(
                egui::pos2(strip.max.x + 10.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                s.name,
                egui::FontId::proportional(13.0),
                pal.text,
            );
            if resp.clicked() {
                pick = Some(i);
            }
        }
    });
    if let Some(i) = pick {
        apply_color_scheme(app, i);
        ui.close();
    }
}

/// Fonts ▾: each font scheme as its heading and body fonts, each in its own face.
pub(super) fn font_gallery(app: &mut DesignApp, ui: &mut Ui) {
    ui.label(egui::RichText::new("Font Schemes").strong().size(12.0));
    ui.add_space(4.0);
    let current = current_font_scheme(app);
    let pal = super::pal();
    let mut pick = None;
    egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
        for (i, s) in FONT_SCHEMES.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(240.0, 58.0), Sense::click());
            if resp.hovered() || current == Some(i) {
                ui.painter().rect_filled(r, 4.0, if current == Some(i) { pal.checked } else { pal.hover });
            }
            ui.painter().text(r.min + vec2(8.0, 10.0), egui::Align2::LEFT_CENTER, s.name, egui::FontId::proportional(11.5), pal.group_label);
            super::fontnames::paint_sample(app, ui, r.min + vec2(8.0, 18.0), s.heading.0, s.heading.1, s.heading.0, 17.0);
            super::fontnames::paint_sample(app, ui, r.min + vec2(8.0, 39.0), s.body.0, s.body.1, s.body.0, 13.0);
            if resp.clicked() {
                pick = Some(i);
            }
        }
    });
    if let Some(i) = pick {
        apply_font_scheme(app, i);
        ui.close();
    }
}

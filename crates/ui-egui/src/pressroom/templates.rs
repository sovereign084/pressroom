//! PressRoom's built-in starting points (original designs): text boxes and picture boxes placed on
//! a new publication, ready to type over.

use serde_json::{Value, json};

use crate::DesignApp;

/// Page `abs`: (spread index, left edge in spread coordinates, width, height), in points.
fn page(app: &DesignApp, abs: usize) -> Option<(usize, f64, f64, f64)> {
    let st = app.session.active()?;
    let (si, pi) = st.doc.page_loc(abs)?;
    let p = st.doc.spreads.get(si)?.pages.get(pi)?;
    Some((si, p.x, p.width, p.height))
}

/// A text box at fractions of page `abs` (left, top, right, bottom), with `text` in `size` pt.
fn text(app: &mut DesignApp, abs: usize, f: [f64; 4], text: &str, size: f64, attrs: Value) {
    let Some((si, x, w, h)) = page(app, abs) else { return };
    let rect = [x + f[0] * w, f[1] * h, x + f[2] * w, f[3] * h];
    if let Ok(made) = app.run("frame.create", json!({"spread": si, "rect": rect, "content": "text", "text": text})) {
        // The frame itself, not a caret in it: the formatting is for all of its text.
        if let Some(id) = made.get("id") {
            let _ = app.run("selection.set", json!({"ids": [id]}));
        }
        let mut a = json!({"size": size});
        if let (Some(m), Some(extra)) = (a.as_object_mut(), attrs.as_object()) {
            for (k, v) in extra {
                m.insert(k.clone(), v.clone());
            }
        }
        let _ = app.run("type.char", json!({"attrs": a}));
    }
}

fn centred(app: &mut DesignApp) {
    let _ = app.run("type.alignCenter", json!({}));
}

/// A picture box (an empty graphic frame: Pictures fills it).
fn picture(app: &mut DesignApp, abs: usize, f: [f64; 4]) {
    let Some((si, x, w, h)) = page(app, abs) else { return };
    let rect = [x + f[0] * w, f[1] * h, x + f[2] * w, f[3] * h];
    let _ = app.run("frame.create", json!({"spread": si, "rect": rect, "content": "graphic"}));
}

pub(super) fn fill(app: &mut DesignApp, kind: &str) {
    match kind {
        "bulletin" => bulletin(app),
        "newsletter" => newsletter(app),
        "flyer" => flyer(app),
        "card" => card(app),
        _ => {}
    }
    let _ = app.run("edit.deselectAll", json!({}));
    let _ = app.run("tool.select", json!({"tool": "selection"}));
}

fn bulletin(app: &mut DesignApp) {
    // Cover.
    text(app, 0, [0.08, 0.06, 0.92, 0.16], "Church Name", 22.0, json!({"fontStyle": "Bold"}));
    centred(app);
    text(app, 0, [0.08, 0.16, 0.92, 0.22], "123 Main Street · Your Town", 10.0, json!({}));
    centred(app);
    picture(app, 0, [0.12, 0.26, 0.88, 0.62]);
    text(app, 0, [0.08, 0.66, 0.92, 0.74], "Sunday Worship", 18.0, json!({}));
    centred(app);
    text(app, 0, [0.08, 0.74, 0.92, 0.8], &super::today(), 11.0, json!({}));
    centred(app);
    text(app, 0, [0.1, 0.84, 0.9, 0.95], "Welcome! We're so glad you're here.", 10.0, json!({}));
    centred(app);
    // Order of worship.
    text(app, 1, [0.08, 0.06, 0.92, 0.13], "Order of Worship", 16.0, json!({"fontStyle": "Bold"}));
    centred(app);
    text(
        app,
        1,
        [0.08, 0.15, 0.92, 0.94],
        "Prelude\nWelcome & Announcements\nCall to Worship\nHymn\nPrayer\nScripture Reading\nMessage\nHymn\nOffering\nBenediction\nPostlude",
        11.0,
        json!({}),
    );
    // Announcements.
    text(app, 2, [0.08, 0.06, 0.92, 0.13], "Announcements", 16.0, json!({"fontStyle": "Bold"}));
    centred(app);
    text(
        app,
        2,
        [0.08, 0.15, 0.92, 0.94],
        "Bible Study – Wednesday at 7:00 pm\n\nCoffee & fellowship after the service\n\nVolunteers needed for next month's community dinner",
        11.0,
        json!({}),
    );
    // Back cover.
    text(app, 3, [0.08, 0.06, 0.92, 0.13], "Prayer Requests", 16.0, json!({"fontStyle": "Bold"}));
    centred(app);
    text(app, 3, [0.08, 0.15, 0.92, 0.7], "Please keep these in your prayers this week:\n\n", 11.0, json!({}));
    text(app, 3, [0.08, 0.76, 0.92, 0.94], "Pastor: Name\nPhone: (000) 000-0000\nEmail: office@yourchurch.org", 10.0, json!({}));
    centred(app);
}

fn newsletter(app: &mut DesignApp) {
    text(app, 0, [0.06, 0.05, 0.94, 0.15], "Newsletter Title", 36.0, json!({"fontStyle": "Bold"}));
    text(app, 0, [0.06, 0.15, 0.94, 0.19], &format!("Volume 1 · {}", super::today()), 10.0, json!({}));
    picture(app, 0, [0.06, 0.22, 0.48, 0.48]);
    text(app, 0, [0.06, 0.5, 0.48, 0.55], "Lead Story Headline", 16.0, json!({"fontStyle": "Bold"}));
    text(
        app,
        0,
        [0.06, 0.56, 0.48, 0.94],
        "Click here and start typing. This story can continue in the next text box: select this box, click Create Link on the Format tab, then click the box it should continue in.",
        10.5,
        json!({}),
    );
    text(app, 0, [0.52, 0.22, 0.94, 0.27], "Secondary Story", 14.0, json!({"fontStyle": "Bold"}));
    text(app, 0, [0.52, 0.28, 0.94, 0.94], "Type your story here.", 10.5, json!({}));
    for p in 1..4 {
        text(app, p, [0.06, 0.05, 0.94, 0.11], "Inside Story Headline", 16.0, json!({"fontStyle": "Bold"}));
        text(app, p, [0.06, 0.12, 0.48, 0.94], "Type your story here.", 10.5, json!({}));
        text(app, p, [0.52, 0.12, 0.94, 0.94], "", 10.5, json!({}));
    }
}

fn flyer(app: &mut DesignApp) {
    text(app, 0, [0.08, 0.06, 0.92, 0.2], "EVENT TITLE", 44.0, json!({"fontStyle": "Bold"}));
    centred(app);
    picture(app, 0, [0.12, 0.24, 0.88, 0.62]);
    text(app, 0, [0.08, 0.66, 0.92, 0.74], "Date · Time · Place", 20.0, json!({}));
    centred(app);
    text(app, 0, [0.12, 0.76, 0.88, 0.94], "A few words about the event: what it is, who it's for, and what to bring.", 13.0, json!({}));
    centred(app);
}

fn card(app: &mut DesignApp) {
    picture(app, 0, [0.12, 0.12, 0.88, 0.6]);
    text(app, 0, [0.08, 0.66, 0.92, 0.8], "Thinking of You", 24.0, json!({}));
    centred(app);
    text(app, 2, [0.1, 0.35, 0.9, 0.65], "Your message here.", 14.0, json!({}));
    centred(app);
}

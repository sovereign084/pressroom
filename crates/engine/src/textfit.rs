//! Text Fit (Text Box Tools › Text Fit): Best Fit and Shrink Text On Overflow. The sizes the person
//! set stay as they are in the story: each box keeps a scale (`TextFrameOptions::fit_scale`) that the
//! composer applies, so text that shrank can grow back when some is deleted, and styles keep reaching
//! it. [`apply`] runs after every undoable command, beside auto-size, so a fit is part of the same
//! undo step and undoing never fits again. Opening a file leaves the scale the file has.
//!
//! Scope: a story in one box. Linked boxes share their sizes, so a scale wouldn't belong to any one
//! of them; a box that gets linked goes back to full size.

use std::sync::Arc;

use designcraft_compose::{ComposeOptions, compose, frame_specs};
use designcraft_doc::{Document, Item, ItemId, StoryId, TextFit};

/// The smallest and largest scale: text stays readable, and a short word doesn't fill a poster.
const MIN_SCALE: f64 = 0.25;
const MAX_SCALE: f64 = 8.0;
/// Bisection steps: within 2% of the largest scale that fits.
const STEPS: usize = 9;
/// A scale this close to the current one isn't worth changing.
const EPS: f64 = 0.015;

/// Whether the story fits its box at scale `k`.
fn fits(d: &Document, sid: StoryId, k: f64) -> bool {
    let Some(story) = d.story(sid) else { return true };
    let mut specs = frame_specs(d, sid);
    for s in &mut specs {
        s.opts.fit_scale = k;
    }
    !compose(d, story, &specs, &ComposeOptions::default()).is_overset()
}

/// The scale the box's setting asks for now.
fn wanted(d: &Document, sid: StoryId, fit: TextFit, current: f64) -> f64 {
    let top = if fit == TextFit::Best { MAX_SCALE } else { 1.0 };
    if d.story(sid).is_none_or(|s| s.text.trim().is_empty()) {
        return 1.0;
    }
    // Most edits leave the fit as it is.
    if fit == TextFit::Shrink && fits(d, sid, 1.0) {
        return 1.0;
    }
    if (MIN_SCALE..=top).contains(&current) && fits(d, sid, current) && (current >= top || !fits(d, sid, (current * (1.0 + EPS)).min(top))) {
        return current;
    }
    if !fits(d, sid, MIN_SCALE) {
        return MIN_SCALE;
    }
    let (mut lo, mut hi) = (MIN_SCALE, top);
    if fits(d, sid, hi) {
        return hi;
    }
    for _ in 0..STEPS {
        let mid = (lo * hi).sqrt();
        if fits(d, sid, mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// The fitted boxes (id, story, setting, scale) that `before` had with other text, styles or
/// geometry; all of them without it.
fn candidates(d: &Document, before: Option<&Document>) -> Vec<(ItemId, StoryId, TextFit, f64)> {
    fn walk(items: &[Arc<Item>], depth: usize, out: &mut Vec<(ItemId, StoryId, TextFit, f64)>) {
        if depth > 64 {
            return;
        }
        for it in items {
            if let Some(tf) = it.text_frame()
                && tf.options.text_fit != TextFit::None
            {
                out.push((it.id, tf.story, tf.options.text_fit, tf.options.fit_scale));
            }
            walk(it.children(), depth + 1, out);
        }
    }
    let mut all = Vec::new();
    for sp in d.spreads.iter().chain(d.parents.iter()) {
        walk(&sp.items, 0, &mut all);
    }
    let Some(b) = before else { return all };
    let styles_same = Arc::ptr_eq(&d.styles, &b.styles);
    all.retain(|(fid, sid, _, _)| {
        let story_same = match (d.stories.get(sid), b.stories.get(sid)) {
            (Some(x), Some(y)) => Arc::ptr_eq(x, y),
            _ => false,
        };
        let frame_same = match (d.item(*fid), b.item(*fid)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
        !(styles_same && story_same && frame_same)
    });
    all
}

/// Refits the boxes the edit changed. Returns whether any scale changed.
pub fn apply(d: &mut Document, before: Option<&Document>) -> bool {
    let mut changed = false;
    for (fid, sid, fit, current) in candidates(d, before) {
        let linked = d.story(sid).is_some_and(|s| s.frames.len() != 1);
        let k = if linked { 1.0 } else { wanted(d, sid, fit, current) };
        if (k - current).abs() < 1e-9 {
            continue;
        }
        if let Some(tf) = d.item_mut(fid).and_then(Item::text_frame_mut) {
            tf.options.fit_scale = k;
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    const WORDS: &str = "The quick brown fox jumps over the lazy dog, and the dog never minded it one bit at all that day. ";

    fn doc_with_box(s: &mut Session, text: &str) -> u64 {
        s.execute("file.new", &json!({})).unwrap();
        s.execute("frame.create", &json!({"rect": [72, 72, 272, 132], "content": "text", "text": text})).unwrap()["id"].as_u64().unwrap()
    }

    fn scale(s: &Session, id: u64) -> f64 {
        s.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().options.fit_scale
    }

    fn overset(s: &Session, id: u64) -> bool {
        let d = &s.doc().unwrap().doc;
        let story = d.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().story;
        s.cache.get(d, story, None).is_overset()
    }

    #[test]
    fn shrink_on_overflow_makes_text_fit_and_leaves_its_sizes_alone() {
        let mut s = Session::new();
        let id = doc_with_box(&mut s, &WORDS.repeat(6));
        assert!(overset(&s, id));
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "shrink"})).unwrap();
        assert!(scale(&s, id) < 1.0 && !overset(&s, id), "scale {}", scale(&s, id));
        let d = &s.doc().unwrap().doc;
        let story = d.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().story;
        assert!(d.story(story).unwrap().chars.iter().all(|r| r.format.over.size.is_none()), "the story's own sizes stay");
    }

    #[test]
    fn text_that_shrank_grows_back_when_some_is_deleted() {
        let mut s = Session::new();
        let id = doc_with_box(&mut s, &WORDS.repeat(6));
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "shrink"})).unwrap();
        let small = scale(&s, id);
        let story = s.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().story.0;
        let len = s.doc().unwrap().doc.story(designcraft_doc::StoryId(story)).unwrap().text.len();
        s.execute("story.replaceRange", &json!({"story": story, "start": 5, "end": len, "text": ""})).unwrap();
        assert!(small < 1.0);
        assert_eq!(scale(&s, id), 1.0, "shrink never goes above full size");
    }

    #[test]
    fn best_fit_also_grows_short_text() {
        let mut s = Session::new();
        let id = doc_with_box(&mut s, "Hi");
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "best"})).unwrap();
        assert!(scale(&s, id) > 1.5 && !overset(&s, id), "scale {}", scale(&s, id));
    }

    #[test]
    fn a_fit_is_one_undo_step_and_undo_doesnt_fit_again() {
        let mut s = Session::new();
        let id = doc_with_box(&mut s, &WORDS.repeat(6));
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "shrink"})).unwrap();
        assert!(scale(&s, id) < 1.0);
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(scale(&s, id), 1.0);
        let o = s.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().options.clone();
        assert_eq!(o.text_fit, designcraft_doc::TextFit::None);
        assert!(overset(&s, id));
        s.execute("edit.redo", &json!({})).unwrap();
        assert!(scale(&s, id) < 1.0);
    }

    #[test]
    fn fit_and_autosize_exclude_each_other() {
        let mut s = Session::new();
        let id = doc_with_box(&mut s, &WORDS.repeat(6));
        let opts = |s: &Session| s.doc().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().text_frame().unwrap().options.clone();
        s.execute("object.textFrameOptions", &json!({"ids": [id], "autoSize": "heightOnly"})).unwrap();
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "best"})).unwrap();
        assert_eq!(opts(&s).auto_size, designcraft_doc::AutoSize::Off);
        s.execute("object.textFrameOptions", &json!({"ids": [id], "autoSize": "heightOnly"})).unwrap();
        assert_eq!(opts(&s).text_fit, designcraft_doc::TextFit::None);
        assert_eq!(opts(&s).fit_scale, 1.0);
        assert!(s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "huge"})).is_err());
    }

    #[test]
    fn a_box_too_small_for_its_text_stops_at_the_smallest_size_without_trouble() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [72, 72, 80, 76], "content": "text", "text": WORDS.repeat(50)})).unwrap()["id"]
            .as_u64()
            .unwrap();
        s.execute("object.textFrameOptions", &json!({"ids": [id], "textFit": "best"})).unwrap();
        assert_eq!(scale(&s, id), super::MIN_SCALE);
    }

    #[test]
    fn old_files_without_the_setting_load_at_full_size() {
        let o: designcraft_doc::TextFrameOptions = serde_json::from_str("{}").unwrap();
        assert_eq!((o.text_fit, o.fit_scale), (designcraft_doc::TextFit::None, 1.0));
        let v = serde_json::to_value(&o).unwrap();
        assert!(v.get("fitScale").is_none() && v.get("textFit").is_none(), "nothing written until used: {v}");
    }
}

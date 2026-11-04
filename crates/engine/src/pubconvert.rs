//! Brings in a Publisher file with PressRoom's own reader ([`designcraft_pubread`]): no copy of
//! Publisher, no other program. The steps it makes are the ones the Publisher route's script makes,
//! run on a session of their own, and the publication is saved beside the original. The `.pub` is only
//! ever read.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use designcraft_color::swatch::NONE;
use designcraft_doc::item::VerticalJustification;
use designcraft_doc::{Document, ItemId, SpreadRef, StoryId};
use designcraft_geom::kurbo;
use designcraft_pubread::fontmap::SystemFonts;
use serde_json::{Value, json};

use crate::script::resolve;

/// What came of a conversion.
#[derive(Clone, Debug, Default)]
pub struct Converted {
    /// What didn't come across, in words for the person who opened the file.
    pub notes: Vec<String>,
    /// One line for the log: how many of what.
    pub summary: String,
}

/// A folder of its own for the pictures on their way into the publication.
fn picture_folder() -> Result<PathBuf, String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("PressRoom-pub-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make a working folder: {e}"))?;
    Ok(dir)
}

/// The same note many times over reads as one, with how many.
fn gather(notes: Vec<String>) -> Vec<String> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for n in notes {
        match out.iter_mut().find(|(t, _)| *t == n) {
            Some((_, c)) => *c += 1,
            None => out.push((n, 1)),
        }
    }
    out.into_iter().map(|(t, c)| if c > 1 { format!("{t} ({c} times)") } else { t }).collect()
}

/// How much taller, as a share of its height, a plain text box may grow to show all its text.
const MOST_GROWTH: f64 = 0.6;
/// Each try at that, in points.
const GROWTH_STEP: f64 = 3.0;
/// Growth too small to mention, in points: about a line.
const SMALL_GROWTH: f64 = 12.0;

/// The page holding frame `id`: its number, counted from 1 (none on a master page), and how far down it ends.
fn page_of_frame(doc: &Document, id: ItemId) -> Option<(Option<usize>, f64)> {
    let middle = doc.item(id)?.bounds().center().x;
    let loc = doc.find(id)?;
    let pages = &doc.spread(loc.spread)?.pages;
    let (n, page) = pages.iter().enumerate().find(|(_, p)| p.x <= middle && middle <= p.x + p.width).or_else(|| pages.first().map(|p| (0, p)))?;
    let number = match loc.spread {
        SpreadRef::Doc(at) => Some(doc.first_page_of_spread(at) + n + 1),
        SpreadRef::Parent(_) => None,
    };
    Some((number, page.height))
}

/// A box holding only a page number, with or without a word before it: Publisher files often keep one
/// behind another, so one that can't show is no news.
fn is_page_number(text: &str) -> bool {
    let is_field = |c: char| ('\u{E000}'..='\u{E1FF}').contains(&c);
    text.chars().any(is_field) && text.chars().filter(|c| !is_field(*c) && !c.is_whitespace()).count() <= 8
}

/// Whether anything but spaces and line ends of `story` is out of sight.
fn has_hidden_text(session: &crate::Session, story: StoryId) -> bool {
    let Ok(state) = session.doc() else { return false };
    let Some(text) = state.doc.stories.get(&story) else { return false };
    let Some(at) = session.cache.get(&state.doc, story, None).overset_at else { return false };
    text.text.get(at..).is_some_and(|rest| rest.chars().any(|c| !c.is_whitespace() && !c.is_control()))
}

/// Makes a box with nothing to show its edges taller until `story` fits, top aligned so nothing moves:
/// how much taller it was made. Left as it was if that isn't enough or wouldn't stay on the page.
fn grow_to_fit(session: &mut crate::Session, story: StoryId, frame: ItemId) -> Option<f64> {
    let (from, bottom) = {
        let doc = &session.doc().ok()?.doc;
        let item = doc.item(frame)?;
        let [_, b, c, _, _, _] = item.xf.as_coeffs();
        let plain = item.fill.swatch == NONE && (item.stroke.swatch == NONE || item.stroke.weight <= 0.0) && b.abs() < 1e-6 && c.abs() < 1e-6;
        let top_aligned = item.text_frame().is_some_and(|t| t.options.vertical_justification == VerticalJustification::Top);
        if !plain || !top_aligned {
            return None;
        }
        (item.bounds(), page_of_frame(doc, frame)?.1)
    };
    let most = (from.height() * MOST_GROWTH).min(bottom - from.y1);
    let mut grown = 0.0;
    let mut current = from;
    while grown + GROWTH_STEP <= most {
        grown += GROWTH_STEP;
        let to = kurbo::Rect::new(from.x0, from.y0, from.x1, from.y1 + grown);
        let rect = |r: kurbo::Rect| json!([r.x0, r.y0, r.x1, r.y1]);
        session.execute("transform.resize", &json!({"ids": [frame.0], "from": rect(current), "to": rect(to)})).ok()?;
        current = to;
        if !has_hidden_text(session, story) {
            return Some(grown);
        }
    }
    let rect = |r: kurbo::Rect| json!([r.x0, r.y0, r.x1, r.y1]);
    let _ = session.execute("transform.resize", &json!({"ids": [frame.0], "from": rect(current), "to": rect(from)}));
    None
}

/// What the finished publication needs, and what the person should be told: fonts standing in for ones
/// this computer doesn't have, and text pushed out of its box. A look-alike font or a line that
/// fits in Publisher by a hair can do that. Boxes grow to bring it back where nothing shows it; text
/// still out of sight is there, and is said.
fn publication_notes(session: &mut crate::Session) -> Vec<String> {
    let mut notes = Vec::new();
    if let Ok(fonts) = session.execute("font.list", &json!({})) {
        let mut missing: Vec<&str> =
            fonts.as_array().into_iter().flatten().filter(|f| f["missing"] == json!(true)).filter_map(|f| f["family"].as_str()).collect();
        missing.sort_unstable();
        missing.dedup();
        if !missing.is_empty() {
            notes.push(format!("These fonts aren't on this computer, so look-alikes stand in and text may fit differently: {}.", missing.join(", ")));
        }
    }
    let stories: Vec<(StoryId, Option<ItemId>, String)> = match session.doc() {
        Ok(state) => state
            .doc
            .stories
            .values()
            .filter(|s| !is_page_number(&s.text))
            .map(|s| (s.id, s.frames.last().copied(), s.text.chars().take(40).map(|c| if c.is_control() { ' ' } else { c }).collect()))
            .collect(),
        Err(_) => return notes,
    };
    for (story, frame, words) in stories {
        if !has_hidden_text(session, story) {
            continue;
        }
        let grew = frame.and_then(|f| grow_to_fit(session, story, f));
        let page = frame.and_then(|f| session.doc().ok().and_then(|state| page_of_frame(&state.doc, f))).and_then(|(n, _)| n);
        let start = page.map_or_else(|| "The".to_string(), |n| format!("On page {n}, the"));
        let words = words.trim();
        match grew {
            Some(extra) if extra <= SMALL_GROWTH => {}
            Some(_) => notes.push(format!("{start} text box beginning \u{201c}{words}\u{201d} was made taller so all its text shows.")),
            None => notes.push(format!("{start} text beginning \u{201c}{words}\u{201d} no longer all fits in its box. The rest is still there: make the box bigger or the text smaller to bring it back.")),
        }
    }
    notes
}

/// A publication's notes file: what didn't come across, beside it.
pub fn notes_path(out: &Path) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Publication".into());
    out.with_file_name(format!("{stem} - what didn't come across.txt"))
}

/// Keeps `notes` about publication `out`, brought in from the file called `name`, in its notes file.
/// With none, an earlier list is taken away rather than left to mislead.
pub fn keep_notes(name: &str, out: &Path, notes: &[String]) {
    let path = notes_path(out);
    if notes.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let text = format!("What didn't come across exactly from {name}:\n\n{}\n", notes.join("\n"));
    if let Err(e) = crate::io::write_file(&path, text.as_bytes()) {
        log::warn!("couldn't keep the notes on {name}: {e}");
    }
}

/// The notes [`keep_notes`] kept about publication `out`.
pub fn kept_notes(out: &Path) -> Vec<String> {
    let Ok(bytes) = crate::io::read(notes_path(out)) else { return Vec::new() };
    String::from_utf8_lossy(&bytes).lines().skip(2).filter(|l| !l.trim().is_empty()).map(str::to_owned).collect()
}

/// The same look over a publication the Publisher route made as this reader gives its own: boxes grow
/// to show their text, and it is saved only if one did. What should be told comes back in words.
pub fn finish(path: &Path) -> Result<Vec<String>, String> {
    let mut session = crate::Session::new();
    session.execute("file.open", &json!({"path": path.to_string_lossy()})).map_err(|e| format!("Couldn't open the publication: {e}"))?;
    let notes = gather(publication_notes(&mut session));
    if session.documents().iter().any(|d| d.is_dirty()) {
        session.execute("file.saveAs", &json!({"path": path.to_string_lossy()})).map_err(|e| format!("Couldn't save the publication: {e}"))?;
    }
    Ok(notes)
}

/// Reads `src` and writes the publication to `out`, saying what it is doing on `say`. An error is
/// in words for a person; a file that can't be read leaves nothing behind.
pub fn convert(src: &Path, out: &Path, say: &mut dyn FnMut(&str)) -> Result<Converted, String> {
    say("Reading the Publisher file");
    let bytes = crate::io::read(src).map_err(|e| format!("Couldn't read the file: {e}"))?;
    let folder = picture_folder()?;
    let result = convert_bytes(&bytes, out, &folder, say);
    let _ = std::fs::remove_dir_all(&folder);
    result
}

fn convert_bytes(bytes: &[u8], out: &Path, folder: &Path, say: &mut dyn FnMut(&str)) -> Result<Converted, String> {
    // A last resort: a bad file is an error, never a lost window.
    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let publication = designcraft_pubread::read(bytes).map_err(|e| e.to_string())?;
        let fonts = SystemFonts::default();
        Ok::<_, String>(designcraft_pubread::steps::steps(&publication, &fonts, folder))
    }))
    .map_err(|_| "This Publisher file looks damaged: reading it went wrong.".to_string())??;

    say("Building the PressRoom publication");
    let mut session = crate::Session::new();
    let mut results: Vec<Value> = Vec::with_capacity(made.steps.len());
    let mut failed = 0usize;
    for step in &made.steps {
        let command = step["command"].as_str().unwrap_or_default();
        let done = resolve(&step["params"], &results).and_then(|p| session.execute(command, &p).map_err(|e| e.to_string()));
        match done {
            Ok(v) => results.push(v),
            Err(e) => {
                failed += 1;
                log::warn!("bringing in a Publisher file, {command}: {e}");
                results.push(json!({"error": e, "command": command}));
            }
        }
    }
    let mut notes = made.notes;
    if failed > 0 {
        notes.push(format!("{failed} of the steps that rebuild this publication didn't work, so something may be missing or look different."));
    }
    notes.extend(publication_notes(&mut session));
    session.execute("file.saveAs", &json!({"path": out.to_string_lossy()})).map_err(|e| format!("Couldn't save the publication: {e}"))?;
    let s = &made.stats;
    let summary = format!(
        "{} text boxes ({} linked boxes), {} pictures, {} shapes, {} lines, {} tables ({} cells of mixed formatting simplified), {} objects Publisher never draws left out; {} steps, {failed} problems",
        s.text,
        s.linked,
        s.pictures,
        s.shapes,
        s.lines,
        s.tables,
        s.mixed_cells,
        s.left_out,
        made.steps.len()
    );
    Ok(Converted { notes: gather(notes), summary })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_publisher_says_so_and_leaves_nothing() {
        let dir = std::env::temp_dir().join(format!("pr-pubconvert-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("not.pub");
        std::fs::write(&src, b"this is not a publisher file").unwrap();
        let out = dir.join("not.pressroom");
        let err = convert(&src, &out, &mut |_| {}).unwrap_err();
        assert!(err.contains("isn't a Publisher file"), "{err}");
        assert!(!out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    const LONG: &str = "Far too many words for so small a box, on and on and on and on and on and on.";

    fn with_box(rect: Value, text: &str) -> (crate::Session, u64) {
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": rect, "content": "text", "text": text})).unwrap()["id"].as_u64().unwrap();
        s.execute("object.stroke", &json!({"swatch": "[None]", "ids": [id]})).unwrap();
        (s, id)
    }

    fn height(s: &crate::Session, id: u64) -> f64 {
        s.doc().unwrap().doc.item(ItemId(id)).unwrap().bounds().height()
    }

    /// A box nothing shows the edges of grows to bring its text back, and says so when that shows.
    #[test]
    fn a_plain_box_grows_until_its_text_fits() {
        let (mut s, id) = with_box(json!([40, 40, 200, 70]), LONG);
        let before = height(&s, id);
        let notes = publication_notes(&mut s);
        assert!(height(&s, id) > before);
        assert!(s.doc().unwrap().doc.stories.values().all(|t| s.cache.get(&s.doc().unwrap().doc, t.id, None).overset_at.is_none()), "{notes:?}");
    }

    /// Too much for that: left alone, and the hidden text is told, with its page.
    #[test]
    fn text_a_box_cant_grow_to_show_is_told_with_its_page() {
        let (mut s, id) = with_box(json!([40, 40, 90, 60]), &LONG.repeat(6));
        let before = height(&s, id);
        let notes = publication_notes(&mut s);
        assert_eq!(height(&s, id), before);
        assert!(
            notes.iter().any(|n| n.contains("On page 1, the text beginning \u{201c}Far too many") && n.contains("no longer all fits")),
            "{notes:?}"
        );
    }

    /// A box that shows its edges keeps its size.
    #[test]
    fn a_filled_box_keeps_its_size() {
        let (mut s, id) = with_box(json!([40, 40, 200, 60]), LONG);
        s.execute("object.color", &json!({"ids": [id], "color": "#663300", "target": "fill"})).unwrap();
        let before = height(&s, id);
        let notes = publication_notes(&mut s);
        assert_eq!(height(&s, id), before);
        assert!(notes.iter().any(|n| n.contains("no longer all fits")), "{notes:?}");
    }

    fn saved(rect: Value, text: &str, name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pr-pubfinish-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("made.pressroom");
        let (mut s, _) = with_box(rect, text);
        s.execute("file.saveAs", &json!({"path": file.to_string_lossy()})).unwrap();
        file
    }

    /// What the Publisher route makes gets the look the reader's own does, and the box's growth stays.
    #[test]
    fn finishing_a_publication_brings_its_text_back_and_saves_it() {
        let file = saved(json!([40, 40, 200, 70]), LONG, "grow");
        let before = std::fs::read(&file).unwrap();
        finish(&file).unwrap();
        assert_ne!(std::fs::read(&file).unwrap(), before, "the growth is saved");
        let mut s = crate::Session::new();
        s.execute("file.open", &json!({"path": file.to_string_lossy()})).unwrap();
        let state = s.doc().unwrap();
        assert!(state.doc.stories.values().all(|t| s.cache.get(&state.doc, t.id, None).overset_at.is_none()), "all the text shows");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    /// Nothing to do leaves the file as it was, down to the byte.
    #[test]
    fn finishing_a_publication_with_nothing_to_do_leaves_it_alone() {
        let file = saved(json!([40, 40, 300, 300]), "A few words.", "alone");
        let before = std::fs::read(&file).unwrap();
        let notes = finish(&file).unwrap();
        assert!(notes.iter().all(|n| !n.contains("fits") && !n.contains("taller")), "{notes:?}");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn finishing_what_isnt_there_is_an_error_in_words() {
        let err = finish(&std::env::temp_dir().join("pr-pubfinish-no-such.pressroom")).unwrap_err();
        assert!(err.starts_with("Couldn't open the publication"), "{err}");
    }

    #[test]
    fn notes_are_kept_beside_the_publication_and_read_back() {
        let dir = std::env::temp_dir().join(format!("pr-pubnotes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("made.pressroom");
        keep_notes("made.pub", &out, &["One thing.".to_string(), "Another \u{201c}thing\u{201d}.".to_string()]);
        assert_eq!(kept_notes(&out), ["One thing.", "Another \u{201c}thing\u{201d}."]);
        assert!(std::fs::read_to_string(notes_path(&out)).unwrap().starts_with("What didn't come across exactly from made.pub:"));
        keep_notes("made.pub", &out, &[]);
        assert!(kept_notes(&out).is_empty() && !notes_path(&out).exists(), "an earlier list goes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_page_number_box_is_told_apart_from_real_text() {
        assert!(is_page_number("Page \u{E000}"));
        assert!(is_page_number("\u{E000}\r"));
        assert!(!is_page_number("Page 2"));
        assert!(!is_page_number("See the notice on page \u{E000}"));
    }

    /// Only an empty line out of sight is nothing to tell.
    #[test]
    fn empty_lines_out_of_sight_are_no_news() {
        let (mut s, _) = with_box(json!([40, 40, 200, 62]), &format!("One line\n{}", "\n".repeat(30)));
        let notes = publication_notes(&mut s);
        assert!(notes.iter().all(|n| !n.contains("fits") && !n.contains("taller")), "{notes:?}");
    }

    #[test]
    fn repeated_notes_read_as_one() {
        let g = gather(vec!["a".into(), "b".into(), "a".into()]);
        assert_eq!(g, ["a (2 times)", "b"]);
    }
}

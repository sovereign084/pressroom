//! Tool hosting: the active tool receives pointer/key events and its actions are executed here.

use std::ops::Range;
use std::sync::Arc;

use designcraft_doc::{CellAddr, Document, Selection, StoryId};
use designcraft_geom::Unit;
pub use designcraft_tools::SnapView;
use designcraft_tools::{Action, CanvasLayout, Cursor, Mods, Overlay, PointerEvent, PointerKind, ToolContext, ToolKey};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{EngineError, HistoryEntry, Interaction, Result, Session, record_undo};

/// How far inside a text box's edge (screen pixels) a press still takes the box rather than its
/// text: Publisher's border, where the pointer becomes the move arrow. With the few pixels the
/// hit reaches outside the edge, the border is about ten pixels wide.
const BOX_EDGE_PX: f64 = 5.0;

/// Whether `p` is well inside a text box (not on or near its edge, where a press moves or resizes
/// it). A text box inside a group counts as in Publisher: the first click selects the group, and
/// once the group (or the box) is selected, or while typing in the box, a press in its text types.
fn text_interior_at(cx: &ToolContext, p: designcraft_geom::Point) -> bool {
    let Some((_, id)) = cx.hit_text_frame(p) else { return false };
    if let Some(top) = cx.doc.top_level_of(id).filter(|top| *top != id) {
        let story = cx.doc.item(id).and_then(|it| it.text_frame()).map(|t| t.story);
        let typing_in = cx.selection.text.is_some_and(|t| Some(t.story) == story);
        if !(typing_in || cx.selection.contains(top) || cx.selection.contains(id)) {
            return false;
        }
    }
    let (Some(it), Some(xf)) = (cx.doc.item(id), cx.item_canvas_xf(id)) else { return false };
    // Type on a path has no inside: its letters are the text.
    if !it.path.is_closed() {
        return true;
    }
    let to_box = (xf * it.xf).inverse();
    let path = it.path.to_bezpath();
    let d = cx.tol(BOX_EDGE_PX);
    let s = d * std::f64::consts::FRAC_1_SQRT_2;
    [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d), (s, s), (s, -s), (-s, s), (-s, -s)].into_iter().all(|(dx, dy)| {
        designcraft_geom::hit::fill_contains(&path, designcraft_geom::FillRule::NonZero, to_box * designcraft_geom::Point::new(p.x + dx, p.y + dy))
    })
}

/// The selection as the Selection tool takes it over from the Type tool: the text's frame,
/// selected as an object.
fn object_selection(doc: &Document, sel: &Selection) -> Selection {
    let mut sel = sel.clone();
    if let Some(t) = sel.text.take()
        && let Some(f) = t.frame.or_else(|| doc.story(t.story).and_then(|s| s.frames.first().copied()))
    {
        sel.items = vec![f];
        sel.cells = None;
    }
    sel
}

/// Something the UI should do in response to a tool or command.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UiRequest {
    Dialog {
        id: String,
        params: Value,
    },
    View {
        params: Value,
    },
    /// Open a file picker for the given purpose (`place`, `open`, `saveAs`, `export`).
    Pick {
        purpose: String,
        params: Value,
    },
}

/// View information the UI passes with pointer events.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ViewInfo {
    /// Screen pixels per point.
    pub zoom: f64,
    pub snap: SnapView,
    pub unit: Unit,
    /// The toolbox's Stroke box is in front (X): colour tools apply to strokes.
    pub stroke_in_front: bool,
    /// PressRoom's window: the selection handles act as Publisher's (see `ToolContext::publisher`).
    pub publisher: bool,
}

impl ViewInfo {
    pub const fn at_zoom(zoom: f64) -> Self {
        Self { zoom, snap: SnapView::FACTORY, unit: Unit::Points, stroke_in_front: false, publisher: false }
    }
}

impl Default for ViewInfo {
    fn default() -> Self {
        Self::at_zoom(1.0)
    }
}

impl Session {
    pub fn tool_id(&self) -> &'static str {
        self.tool.id()
    }

    pub fn set_tool(&mut self, id: &str) {
        if self.tool.id() == id {
            return;
        }
        // A composition ends with the tool that shows it: its marked text stays as typed.
        if let Err(e) = self.end_composition() {
            log::warn!("ending the IME composition: {e}");
        }
        self.tool = designcraft_tools::create(id);
        self.temp_tool = None;
        if matches!(self.tool.id(), "selection" | "directSelection") {
            self.last_selection_tool = self.tool.id();
        }
        // Any other tool unloads the place cursor.
        if id != "placeGun" {
            self.loaded = None;
        }
        // Leaving the Type tool keeps the text frame selected.
        if id != "type"
            && let Some(st) = self.active_mut()
        {
            st.selection = object_selection(&st.doc, &st.selection);
        }
    }

    pub fn layout(&self) -> CanvasLayout {
        match self.active() {
            Some(st) => CanvasLayout::new(&st.doc, st.editing_parents),
            None => CanvasLayout::default(),
        }
    }

    fn with_ctx<T>(&mut self, view: ViewInfo, f: impl FnOnce(&mut Self, &ToolContext) -> T) -> Option<T> {
        self.with_ctx_as(view, false, f)
    }

    /// `with_ctx`; with `object`, the selection is as the Selection tool takes it over from the
    /// Type tool (the text's frame).
    fn with_ctx_as<T>(&mut self, view: ViewInfo, object: bool, f: impl FnOnce(&mut Self, &ToolContext) -> T) -> Option<T> {
        let st = self.active()?;
        let doc = st.doc.clone();
        let sel = if object { object_selection(&doc, &st.selection) } else { st.selection.clone() };
        let layer = st.active_layer;
        let layout = CanvasLayout::new(&doc, st.editing_parents);
        let cache = self.cache.clone();
        let cx = ToolContext {
            doc: &doc,
            selection: &sel,
            cache: &cache,
            layout: &layout,
            zoom: view.zoom.max(1e-6),
            layer,
            snap: view.snap,
            unit: view.unit,
            stroke_in_front: view.stroke_in_front,
            publisher: view.publisher,
        };
        Some(f(self, &cx))
    }

    /// The tool pointer, key, overlay and cursor events go to: the selection tool a Cmd-held press
    /// borrowed, for the rest of that gesture, or else the chosen tool.
    fn gesture_tool(&mut self) -> &mut dyn designcraft_tools::Tool {
        match self.temp_tool.as_deref_mut() {
            Some(t) => t,
            None => self.tool.as_mut(),
        }
    }

    /// Which selection tool a Cmd-held press borrows while another tool is chosen: whichever of
    /// Selection and Direct Selection was chosen last. None for the selection tools themselves,
    /// Hand, Zoom and a loaded place cursor, where Cmd means something else or nothing.
    fn borrowed_selection_tool(&self) -> Option<&'static str> {
        match self.tool.id() {
            "selection" | "directSelection" | "hand" | "zoom" | "placeGun" => None,
            _ => Some(self.last_selection_tool),
        }
    }

    pub fn pointer(&mut self, ev: &PointerEvent, view: ViewInfo) -> Result<()> {
        // A press ends a composition, keeping its marked text as typed (like clicking away from
        // one in a native text view); the UI tells the system IME to drop it.
        if matches!(ev.kind, PointerKind::Down | PointerKind::DoubleClick) {
            self.end_composition()?;
        }
        // Cmd held at a press: the gesture (press, drags, release) goes to the selection tool the
        // chosen tool borrows, which selects, moves or resizes without a tool change (#919).
        if matches!(ev.kind, PointerKind::Down | PointerKind::DoubleClick)
            && ev.mods.cmd
            && self.temp_tool.is_none()
            && !self.tool.busy()
            && let Some(id) = self.borrowed_selection_tool()
        {
            self.temp_tool = Some(designcraft_tools::create(id));
        }
        self.publisher_pointer(ev, view);
        let actions = self.with_ctx(view, |s, cx| s.gesture_tool().pointer(cx, ev)).unwrap_or_default();
        let r = self.run_actions(actions);
        if ev.kind == PointerKind::Up && self.temp_tool.as_ref().is_some_and(|t| !t.busy()) {
            self.temp_tool = None;
        }
        r
    }

    /// PressRoom's single pointer, as in Publisher: a press inside a text box puts the text cursor
    /// there (the Type tool takes the press), and a press anywhere else while typing goes back to
    /// selecting (the Selection tool takes it). A tool chosen on purpose (Draw Text Box, a shape)
    /// keeps its first press, which draws.
    fn publisher_pointer(&mut self, ev: &PointerEvent, view: ViewInfo) {
        if ev.kind != PointerKind::Down || ev.click_count() != 1 || ev.mods.cmd || ev.mods.shift || self.temp_tool.is_some() || self.tool.busy() {
            return;
        }
        if std::mem::take(&mut self.tool_armed) {
            return;
        }
        match self.tool.id() {
            "selection" if self.with_ctx(view, |_, cx| text_interior_at(cx, ev.pos)).unwrap_or(false) => self.set_tool("type"),
            // Typing, a press on a text box's border takes the box (to move it, or to delete it),
            // as on Publisher's border; so does a press on its handles or off it.
            "type" if !self.with_ctx(view, |_, cx| text_interior_at(cx, ev.pos)).unwrap_or(false) => self.set_tool("selection"),
            _ => {}
        }
    }

    /// The pointer Publisher shows where a press would act (see `publisher_pointer`): the text
    /// cursor inside a text box, the four-way move arrow on a text box's border and on other
    /// objects, and the handles' arrows on the handles, also while typing.
    fn publisher_cursor(&mut self, tool_cursor: Cursor, p: designcraft_geom::Point, mods: Mods, view: ViewInfo) -> Cursor {
        if mods.cmd || self.temp_tool.is_some() || self.tool.busy() || self.tool_armed {
            return tool_cursor;
        }
        let object = |cx: &ToolContext, c: Cursor| match c {
            Cursor::Arrow if cx.hit(p).is_some() => Cursor::Move,
            c => c,
        };
        match self.tool.id() {
            "selection" if tool_cursor == Cursor::Arrow => {
                self.with_ctx(view, |_, cx| if text_interior_at(cx, p) { Cursor::Text } else { object(cx, Cursor::Arrow) }).unwrap_or(tool_cursor)
            }
            "type" if !self.with_ctx(view, |_, cx| text_interior_at(cx, p)).unwrap_or(true) => {
                // What the Selection tool shows once the press has made the text's box the selection.
                let select = designcraft_tools::create("selection");
                self.with_ctx_as(view, true, |_, cx| object(cx, select.cursor(cx, p, mods))).unwrap_or(tool_cursor)
            }
            _ => tool_cursor,
        }
    }

    /// Dispatch a temporary view tool without replacing the active tool or its state.
    /// Its actions use the same execution path as the selected tool's gestures.
    pub fn pointer_with_tool(&mut self, tool: &mut dyn designcraft_tools::Tool, ev: &PointerEvent, view: ViewInfo) -> Result<()> {
        let actions = self.with_ctx(view, |_, cx| tool.pointer(cx, ev)).unwrap_or_default();
        self.run_actions(actions)
    }

    pub fn tool_key(&mut self, key: ToolKey, mods: Mods, view: ViewInfo) -> Result<bool> {
        // Keys belong to the IME while it composes: nothing else (Escape's deselect) takes them.
        if self.tool.composing() {
            return Ok(true);
        }
        let actions = self.with_ctx(view, |s, cx| s.gesture_tool().key(cx, key, mods)).unwrap_or_default();
        let handled = !actions.is_empty();
        self.run_actions(actions)?;
        Ok(handled)
    }

    pub fn overlays(&mut self, view: ViewInfo) -> Vec<Overlay> {
        self.with_ctx(view, |s, cx| s.gesture_tool().overlays(cx)).unwrap_or_default()
    }

    pub fn cursor(&mut self, p: designcraft_geom::Point, mods: Mods, view: ViewInfo) -> Cursor {
        // Cmd held over the canvas shows the selection tool a press would borrow.
        if mods.cmd
            && self.temp_tool.is_none()
            && !self.tool.busy()
            && let Some(id) = self.borrowed_selection_tool()
        {
            let borrowed = designcraft_tools::create(id);
            return self.with_ctx(view, |_, cx| borrowed.cursor(cx, p, mods)).unwrap_or_default();
        }
        let c = self.with_ctx(view, |s, cx| s.gesture_tool().cursor(cx, p, mods)).unwrap_or_default();
        if view.publisher { self.publisher_cursor(c, p, mods, view) } else { c }
    }

    /// Canvas centres of the content grabbers the Selection tool shows: the selected frame's and
    /// the one under `hover`.
    pub fn content_grabbers(&mut self, hover: Option<designcraft_geom::Point>, view: ViewInfo) -> Vec<designcraft_geom::Point> {
        if self.tool.id() != "selection" {
            return vec![];
        }
        self.with_ctx(view, |_, cx| designcraft_tools::select::grabbers(cx, hover).into_iter().map(|(_, c)| c).collect()).unwrap_or_default()
    }

    pub fn tool_busy(&self) -> bool {
        self.tool.busy()
    }

    pub fn wants_text(&mut self) -> bool {
        self.with_ctx(ViewInfo::at_zoom(1.0), |s, cx| s.gesture_tool().wants_text(cx)).unwrap_or(false)
    }

    /// IME marked text at the text caret (see `Tool::ime_preedit`; `active_chars` counts
    /// characters of `text`). It shows in place as a preview: no undo step, nothing journaled.
    pub fn tool_preedit(&mut self, text: &str, active_chars: Option<std::ops::Range<usize>>, view: ViewInfo) -> Result<()> {
        let actions = self.with_ctx(view, |s, cx| s.tool.ime_preedit(cx, text, active_chars)).unwrap_or_default();
        let r = self.run_actions(actions);
        if r.is_err() && self.tool.composing() {
            // The marked text can't show (its story went away): the composition ends with nothing
            // typed, so no gesture stays open.
            match self.with_ctx(view, |s, cx| s.tool.ime_preedit(cx, "", None)) {
                Some(cancel) => self.run_actions(cancel)?,
                None => self.tool = designcraft_tools::create(self.tool.id()),
            }
        }
        r
    }

    /// The IME committed `text`: it replaces the marked text as one `text.insert` (one undo step,
    /// one journaled command).
    pub fn tool_ime_commit(&mut self, text: &str, view: ViewInfo) -> Result<()> {
        let actions = self.with_ctx(view, |s, cx| s.tool.ime_commit(cx, text)).unwrap_or_default();
        self.run_actions(actions)
    }

    /// Is the active tool showing uncommitted IME text?
    pub fn tool_composing(&self) -> bool {
        self.tool.composing()
    }

    /// End an IME composition from outside the IME (a click, a tool switch, another command): the
    /// marked text stays as typed, as one undo step.
    pub fn end_composition(&mut self) -> Result<()> {
        if !self.tool.composing() {
            return Ok(());
        }
        match self.with_ctx(ViewInfo::default(), |s, cx| s.tool.ime_end(cx)) {
            Some(actions) => self.run_actions(actions),
            None => {
                // No document to type into: only the tool's state goes.
                self.tool = designcraft_tools::create(self.tool.id());
                Ok(())
            }
        }
    }

    /// The caret line (canvas space) the IME candidate window follows.
    pub fn tool_ime_caret(&mut self, view: ViewInfo) -> Option<(designcraft_geom::Point, designcraft_geom::Point)> {
        self.with_ctx(view, |s, cx| s.tool.ime_caret(cx)).flatten()
    }

    pub fn run_actions(&mut self, actions: Vec<Action>) -> Result<()> {
        for a in actions {
            match a {
                Action::Begin(label) => self.begin_interaction(&label)?,
                Action::Preview(cmd, p) => self.preview(&cmd, p)?,
                Action::Commit => self.commit_interaction()?,
                Action::Cancel => self.cancel_interaction(),
                Action::Exec(cmd, p) => {
                    self.execute(&cmd, &p)?;
                }
                Action::Dialog(id, params) => self.ui_requests.push(UiRequest::Dialog { id, params }),
                Action::SwitchTool(id) => self.set_tool(&id),
                Action::View(params) => self.ui_requests.push(UiRequest::View { params }),
                Action::Handled => {}
            }
        }
        Ok(())
    }

    pub fn begin_interaction(&mut self, label: &str) -> Result<()> {
        let st = self.doc_mut()?;
        st.interaction =
            Some(Interaction { label: label.into(), doc: st.doc.clone(), selection: st.selection.clone(), preview: None, revision: st.revision });
        Ok(())
    }

    /// Apply `cmd` on top of the interaction's snapshot (replacing the previous preview).
    pub fn preview(&mut self, cmd: &str, params: Value) -> Result<()> {
        let st = self.doc_mut()?;
        let Some(it) = st.interaction.clone() else { return Err(EngineError::Other("no interaction".into())) };
        st.doc = it.doc.clone();
        st.selection = it.selection.clone();
        // Not through `execute`: a preview is part of the gesture (an IME composition included),
        // not a command that ends it.
        let r = self.guarded(cmd, |s| s.execute_unguarded(cmd, &params));
        if let Some(st) = self.active_mut()
            && let Some(i) = st.interaction.as_mut()
        {
            i.preview = Some((cmd.to_string(), params));
        }
        // Journal only the committed result.
        if r.is_ok() {
            self.journal.pop();
        }
        r.map(|_| ())
    }

    pub fn commit_interaction(&mut self) -> Result<()> {
        let st = self.doc_mut()?;
        let Some(it) = st.interaction.take() else { return Ok(()) };
        if !Arc::ptr_eq(&it.doc, &st.doc) {
            record_undo(st, HistoryEntry { label: it.label, doc: it.doc, selection: it.selection });
        }
        if let Some((c, p)) = it.preview {
            self.journal.push((c, p));
        }
        // The interaction's last preview is the transform to repeat; the next one starts afresh.
        self.transforms.2 = false;
        Ok(())
    }

    pub fn cancel_interaction(&mut self) {
        if let Some(st) = self.active_mut()
            && let Some(it) = st.interaction.take()
        {
            // Nothing changed: the document goes back to the state and revision it began with.
            st.doc = it.doc;
            st.selection = it.selection;
            st.revision = it.revision;
            if self.transforms.2 {
                self.transforms.0.pop();
                self.transforms.2 = false;
            }
        }
    }

    /// Cancel the interaction of the document with `uid`, active or not: a gesture that began in
    /// one document and lost it to another tab ends there, leaving the document as it began.
    pub fn cancel_interaction_of(&mut self, uid: u64) {
        if self.active().is_some_and(|st| st.uid == uid) {
            self.cancel_interaction();
        } else if let Some(st) = self.docs.iter_mut().find(|st| st.uid == uid)
            && let Some(it) = st.interaction.take()
        {
            st.doc = it.doc;
            st.selection = it.selection;
            st.revision = it.revision;
        }
    }
}

/// The committed text around the Type tool's IME caret.
///
/// While an IME composition is active the marked text is only an interaction preview, so this
/// borrows the interaction's committed snapshot and selection instead of the previewed document.
/// `anchor` and `focus` are UTF-8 byte offsets into `text`; `focus` is the primary caret end.
#[derive(Clone, Copy, Debug)]
pub struct ImeTextContext<'a> {
    /// Stable document identity (a new document gets a new value).
    pub document_uid: u64,
    /// Document-state identity: changes when the state is recreated even if `document_uid` is
    /// preserved (File › Revert).
    pub document_epoch: u64,
    /// Story being edited, or the story that owns the edited table cell.
    pub story: StoryId,
    /// Edited table cell, when the caret is in one.
    pub cell: Option<CellAddr>,
    /// Committed logical story text, without the current IME pre-edit.
    pub text: &'a str,
    /// Selection anchor (the end that stays put under Shift-arrow selection).
    pub anchor: usize,
    /// Primary selection end (the caret).
    pub focus: usize,
}

/// What an accepted IME mutation applied to the committed story text.
///
/// The engine reports this from the whole story, not from the bounded outgoing IME window, so a
/// successful mutation can be described exactly even when the deleted text lies outside that
/// window. `deleted_before` and `deleted_after` are byte ranges of the text before the mutation;
/// `inserted` is the text the mutation typed at the selection. Invalid or unreportable mutations
/// return `None` rather than a guessed description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeMutation {
    /// Stable document identity of the mutation.
    pub document_uid: u64,
    /// Document-state identity (see [`ImeTextContext::document_epoch`]).
    pub document_epoch: u64,
    /// Story (or the story owning the edited cell) that changed.
    pub story: StoryId,
    /// Edited table cell, when the caret was in one.
    pub cell: Option<CellAddr>,
    /// Committed selection before and after the mutation (UTF-8 byte offsets).
    pub anchor_before: usize,
    pub focus_before: usize,
    pub anchor_after: usize,
    pub focus_after: usize,
    /// Committed story text length before and after the mutation.
    pub text_len_before: usize,
    pub text_len_after: usize,
    /// Text removed before and after the selection (empty when nothing was removed).
    pub deleted_before: Range<usize>,
    pub deleted_after: Range<usize>,
    /// Text typed at the selection, after the commit normalization.
    pub inserted: String,
}

/// The identity, committed selection and text length at the IME caret.
#[derive(Clone, Copy, Debug)]
struct ImeState {
    document_uid: u64,
    document_epoch: u64,
    story: StoryId,
    cell: Option<CellAddr>,
    anchor: usize,
    focus: usize,
    text_len: usize,
}

/// The ranges a successful `text.deleteSurrounding` reports it removed.
fn ime_ranges_from_command(r: &Value) -> Option<(Range<usize>, Range<usize>)> {
    let range = |key: &str| {
        let arr = r.get(key)?.as_array()?;
        let start = usize::try_from(arr.first()?.as_u64()?).ok()?;
        let end = usize::try_from(arr.get(1)?.as_u64()?).ok()?;
        (start <= end).then_some(start..end)
    };
    Some((range("deletedBefore")?, range("deletedAfter")?))
}

/// The byte ranges an IME delete-surrounding request removes around a selection.
///
/// The returned pair is `(before_selection, after_selection)`. Counts exclude the selection
/// itself and the current pre-edit. Character counts are Unicode scalar values. Byte counts
/// round inward: a count that ends inside a character leaves that whole character. Invalid or
/// non-boundary selection offsets return `None` rather than guessed ranges.
pub fn ime_delete_ranges(text: &str, anchor: usize, focus: usize, before: usize, after: usize, bytes: bool) -> Option<(Range<usize>, Range<usize>)> {
    let (min, max) = ime_selection_bounds(text, anchor, focus)?;
    Some(if bytes { ime_byte_delete_ranges(text, min, max, before, after) } else { ime_character_delete_ranges(text, min, max, before, after) })
}

/// Validate the committed selection and return its lower and upper UTF-8 byte offsets.
fn ime_selection_bounds(text: &str, anchor: usize, focus: usize) -> Option<(usize, usize)> {
    if anchor > text.len() || focus > text.len() || !text.is_char_boundary(anchor) || !text.is_char_boundary(focus) {
        return None;
    }
    Some((anchor.min(focus), anchor.max(focus)))
}

/// Byte-counted ranges around the selection, rounded inward to character boundaries.
fn ime_byte_delete_ranges(text: &str, min: usize, max: usize, before: usize, after: usize) -> (Range<usize>, Range<usize>) {
    // Start at the requested byte offset and step at most three bytes to the next character
    // boundary: the scan is bounded by the count, not by the caret's offset in the story.
    let from = min.saturating_sub(before).min(min);
    let mut before_start = from;
    while before_start < min && !text.is_char_boundary(before_start) {
        before_start += 1;
    }
    let target = max.saturating_add(after).min(text.len());
    let mut after_end = max;
    for (i, c) in text[max..].char_indices() {
        let end = max + i + c.len_utf8();
        if end <= target {
            after_end = end;
        } else {
            break;
        }
    }
    (before_start..min, max..after_end)
}

/// Character-counted ranges around the selection, using Unicode scalar values.
fn ime_character_delete_ranges(text: &str, min: usize, max: usize, before: usize, after: usize) -> (Range<usize>, Range<usize>) {
    let before_start = if before == 0 {
        min
    } else {
        let mut seen = 0usize;
        let mut start = 0usize;
        for (i, _) in text[..min].char_indices().rev() {
            seen += 1;
            start = i;
            if seen == before {
                break;
            }
        }
        if seen < before { 0 } else { start }
    };
    let mut after_end = max;
    if after > 0 {
        let mut seen = 0usize;
        for (i, c) in text[max..].char_indices() {
            seen += 1;
            after_end = max + i + c.len_utf8();
            if seen == after {
                break;
            }
        }
    }
    (before_start..min, max..after_end)
}

impl Session {
    /// The committed text and selection at the Type tool's IME caret.
    ///
    /// Returns `None` without an insertion point, without a committed story, or while a
    /// composition has no interaction snapshot to describe its committed text. The query never
    /// copies the story.
    pub fn ime_text_context(&self) -> Option<ImeTextContext<'_>> {
        let st = self.active()?;
        let (doc, ts) = if self.tool.composing() {
            let it = st.interaction.as_ref()?;
            (it.doc.as_ref(), it.selection.text?)
        } else {
            (st.doc.as_ref(), st.selection.text?)
        };
        let story = doc.text_story(ts.story, ts.cell)?;
        if ts.anchor > story.text.len()
            || ts.focus > story.text.len()
            || !story.text.is_char_boundary(ts.anchor)
            || !story.text.is_char_boundary(ts.focus)
        {
            // A stale or corrupt insertion point: report unavailable rather than inventing offsets.
            return None;
        }
        Some(ImeTextContext {
            document_uid: st.uid,
            document_epoch: st.identity_epoch,
            story: ts.story,
            cell: ts.cell,
            text: &story.text,
            anchor: ts.anchor,
            focus: ts.focus,
        })
    }

    /// Delete the text an IME asked to remove around the selection, as one undo step.
    ///
    /// The mutation goes through the `text.deleteSurrounding` command, so it restores a Data Merge
    /// preview, records undo, auto-sizes, journals and is panic-guarded like any other edit.
    /// The selection itself and any current pre-edit are excluded. Character counts are scalar
    /// values; byte counts round inward to whole characters. A selection keeps its direction and
    /// is remapped past the deleted text.
    pub fn ime_delete_surrounding(&mut self, before: usize, after: usize, bytes: bool) -> Result<Option<ImeMutation>> {
        if !self.ime_deletion_changes_text(before, after, bytes)? {
            return Ok(None);
        }
        let Some(pre) = self.ime_state() else { return Ok(None) };
        self.end_preedit_without_typing()?;
        let r = self.execute("text.deleteSurrounding", &json!({"before": before, "after": after, "bytes": bytes}))?;
        Ok(self.ime_mutation_from_command(pre, &r, String::new()))
    }

    /// The IME committed `text`: one `text.insert`, reported as the mutation it applied.
    pub fn ime_commit(&mut self, text: &str, view: ViewInfo) -> Result<Option<ImeMutation>> {
        let Some(pre) = self.ime_state() else { return self.tool_ime_commit(text, view).map(|()| None) };
        self.tool_ime_commit(text, view)?;
        let inserted = designcraft_tools::ime_commit_text(text);
        // A commit replaces the selection: the removed ranges are empty at its two ends.
        let (start, end) = (pre.anchor.min(pre.focus), pre.anchor.max(pre.focus));
        Ok(self.ime_mutation_from_states(pre, start..start, end..end, inserted))
    }

    /// Delete surrounding text and insert the IME commit in one undo step.
    ///
    /// This is the normal delete-surrounding-then-commit batch: both mutations are one
    /// `text.deleteSurrounding` command, so one Undo restores the state before the batch.
    pub fn ime_commit_after_delete(&mut self, text: &str, view: ViewInfo, before: usize, after: usize, bytes: bool) -> Result<Option<ImeMutation>> {
        if !self.ime_deletion_changes_text(before, after, bytes)? {
            return self.ime_commit(text, view);
        }
        let Some(pre) = self.ime_state() else { return Ok(None) };
        self.end_preedit_without_typing()?;
        let inserted = designcraft_tools::ime_commit_text(text);
        let r = self.execute("text.deleteSurrounding", &json!({"before": before, "after": after, "bytes": bytes, "commit": inserted.clone()}))?;
        Ok(self.ime_mutation_from_command(pre, &r, inserted))
    }

    /// The identity, committed selection and text length of the IME caret's story.
    fn ime_state(&self) -> Option<ImeState> {
        let context = self.ime_text_context()?;
        Some(ImeState {
            document_uid: context.document_uid,
            document_epoch: context.document_epoch,
            story: context.story,
            cell: context.cell,
            anchor: context.anchor,
            focus: context.focus,
            text_len: context.text.len(),
        })
    }

    /// Report the mutation the delete command applied, from its ranges and the states around it.
    fn ime_mutation_from_command(&self, before: ImeState, r: &Value, inserted: String) -> Option<ImeMutation> {
        let (deleted_before, deleted_after) = ime_ranges_from_command(r)?;
        self.ime_mutation_from_states(before, deleted_before, deleted_after, inserted)
    }

    /// Return a mutation only when the document uid/epoch, story and cell still match; otherwise report no mutation.
    fn ime_mutation_from_states(
        &self,
        before: ImeState,
        deleted_before: Range<usize>,
        deleted_after: Range<usize>,
        inserted: String,
    ) -> Option<ImeMutation> {
        let after = self.ime_state()?;
        // The mutation must have stayed in the story it began in.
        if after.document_uid != before.document_uid
            || after.document_epoch != before.document_epoch
            || after.story != before.story
            || after.cell != before.cell
        {
            return None;
        }
        Some(ImeMutation {
            document_uid: before.document_uid,
            document_epoch: before.document_epoch,
            story: before.story,
            cell: before.cell,
            anchor_before: before.anchor,
            focus_before: before.focus,
            anchor_after: after.anchor,
            focus_after: after.focus,
            text_len_before: before.text_len,
            text_len_after: after.text_len,
            deleted_before,
            deleted_after,
            inserted,
        })
    }

    /// Clear an active pre-edit without inserting text; if no interaction snapshot exists, reset the tool.
    fn end_preedit_without_typing(&mut self) -> Result<()> {
        if !self.tool.composing() {
            return Ok(());
        }
        match self.with_ctx(ViewInfo::default(), |s, cx| s.tool.ime_preedit(cx, "", None)) {
            Some(actions) => self.run_actions(actions),
            None => {
                self.tool = designcraft_tools::create(self.tool.id());
                Ok(())
            }
        }
    }

    /// Report false for no IME context or no-op ranges, and error when the selection is not on character boundaries.
    fn ime_deletion_changes_text(&self, before: usize, after: usize, bytes: bool) -> Result<bool> {
        let Some(context) = self.ime_text_context() else { return Ok(false) };
        let Some((before_range, after_range)) = ime_delete_ranges(context.text, context.anchor, context.focus, before, after, bytes) else {
            return Err(EngineError::Other("the IME text selection is not on character boundaries".into()));
        };
        Ok(!before_range.is_empty() || !after_range.is_empty())
    }
}

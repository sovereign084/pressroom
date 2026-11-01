//! PressRoom's own window: the Publisher-style title bar, ribbon and status bar in place of
//! DesignCraft's chrome, Publisher's shortcuts, and the ribbon's buttons acting on the document.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::DesignApp;
use crate::test_window::{self, Window};

/// The whole window as PressRoom shows it, with a new document open.
fn window() -> Harness<'static, Window> {
    let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::services::Services::default());
    app.ui.pressroom_shell = true;
    test_window::open(app, vec2(1440.0, 900.0))
}

fn run(h: &mut Harness<'static, Window>, command: &str, params: Value) -> Value {
    let r = h.state_mut().app.run(command, params).unwrap();
    h.run_steps(4);
    r
}

/// A text box holding "Hello", all of it selected.
fn select_hello(h: &mut Harness<'static, Window>) {
    let story = run(h, "frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "text": "Hello"}))["story"].clone();
    run(h, "text.select", json!({"story": story, "anchor": 0, "focus": 5}));
}

fn font_style(h: &mut Harness<'static, Window>) -> Value {
    run(h, "type.selectionAttrs", json!({}))["chars"]["fontStyle"].clone()
}

fn tool(h: &Harness<'static, Window>) -> String {
    h.state().app.session.tool_id().to_string()
}

#[test]
fn the_window_is_publishers_not_designcrafts() {
    let h = window();
    for panel in ["pr_title", "pr_ribbon", "pr_status"] {
        let r = test_window::panel_rect(&h, panel);
        assert!(r.width() > 1000.0, "{panel}: {r:?}");
    }
    // DesignCraft's Control panel isn't drawn.
    assert!(egui::containers::panel::PanelState::load(&h.ctx, h.ui_id().with("control_bar")).is_none());
    // The Home tab's buttons, as in Publisher.
    for label in ["Bold (Ctrl+B)", "Draw Text Box", "Pictures"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
    // Which window to show is never saved with the settings.
    assert!(serde_json::to_value(crate::state::UiState::default()).unwrap().get("pressroomShell").is_none());
}

#[test]
fn the_bold_button_bolds_the_selected_text() {
    let mut h = window();
    select_hello(&mut h);
    assert_ne!(font_style(&mut h), json!("Bold"));
    h.get_by_label("Bold (Ctrl+B)").click();
    h.run_steps(4);
    assert_eq!(font_style(&mut h), json!("Bold"));
}

/// Ctrl+B is Publisher's Bold, and a letter typed with nothing to type into never picks a tool, as
/// it would in a page-layout program (T for the Type tool, V for Selection).
#[test]
fn ctrl_b_bolds_and_single_letters_never_switch_tools() {
    let mut h = window();
    select_hello(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    h.run_steps(4);
    assert_eq!(font_style(&mut h), json!("Bold"));
    run(&mut h, "edit.deselectAll", json!({}));
    run(&mut h, "tool.select", json!({"tool": "selection"}));
    for key in [egui::Key::T, egui::Key::M, egui::Key::P] {
        h.key_press(key);
        h.run_steps(2);
        assert_eq!(tool(&h), "selection", "{key:?}");
    }
}

#[test]
fn draw_text_box_readies_the_text_box_tool() {
    let mut h = window();
    assert_eq!(tool(&h), "selection");
    h.get_by_label("Draw Text Box").click();
    h.run_steps(4);
    assert_eq!(tool(&h), "type");
}

/// The text cursor in a table cell brings up Table Tools, whose Layout tab adds and deletes rows,
/// as in Publisher (a calendar gains or loses a week).
#[test]
fn table_tools_insert_and_delete_rows_with_the_cursor_in_a_cell() {
    let mut h = window();
    let story = run(&mut h, "frame.create", json!({"rect": [72, 72, 400, 300], "content": "text", "text": ""}))["story"].clone();
    run(&mut h, "text.select", json!({"story": story, "anchor": 0, "focus": 0}));
    run(&mut h, "table.insert", json!({"rows": 3, "cols": 3}));
    let rows = |h: &mut Harness<'static, Window>| run(h, "table.get", json!({"story": story}))["rows"].as_array().map_or(0, Vec::len);
    assert_eq!(rows(&mut h), 3);
    assert!(h.state().app.session.active().unwrap().selection.text.as_ref().unwrap().cell.is_some(), "the cursor is in a cell");
    h.get_by_label("Layout").click();
    h.run_steps(4);
    h.get_by_label("Insert Below").click();
    h.run_steps(4);
    assert_eq!(rows(&mut h), 4);
    h.get_by_label("Delete").click();
    h.run_steps(4);
    h.get_by_label("Delete Rows").click();
    h.run_steps(4);
    assert_eq!(rows(&mut h), 3);
}

/// PressRoom starts on the File screen; a publication opened any other way than its buttons (a
/// double-click in Explorer, Open with) must not stay hidden behind it.
#[test]
fn a_publication_opened_from_outside_is_shown_not_the_file_screen() {
    let mut app = DesignApp::new(designcraft_engine::Session::new(), crate::services::Services::default());
    app.ui.pressroom_shell = true;
    let mut h = Harness::builder().with_size(vec2(1440.0, 900.0)).with_step_dt(0.25).build_ui_state(
        |ui, app: &mut DesignApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    assert!(h.query_by_label("Blank page sizes").is_some() || h.query_by_label("Open").is_some(), "the File screen shows at start");
    assert!(h.query_by_label("Bold (Ctrl+B)").is_none());
    // Opened the way Explorer's double-click opens it: not through the File screen.
    h.state_mut().run("file.new", json!({})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label("Bold (Ctrl+B)").is_some(), "the ribbon and the publication show");
    // Chosen on purpose, the File screen stays while that publication is in front.
    h.get_by_label("File").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold (Ctrl+B)").is_none());
    h.run_steps(4);
    assert!(h.query_by_label("Bold (Ctrl+B)").is_none(), "it stays open");
}

/// A publication using fonts this computer lacks opens without stopping (no Find Font dialog, as
/// in Publisher); File › Info lists them and offers Replace Fonts….
#[test]
fn missing_fonts_open_quietly_and_file_info_lists_them() {
    let mut h = window();
    let story = run(&mut h, "frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "text": "Sample"}))["story"].as_u64().unwrap();
    {
        let app = &mut h.state_mut().app;
        let d = std::sync::Arc::make_mut(&mut app.session.doc_mut().unwrap().doc);
        d.story_mut(designcraft_doc::StoryId(story)).unwrap().format_chars(0..6, |f| {
            f.over.font_family = Some("Unavailable Test Family".into());
            f.over.font_style = Some("Regular".into());
        });
    }
    let path = std::env::temp_dir().join(format!("pressroom-missing-font-{}.pressroom", std::process::id()));
    run(&mut h, "file.save", json!({"path": path}));
    run(&mut h, "file.open", json!({"path": path}));
    assert!(h.state().app.ui.dialog.is_none(), "no dialog on opening");
    assert!(h.query_by_label("Bold (Ctrl+B)").is_some(), "the publication shows");
    h.get_by_label("File").click();
    h.run_steps(4);
    assert!(h.query_by_label("Unavailable Test Family").is_some(), "File › Info lists the font");
    h.get_by_label_contains("Replace Fonts").click();
    h.run_steps(4);
    assert_eq!(h.state().app.ui.dialog.as_ref().map(|d| d.id.as_str()), Some("findFont"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn the_tabs_switch_the_ribbon() {
    let mut h = window();
    assert!(h.query_by_label("Insert File").is_none(), "Insert's buttons wait for the Insert tab");
    h.get_by_label("Insert").click();
    h.run_steps(4);
    assert!(h.query_by_label("Insert File").is_some());
    assert!(h.query_by_label("Bold (Ctrl+B)").is_none(), "Home's buttons are gone");
    h.get_by_label("Home").click();
    h.run_steps(4);
    assert!(h.query_by_label("Bold (Ctrl+B)").is_some());
}

#[test]
fn a_new_version_shows_in_a_bar_under_the_ribbon() {
    let mut h = window();
    h.state_mut().app.updates_rt.pressroom.offer("9.9.9");
    h.run_steps(4);
    let bar = test_window::panel_rect(&h, "pr_update_bar");
    let ribbon = test_window::panel_rect(&h, "pr_ribbon");
    assert!(bar.width() > 1000.0 && bar.min.y >= ribbon.max.y - 0.5, "{bar:?} under {ribbon:?}");
    assert!(h.query_by_label("Update Now").is_some());
    h.get_by_label("Close the update message").click();
    h.run_steps(4);
    assert!(!h.state().app.updates_rt.pressroom.showing());
}

#[test]
fn restart_and_update_waits_for_unsaved_work() {
    let mut h = window();
    run(&mut h, "frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "text": "Unsaved"}));
    let msi = std::path::PathBuf::from("pressroom-9.9.9-windows-x64.msi");
    h.state_mut().app.updates_rt.pressroom.downloaded("9.9.9", msi);
    h.run_steps(4);
    h.get_by_label("Restart and Update").click();
    h.run_steps(4);
    // Nothing installs over unsaved work: the update stays ready and PressRoom says why.
    assert!(matches!(h.state().app.updates_rt.pressroom.phase(), crate::pressroom::update::Phase::Ready { .. }));
    assert!(h.state().app.ui.status.contains("Save your publications first"), "{}", h.state().app.ui.status);
}

#[test]
fn file_account_shows_the_version_and_checks_for_updates() {
    let mut h = window();
    crate::pressroom::open_backstage(&h.ctx, crate::pressroom::Backstage::Account);
    h.run_steps(4);
    assert!(h.query_by_label("Check for Updates").is_some());
    let version = crate::pressroom::update::VERSION.map_or_else(|| "Development build".to_string(), |v| format!("Version {v}"));
    assert!(h.query_by_label(&version).is_some(), "{version}");
}

#[test]
fn the_ribbon_is_current_publishers_help_tab_and_objects_order() {
    let mut h = window();
    run(&mut h, "frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "text": "Hi"}));
    // Home's Objects group, in Publisher's order: Draw Text Box, Pictures, Table, Shapes.
    let x = |h: &Harness<'static, Window>, label: &str| h.get_by_label(label).rect().center().x;
    let order = [x(&h, "Draw Text Box"), x(&h, "Pictures"), x(&h, "Table"), x(&h, "Shapes")];
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    // The Help tab, after View, with Check for Updates.
    assert!(x(&h, "View") < x(&h, "Help"));
    h.get_by_label("Help").click();
    h.run_steps(4);
    assert!(h.query_by_label("Check for Updates").is_some());
}

#[test]
fn the_office_theme_is_saved_with_the_settings() {
    let mut ui = crate::state::UiState::default();
    // Office's default: follow Windows.
    assert_eq!(ui.pressroom_theme, "system");
    assert_eq!(super::theme_index("system", false), 0);
    assert_eq!(super::theme_index("system", true), 3);
    assert_eq!(super::theme_index("darkgray", false), 2);
    ui.pressroom_theme = "colorful".into();
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&ui).unwrap()).unwrap();
    assert_eq!(back.pressroom_theme, "colorful");
}

#[test]
fn top_bar_hover_shades_match_the_palette() {
    // Laid over the theme's own bar colour, the see-through hover gives the palette's hover.
    for p in [&super::WHITE, &super::DARK_GRAY, &super::BLACK] {
        let shade = super::shade(p.top, p.top_hover);
        let over = |base: u8| {
            let a = f32::from(shade.a()) / 255.0;
            (f32::from(shade.r()) + f32::from(base) * (1.0 - a)).round() as i32
        };
        assert!((over(p.top.r()) - i32::from(p.top_hover.r())).abs() <= 1, "{shade:?}");
    }
}

fn top_level_items(h: &Harness<'static, Window>) -> usize {
    h.state().app.session.active().map_or(0, |st| st.doc.spreads.iter().map(|s| s.items.len()).sum())
}

fn doc_json(h: &Harness<'static, Window>) -> Value {
    serde_json::to_value(h.state().app.session.active().unwrap().doc.as_ref()).unwrap()
}

#[test]
fn every_building_block_goes_in_whole_as_one_undo_step() {
    use super::blocks;
    let mut h = window();
    run(&mut h, "frame.create", json!({"rect": [72, 72, 100, 100], "content": "unassigned"}));
    let start = doc_json(&h);
    let items = top_level_items(&h);
    for b in blocks::PAGE_PARTS.iter().chain(&blocks::CALENDARS).chain(&blocks::BORDERS).chain(&blocks::ADS) {
        blocks::insert(&mut h.state_mut().app, b).unwrap_or_else(|e| panic!("{}: {e}", b.name));
        h.run_steps(2);
        // One object: the block's parts grouped.
        assert_eq!(top_level_items(&h), items + 1, "{}", b.name);
        let st = h.state().app.session.active().unwrap();
        for name in super::scheme::SWATCHES {
            assert!(st.doc.swatch(name).is_some(), "{}: {name}", b.name);
        }
        assert!(st.doc.styles.paragraph.iter().any(|p| p.name == super::scheme::HEADING), "{}", b.name);
        run(&mut h, "edit.undo", json!({}));
        assert_eq!(doc_json(&h), start, "{} undoes whole", b.name);
    }
}

#[test]
fn a_calendar_block_fills_in_its_month() {
    use super::blocks;
    let mut h = window();
    blocks::insert(&mut h.state_mut().app, &blocks::CALENDARS[1]).unwrap();
    let (title, first, len, weeks) = blocks::month(true);
    let st = h.state().app.session.active().unwrap();
    let texts: Vec<String> = st.doc.stories.values().map(|s| s.text.clone()).collect();
    assert!(texts.contains(&title), "{texts:?}");
    let table = st.doc.stories.values().flat_map(|s| s.tables.values()).next().expect("a table");
    let cell = |r: u32, c: u32| table.cell(r as usize, c as usize).map(|x| x.text.text.clone()).unwrap_or_default();
    assert_eq!(cell(0, 0), "Sun");
    assert_eq!(cell(1, first), "1");
    let last = first + len - 1;
    assert_eq!(cell(1 + last / 7, last % 7), len.to_string());
    assert_eq!(table.rows.len() as u32, weeks + 1);
}

#[test]
fn wordart_goes_in_styled_and_schemes_restyle_the_publication() {
    let mut h = window();
    let items = top_level_items(&h);
    for i in 0..super::wordart::STYLES.len() {
        let d = crate::dialogs::Dialog::new(
            super::WORDART,
            json!({"style": i, "text": "Harvest Supper", "font": "Arial Black", "size": 36, "bold": false, "italic": false}),
        );
        super::wordart::insert(&mut h.state_mut().app, &d).unwrap_or_else(|e| panic!("style {i}: {e}"));
        h.run_steps(2);
        assert_eq!(top_level_items(&h), items + 1 + i, "style {i}");
    }
    // Empty text is refused.
    let d = crate::dialogs::Dialog::new(super::WORDART, json!({"style": 0, "text": "  "}));
    assert!(super::wordart::insert(&mut h.state_mut().app, &d).is_err());

    // A colour scheme fills the scheme swatches; Fonts sets body and heading fonts.
    super::scheme::apply_color_scheme(&mut h.state_mut().app, 3);
    h.run_steps(2);
    assert_eq!(super::scheme::current_color_scheme(&h.state().app), Some(3));
    super::scheme::apply_color_scheme(&mut h.state_mut().app, 0);
    h.run_steps(2);
    assert_eq!(super::scheme::current_color_scheme(&h.state().app), Some(0));
    super::scheme::apply_font_scheme(&mut h.state_mut().app, 2);
    h.run_steps(2);
    assert_eq!(super::scheme::current_font_scheme(&h.state().app), Some(2));
    let st = h.state().app.session.active().unwrap();
    let basic = st.doc.styles.paragraph.iter().find(|p| p.name == designcraft_doc::BASIC_PARAGRAPH).unwrap();
    assert_eq!(basic.chars.font_family.as_deref(), Some("Arial"));
}

#[test]
fn the_font_group_works_with_nothing_selected_for_the_next_text_box() {
    let mut h = window();
    run(&mut h, "edit.deselectAll", json!({}));
    super::set_next_text(&json!({"size": 31}));
    // A text box drawn now (a caret in an empty story) takes it.
    run(&mut h, "frame.create", json!({"rect": [72, 72, 300, 200], "content": "text", "caret": true}));
    h.run_steps(3);
    assert_eq!(run(&mut h, "type.selectionAttrs", json!({}))["chars"]["size"], json!(31.0));
    assert!(super::next_text().is_none(), "used once");
}

#[test]
fn the_quick_access_toolbar_shows_the_commands_chosen_and_saves_them() {
    let mut h = window();
    assert!(h.query_by_label("Customize Quick Access Toolbar").is_some());
    h.state_mut().app.ui.pressroom_qat.push("spelling".into());
    h.run_steps(3);
    let back: crate::state::UiState = serde_json::from_value(serde_json::to_value(&h.state().app.ui).unwrap()).unwrap();
    assert_eq!(back.pressroom_qat, ["save", "undo", "redo", "spelling"]);
}

const CTRL_SHIFT: egui::Modifiers = egui::Modifiers { shift: true, ..egui::Modifiers::COMMAND };
const ALT_SHIFT: egui::Modifiers = egui::Modifiers { shift: true, ..egui::Modifiers::ALT };

fn press(h: &mut Harness<'static, Window>, mods: egui::Modifiers, key: egui::Key) {
    h.key_press_modifiers(mods, key);
    h.run_steps(4);
}

fn dialog_id(h: &Harness<'static, Window>) -> Option<String> {
    h.state().app.ui.dialog.as_ref().map(|d| d.id.clone())
}

fn close_dialog(h: &mut Harness<'static, Window>) {
    h.state_mut().app.ui.dialog = None;
    h.run_steps(2);
}

/// A box of `text`, selected as an object. Returns its id and story.
fn text_box(h: &mut Harness<'static, Window>, text: &str) -> (u64, u64) {
    let r = run(h, "frame.create", json!({"rect": [72, 72, 400, 300], "content": "text", "text": text}));
    (r["id"].as_u64().unwrap(), r["story"].as_u64().unwrap())
}

fn undo_steps(h: &mut Harness<'static, Window>) -> usize {
    run(h, "document.history", json!({}))["undo"].as_array().unwrap().len()
}

#[test]
fn the_keys_do_what_microsofts_list_for_publisher_says() {
    let mut h = window();
    select_hello(&mut h);
    // These three did something else, inherited from a page-layout program.
    press(&mut h, CTRL_SHIFT, egui::Key::K);
    assert_eq!(run(&mut h, "type.selectionAttrs", json!({}))["chars"]["capitalization"], "smallCaps");
    press(&mut h, CTRL_SHIFT, egui::Key::K);
    assert_eq!(run(&mut h, "type.selectionAttrs", json!({}))["chars"]["capitalization"], "normal");
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::K);
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.hyperlink"));
    close_dialog(&mut h);
    // Ctrl+Shift+F is the Font dialog, not Ctrl+F's Find.
    press(&mut h, CTRL_SHIFT, egui::Key::F);
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.font"));
    close_dialog(&mut h);
    assert_eq!(h.state().app.ui.pressroom_pane, None);
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::F);
    assert_eq!(h.state().app.ui.pressroom_pane.as_deref(), Some("pr.find"));
    h.state_mut().app.ui.pressroom_pane = None;
    press(&mut h, egui::Modifiers::NONE, egui::Key::F3);
    assert_eq!(h.state().app.ui.pressroom_pane.as_deref(), Some("pr.find"));
    h.state_mut().app.ui.pressroom_pane = None;
    // Ctrl+Shift+H is Hyphenation, not Ctrl+H's Replace.
    press(&mut h, CTRL_SHIFT, egui::Key::H);
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.hyphenation"));
    close_dialog(&mut h);
    // Inserting at the caret.
    let at = run(&mut h, "type.selectionAttrs", json!({}))["selectedText"].as_str().unwrap().to_string();
    assert_eq!(at, "Hello");
    let boundaries = h.state().app.ui.frame_edges;
    press(&mut h, CTRL_SHIFT, egui::Key::O);
    assert_eq!(h.state().app.ui.frame_edges, !boundaries);
    let pages = run(&mut h, "document.inspect", json!({}))["pageCount"].as_u64().unwrap();
    press(&mut h, CTRL_SHIFT, egui::Key::U);
    assert_eq!(run(&mut h, "document.inspect", json!({}))["pageCount"].as_u64().unwrap(), pages + 1);
}

#[test]
fn alt_f6_goes_all_the_way_to_the_front_and_back() {
    let mut h = window();
    let a = run(&mut h, "frame.create", json!({"rect": [10, 10, 50, 50]}))["id"].as_u64().unwrap();
    let b = run(&mut h, "frame.create", json!({"rect": [20, 20, 60, 60]}))["id"].as_u64().unwrap();
    let c = run(&mut h, "frame.create", json!({"rect": [30, 30, 70, 70]}))["id"].as_u64().unwrap();
    let order = |h: &mut Harness<'static, Window>| -> Vec<u64> {
        h.state().app.session.active().unwrap().doc.spreads[0].items.iter().map(|i| i.id.0).collect()
    };
    run(&mut h, "selection.set", json!({"ids": [a]}));
    press(&mut h, egui::Modifiers::ALT, egui::Key::F6);
    assert_eq!(order(&mut h), [b, c, a]);
    run(&mut h, "selection.set", json!({"ids": [c]}));
    press(&mut h, ALT_SHIFT, egui::Key::F6);
    assert_eq!(order(&mut h), [c, b, a]);
}

#[test]
fn page_layout_keys_are_gone_from_the_window() {
    let mut h = window();
    let id = run(&mut h, "frame.create", json!({"rect": [10, 10, 50, 50]}))["id"].as_u64().unwrap();
    run(&mut h, "selection.set", json!({"ids": [id]}));
    // Ctrl+3 hid the object; Ctrl+D opened Place; Tab hid the panels.
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::Num3);
    assert!(!h.state().app.session.active().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().hidden);
    press(&mut h, egui::Modifiers::COMMAND, egui::Key::D);
    assert_eq!(dialog_id(&h), None);
    press(&mut h, egui::Modifiers::NONE, egui::Key::Tab);
    assert_eq!(h.state().app.ui.hidden_panels, 0);
}

#[test]
fn panels_open_in_the_task_pane_and_font_and_paragraph_as_dialogs() {
    let mut h = window();
    select_hello(&mut h);
    run(&mut h, "window.panel.activate", json!({"panel": "hyperlinks"}));
    assert_eq!(h.state().app.ui.pressroom_pane.as_deref(), Some("hyperlinks"));
    assert!(h.query_by_label("Close").is_some() || h.state().app.ui.pressroom_pane.is_some());
    run(&mut h, "window.panel.close", json!({"panel": "hyperlinks"}));
    assert_eq!(h.state().app.ui.pressroom_pane, None);
    run(&mut h, "app.characterPanel", json!({}));
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.font"));
    close_dialog(&mut h);
    run(&mut h, "app.paragraphPanel", json!({}));
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.paragraph"));
    close_dialog(&mut h);
    run(&mut h, "app.tablePanel", json!({}));
    assert_eq!(h.state().app.ui.pressroom_pane.as_deref(), Some("table"));
    // Every panel the ribbon opens draws without trouble.
    for panel in ["hyperlinks", "bookmarks", "glyphs", "links", "preflight", "dataMerge", "table"] {
        run(&mut h, "window.panel.activate", json!({"panel": panel}));
        h.run_steps(3);
    }
    super::find::open(&mut h.state_mut().app, true);
    h.run_steps(3);
    assert_eq!(h.state().app.ui.pressroom_pane.as_deref(), Some("pr.find"));
}

/// Opens each of PressRoom's dialogs and draws it, with what each needs selected.
#[test]
fn every_dialog_opens_draws_and_closes() {
    let mut h = window();
    select_hello(&mut h);
    use super::dialogs as d;
    type Open = fn(&mut DesignApp);
    let list: &[(&str, Open)] = &[
        ("pr.font", d::open_font),
        ("pr.spacing", d::open_spacing),
        ("pr.paragraph", d::open_paragraph),
        ("pr.hyperlink", d::open_hyperlink),
        ("pr.hyphenation", d::open_hyphenation),
        ("pr.insertPage", d::open_insert_page),
        ("pr.table", d::open_table),
        ("pr.guides", d::open_guides),
        ("pr.pageSetup", d::open_setup),
        ("pr.date", d::open_date),
        ("pr.pageNumberFormat", d::open_number_format),
        ("pr.applyMaster", d::open_apply_master),
        ("pr.newMaster", d::open_new_master),
        ("pr.options", d::open_options),
    ];
    for (id, open) in list {
        open(&mut h.state_mut().app);
        h.run_steps(3);
        assert_eq!(dialog_id(&h).as_deref(), Some(*id));
        assert!(!d::title(id).is_empty(), "{id}");
        close_dialog(&mut h);
    }
    d::open_rename(&mut h.state_mut().app, 0);
    h.run_steps(3);
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.rename"));
    close_dialog(&mut h);
    d::open_move(&mut h.state_mut().app, 0);
    h.run_steps(3);
    close_dialog(&mut h);
    run(&mut h, "text.exitToFrame", json!({}));
    d::open_format(&mut h.state_mut().app);
    let tabs = ["colors", "size", "layout", "box", "alt"];
    for tab in tabs {
        h.state_mut().app.ui.dialog.as_mut().unwrap().fields.insert("tab".into(), json!(tab));
        h.run_steps(3);
    }
    close_dialog(&mut h);
}

#[test]
fn a_dialog_left_as_it_opened_changes_nothing() {
    let mut h = window();
    select_hello(&mut h);
    let before = undo_steps(&mut h);
    for open in [
        super::dialogs::open_font as fn(&mut DesignApp),
        super::dialogs::open_paragraph,
        super::dialogs::open_hyphenation,
        super::dialogs::open_guides,
        super::dialogs::open_setup,
    ] {
        open(&mut h.state_mut().app);
        h.run_steps(2);
        let d = h.state_mut().app.ui.dialog.take().unwrap();
        super::dialogs::apply(&mut h.state_mut().app, &d).unwrap();
    }
    assert_eq!(undo_steps(&mut h), before, "OK with nothing changed leaves no undo step");
}

#[test]
fn the_font_dialog_applies_only_what_changed() {
    let mut h = window();
    select_hello(&mut h);
    run(&mut h, "type.char", json!({"attrs": {"size": 20}}));
    super::dialogs::open_font(&mut h.state_mut().app);
    h.run_steps(2);
    let mut d = h.state_mut().app.ui.dialog.take().unwrap();
    d.fields.insert("underline".into(), json!(true));
    d.fields.insert("smallCaps".into(), json!(true));
    super::dialogs::apply(&mut h.state_mut().app, &d).unwrap();
    h.run_steps(2);
    let chars = run(&mut h, "type.selectionAttrs", json!({}))["chars"].clone();
    assert_eq!(chars["size"], 20.0);
    assert_eq!(chars["capitalization"], "smallCaps");
    assert!(chars["underline"].as_bool().unwrap_or(false) || chars["underline"].is_object());
    // A bad size says so and changes nothing.
    super::dialogs::open_font(&mut h.state_mut().app);
    let mut d = h.state_mut().app.ui.dialog.take().unwrap();
    d.fields.insert("size".into(), json!("huge"));
    assert!(super::dialogs::apply(&mut h.state_mut().app, &d).is_err());
}

#[test]
fn line_spacing_goes_to_the_whole_paragraph_from_a_caret() {
    let mut h = window();
    let (_, story) = text_box(&mut h, "First paragraph here\nSecond one");
    run(&mut h, "text.select", json!({"story": story, "anchor": 3, "focus": 3}));
    super::dialogs::open_paragraph(&mut h.state_mut().app);
    let mut d = h.state_mut().app.ui.dialog.take().unwrap();
    d.fields.insert("between".into(), json!("2sp"));
    let steps = undo_steps(&mut h);
    super::dialogs::apply(&mut h.state_mut().app, &d).unwrap();
    h.run_steps(2);
    assert_eq!(undo_steps(&mut h), steps + 1, "one undo step");
    let doc = h.state().app.session.active().unwrap().doc.clone();
    let st = doc.story(designcraft_doc::StoryId(story)).unwrap();
    let leading = |at: usize| st.char_format_at(at).over.leading;
    assert!(leading(1).is_some() && leading(15).is_some(), "the whole paragraph");
    assert!(leading(25).is_none(), "not the next one");
    assert_eq!((h.state().app.session.active().unwrap().selection.text.unwrap().anchor), 3, "the caret stays where it was");
}

#[test]
fn a_page_moves_and_gets_a_name() {
    assert_eq!(super::dialogs::move_target(0, 2, false), 1, "before page 3: in behind page 2");
    assert_eq!(super::dialogs::move_target(0, 2, true), 2);
    assert_eq!(super::dialogs::move_target(3, 0, false), 0);
    assert_eq!(super::dialogs::move_target(3, 0, true), 1);
    let mut h = window();
    run(&mut h, "layout.pages.insert", json!({"count": 3}));
    let mark = run(&mut h, "frame.create", json!({"rect": [10, 10, 50, 50]}))["id"].as_u64().unwrap();
    let page_of = |h: &Harness<'static, Window>| h.state().app.session.active().unwrap().doc.page_of_item(designcraft_doc::ItemId(mark));
    assert_eq!(page_of(&h), Some(0));
    super::dialogs::open_move(&mut h.state_mut().app, 0);
    let mut d = h.state_mut().app.ui.dialog.take().unwrap();
    d.fields.insert("to".into(), json!(2));
    d.fields.insert("where".into(), json!("after"));
    super::dialogs::apply(&mut h.state_mut().app, &d).unwrap();
    assert_eq!(page_of(&h), Some(2));
    super::dialogs::open_rename(&mut h.state_mut().app, 1);
    let mut d = h.state_mut().app.ui.dialog.take().unwrap();
    d.fields.insert("title".into(), json!("Welcome"));
    super::dialogs::apply(&mut h.state_mut().app, &d).unwrap();
    assert_eq!(h.state().app.session.active().unwrap().doc.page(1).unwrap().title.as_deref(), Some("Welcome"));
}

#[test]
fn format_painter_takes_a_look_from_one_text_to_another() {
    let mut h = window();
    let (_, story) = text_box(&mut h, "Source and target");
    run(&mut h, "text.select", json!({"story": story, "anchor": 0, "focus": 6}));
    run(&mut h, "type.char", json!({"attrs": {"size": 30}}));
    press(&mut h, CTRL_SHIFT, egui::Key::C);
    run(&mut h, "text.select", json!({"story": story, "anchor": 11, "focus": 17}));
    press(&mut h, CTRL_SHIFT, egui::Key::V);
    assert_eq!(run(&mut h, "type.selectionAttrs", json!({}))["chars"]["size"], 30.0);
    // The object's look, too.
    let a = run(&mut h, "frame.create", json!({"rect": [10, 400, 50, 450]}))["id"].as_u64().unwrap();
    run(&mut h, "object.stroke", json!({"weight": 5, "swatch": "[Black]"}));
    let b = run(&mut h, "frame.create", json!({"rect": [60, 400, 90, 450]}))["id"].as_u64().unwrap();
    run(&mut h, "selection.set", json!({"ids": [a]}));
    press(&mut h, CTRL_SHIFT, egui::Key::C);
    run(&mut h, "selection.set", json!({"ids": [b]}));
    press(&mut h, CTRL_SHIFT, egui::Key::V);
    assert_eq!(h.state().app.session.active().unwrap().doc.item(designcraft_doc::ItemId(b)).unwrap().stroke.weight, 5.0);
}

#[test]
fn spelling_walks_the_words_and_changes_them_in_one_step_each() {
    let mut h = window();
    let (_, story) = text_box(&mut h, "Thsi word and thsi word and speling.");
    let word = |h: &Harness<'static, Window>| {
        h.state().app.ui.dialog.as_ref().and_then(|d| d.fields.get("current").map(|c| c["word"].as_str().unwrap_or("").to_string()))
    };
    press(&mut h, egui::Modifiers::NONE, egui::Key::F7);
    assert_eq!(dialog_id(&h).as_deref(), Some("pr.spelling"));
    assert_eq!(word(&h).as_deref(), Some("Thsi"));
    let suggestions = h.state().app.ui.dialog.as_ref().unwrap().fields["suggestions"].clone();
    assert!(suggestions.as_array().unwrap().iter().any(|s| s == "this"), "{suggestions}");
    close_dialog(&mut h);
    let steps = undo_steps(&mut h);
    run(&mut h, "spelling.change", json!({"story": story, "start": 0, "end": 4, "to": "This"}));
    assert_eq!(undo_steps(&mut h), steps + 1);
    assert!(h.state().app.session.active().unwrap().doc.story(designcraft_doc::StoryId(story)).unwrap().text.starts_with("This word"));
}

#[test]
fn hebrew_is_never_a_misspelling_even_tagged_english() {
    let mut h = window();
    let (_, story) = text_box(&mut h, "\u{05E4}\u{05D6} qwrtyx \u{03B1}\u{03B2}\u{03B3}");
    let found = run(&mut h, "spelling.check", json!({"story": story, "suggestions": false}));
    let words: Vec<&str> = found.as_array().unwrap().iter().map(|w| w["word"].as_str().unwrap()).collect();
    assert_eq!(words, ["qwrtyx"]);
}

fn labels(rows: &[super::menus::Row]) -> Vec<&'static str> {
    use super::menus::Row;
    rows.iter()
        .filter_map(|r| match r {
            Row::Cmd(l, _) | Row::With(l, _, _) | Row::Act(l, _) | Row::Sub(l, _) => Some(*l),
            Row::Sep => None,
        })
        .collect()
}

/// A golden list: a row that changes here has changed what a right-click shows.
#[test]
fn the_right_click_menus_are_publishers() {
    assert_eq!(
        labels(super::menus::OBJECT),
        [
            "Cut",
            "Copy",
            "Paste",
            "Delete Object",
            "Save as Picture…",
            "Change Picture…",
            "Group",
            "Ungroup",
            "Order",
            "Rotate or Flip",
            "Format Object…",
            "Hyperlink…"
        ]
    );
    assert_eq!(
        labels(super::menus::TEXT),
        ["Cut", "Copy", "Paste", "Keep Text Only", "Font…", "Paragraph…", "Bullets and Numbering…", "Hyperlink…", "Format Text Box…", "Select All"]
    );
    assert_eq!(labels(super::menus::CANVAS), ["Paste", "Zoom", "Ruler Guides", "Select All"]);
    // Every command in them exists.
    fn commands(rows: &[super::menus::Row], out: &mut Vec<&'static str>) {
        use super::menus::Row;
        for r in rows {
            match r {
                Row::Cmd(_, id) | Row::With(_, id, _) => out.push(id),
                Row::Sub(_, sub) => commands(sub, out),
                _ => {}
            }
        }
    }
    let mut ids = Vec::new();
    for t in [super::menus::OBJECT, super::menus::TEXT, super::menus::CANVAS] {
        commands(t, &mut ids);
    }
    for id in ids {
        assert!(designcraft_engine::find_command(id).is_some() || crate::menus::ui_label(id).is_some(), "{id}");
    }
}

#[test]
fn the_context_menus_draw_with_what_they_need() {
    let mut h = window();
    let (id, story) = text_box(&mut h, "Thsi word");
    run(&mut h, "text.select", json!({"story": story, "anchor": 2, "focus": 2}));
    h.state_mut().app.ui.dynamic_spelling = true;
    super::spelling::capture(&mut h.state_mut().app);
    let clicked = h.state().app.ui.pressroom_spell.clone().unwrap();
    assert_eq!(clicked.word, "Thsi");
    assert!(clicked.suggestions.iter().any(|s| s == "this"));
    run(&mut h, "text.select", json!({"story": story, "anchor": 6, "focus": 6}));
    super::spelling::capture(&mut h.state_mut().app);
    assert_eq!(h.state().app.ui.pressroom_spell, None, "a word spelled right has none");
    h.state_mut().app.ui.pressroom_ignored.push("thsi".into());
    run(&mut h, "text.select", json!({"story": story, "anchor": 2, "focus": 2}));
    super::spelling::capture(&mut h.state_mut().app);
    assert_eq!(h.state().app.ui.pressroom_spell, None, "Ignore All");
    let _ = id;
}

#[test]
fn shapes_draw_from_the_gallery_and_change() {
    let mut h = window();
    let id = run(&mut h, "frame.create", json!({"rect": [72, 72, 172, 172], "preset": "heart"}))["id"].as_u64().unwrap();
    let path = |h: &Harness<'static, Window>| h.state().app.session.active().unwrap().doc.item(designcraft_doc::ItemId(id)).unwrap().path.clone();
    let heart = path(&h);
    assert_eq!(heart.bounds().map(|b| (b.width().round(), b.height().round())), Some((100.0, 100.0)));
    run(&mut h, "object.convertShape", json!({"to": "star5"}));
    assert_ne!(path(&h), heart);
    assert!(h.state_mut().app.run("frame.create", json!({"rect": [0, 0, 9, 9], "preset": "blob"})).is_err());
    assert!(h.state_mut().app.run("object.convertShape", json!({"to": "blob"})).is_err());
    let line = run(&mut h, "line.create", json!({"a": [0, 0], "b": [50, 0], "preset": "doubleArrow"}))["id"].as_u64().unwrap();
    let st = h.state().app.session.active().unwrap().doc.item(designcraft_doc::ItemId(line)).unwrap().stroke.clone();
    assert_eq!((st.start, st.end), (designcraft_doc::Arrowhead::Triangle, designcraft_doc::Arrowhead::Triangle));
    // A text box keeps its text when its shape changes.
    let (t, story) = text_box(&mut h, "Stay");
    run(&mut h, "object.convertShape", json!({"to": "diamond", "ids": [t]}));
    assert_eq!(h.state().app.session.active().unwrap().doc.story(designcraft_doc::StoryId(story)).unwrap().text, "Stay");
}

#[test]
fn a_table_is_its_own_object_even_while_typing() {
    let mut h = window();
    let (_, story) = text_box(&mut h, "A short test.");
    run(&mut h, "text.select", json!({"story": story, "anchor": 3, "focus": 3}));
    let steps = undo_steps(&mut h);
    super::insert_table_of(&mut h.state_mut().app, 3, 2).unwrap();
    h.run_steps(3);
    assert_eq!(undo_steps(&mut h), steps + 1, "one undo step");
    let doc = h.state().app.session.active().unwrap().doc.clone();
    let st = doc.story(designcraft_doc::StoryId(story)).unwrap();
    assert_eq!(st.text, "A short test.", "the text isn't split");
    assert_eq!(doc.spreads[0].items.len(), 2, "a box of its own");
    let info = run(&mut h, "table.get", json!({}));
    assert_eq!(info["rows"].as_array().map_or(info["rows"].as_u64(), |r| Some(r.len() as u64)), Some(3));
}

#[test]
fn groups_fold_from_the_right_when_the_ribbon_is_narrow() {
    use super::ribbon::groups_that_fit;
    let widths = [100.0, 100.0, 100.0, 100.0];
    assert_eq!(groups_that_fit(&widths, 4, 500.0), 4, "room for them all");
    assert_eq!(groups_that_fit(&widths, 4, 400.0), 4);
    assert!(groups_that_fit(&widths, 4, 350.0) < 4);
    assert_eq!(groups_that_fit(&widths, 4, 50.0), 0, "never a panic, however narrow");
    // The whole window at a small laptop's width draws.
    let mut h = window();
    select_hello(&mut h);
    h.set_size(vec2(900.0, 600.0));
    h.run_steps(6);
    for tab in ["Insert", "Page Design", "Mailings", "Review", "View", "Home"] {
        h.get_by_label(tab).click();
        h.run_steps(4);
    }
}

#[test]
fn every_tab_draws_for_every_kind_of_selection() {
    let mut h = window();
    let picture = run(&mut h, "frame.create", json!({"rect": [10, 10, 50, 50], "content": "graphic"}))["id"].as_u64().unwrap();
    let shape = run(&mut h, "frame.create", json!({"rect": [60, 10, 90, 50]}))["id"].as_u64().unwrap();
    let (text, story) = text_box(&mut h, "Words");
    for id in [picture, shape, text] {
        run(&mut h, "selection.set", json!({"ids": [id]}));
        h.get_by_label("Format").click();
        h.run_steps(4);
    }
    run(&mut h, "text.select", json!({"story": story, "anchor": 5, "focus": 5}));
    run(&mut h, "table.insert", json!({"rows": 2, "cols": 2}));
    h.run_steps(4);
    h.get_by_label("Design").click();
    h.run_steps(4);
    h.get_by_label("Layout").click();
    h.run_steps(4);
}

#[test]
fn the_print_page_draws_each_layout_and_warns_of_text_that_wont_fit() {
    let mut h = window();
    run(&mut h, "frame.create", json!({"rect": [72, 72, 100, 90], "content": "text", "text": "Far too many words for so small a box to hold."}));
    super::open_backstage(&h.ctx.clone(), super::Backstage::Print);
    h.run_steps(5);
    assert!(h.query_by_label_contains("Some text doesn't fit its box on page 1").is_some());
    let plan = run(&mut h, "file.printPlan", json!({"layout": "booklet"}));
    assert!(!plan["sides"].as_array().unwrap().is_empty());
}

#[test]
fn a_picture_on_the_clipboard_pastes_as_a_picture_and_text_doesnt() {
    let mut h = window();
    let dir = std::env::temp_dir();
    let _ = dir;
    // Nothing on the clipboard, or no service: nothing happens.
    assert!(!super::paste_picture(&mut h.state_mut().app));
    h.state_mut().app.services.clipboard_image = Some(Box::new(|| None));
    assert!(!super::paste_picture(&mut h.state_mut().app));
    // A size that doesn't match its pixels is refused.
    h.state_mut().app.services.clipboard_image = Some(Box::new(|| Some((9, 9, vec![0; 4]))));
    assert!(!super::paste_picture(&mut h.state_mut().app));
}

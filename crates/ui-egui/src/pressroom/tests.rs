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

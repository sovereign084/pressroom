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

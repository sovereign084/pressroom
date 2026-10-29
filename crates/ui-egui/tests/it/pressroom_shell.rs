//! The app as shipped shows PressRoom's Publisher-style window. (Built for its own unit tests the
//! crate shows DesignCraft's, so they keep checking it; this test builds the crate as the app does.)

use designcraft_ui_egui::UiState;

#[test]
fn the_shipped_app_shows_pressrooms_window() {
    assert!(UiState::default().pressroom_shell);
    // Settings saved before (or by DesignCraft) don't change it: it is never saved.
    let saved: UiState = serde_json::from_str(r#"{"pressroomShell": false}"#).unwrap();
    assert!(saved.pressroom_shell);
    assert!(serde_json::to_value(UiState::default()).unwrap().get("pressroomShell").is_none());
}

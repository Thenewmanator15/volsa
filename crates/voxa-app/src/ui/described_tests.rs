//! The interface as a screen reader is told of it.

use super::toolbar;
use crate::state::AppState;

#[test]
fn the_toolbar_names_its_buttons_for_a_screen_reader() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let state = AppState::default();
    let mut actions = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        toolbar::show(ui, &state, true, &mut actions);
    });
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    let described = output.platform_output.accesskit_update;
    let update = described.expect("the interface is described");
    let named = |wanted: &str| {
        let is_it = |(_, node): &(_, egui::accesskit::Node)| node.label() == Some(wanted);
        update.nodes.iter().any(is_it)
    };
    for button in ["Series", "Folder…", "File…", "Tags", "Size 100%", "About"] {
        assert!(named(button), "nothing is named {button}");
    }
}

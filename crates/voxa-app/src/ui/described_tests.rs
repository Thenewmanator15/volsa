//! The interface as a screen reader is told of it.

use super::search::Search;
use super::theme::ThemeChoice;
use super::top;
use crate::state::AppState;

#[test]
fn the_top_bar_names_its_buttons_for_a_screen_reader() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let state = AppState::default();
    let mut actions = Vec::new();
    let mut search = Search::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        top::show(ui, &state, &mut search, ThemeChoice::System, &mut actions);
    });
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    let described = output.platform_output.accesskit_update;
    let update = described.expect("the interface is described");
    let named = |wanted: &str| {
        let is_it = |(_, node): &(_, egui::accesskit::Node)| node.label() == Some(wanted);
        update.nodes.iter().any(is_it)
    };
    // A button is named by its action's name in plain words, whatever its label says. The
    // theme button is named for what it does: the choice after the one it shows.
    let ids = ["help.keys", "theme.light"];
    for id in ids {
        let command = super::commands::find(id).expect("listed");
        assert!(named(command.name), "nothing is named {}", command.name);
        assert!(
            !named(command.label),
            "{} is named by its label",
            command.id
        );
    }
    // The search box writes "Find a tool or part" on itself, so that is what its name begins
    // with, for a person who speaks the words they see.
    let search = super::commands::find("search.open").expect("listed");
    let spoken = format!("{}, {}", top::SEARCH_SAYS, search.name);
    assert_eq!(top::SEARCH_SAYS, "Find a tool or part");
    assert!(named(&spoken), "the search box is not named {spoken}");
    // The menus are named by their buttons.
    assert!(named("Open"), "the Open menu");
    assert!(named(top::MENU), "the menu at the right");
}

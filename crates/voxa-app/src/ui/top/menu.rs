//! The menu at the right of the top bar: the size of the interface, full screen, the keys and
//! mouse, where the log is, and About.

use egui::containers::menu::MenuButton;
use egui::{Atom, Button, Id, Ui, Vec2, vec2};

use super::{ICON_BUTTON, Route};
use crate::ui::commands::{self, Request, menus};
use crate::ui::size;
use crate::ui::symbols::{self, Symbol};

/// The actions the menu lists, in groups. The scans are folded from their own strip and by
/// B, and a file's tags are shown from its scan's card.
const GROUPS: [&[&str]; 2] = [
    &["window.full_screen"],
    &["help.keys", "log.where", "about"],
];

/// Draw the button that opens the menu, and the menu while it is open.
pub(super) fn button(ui: &mut Ui, route: &mut Route<'_>) {
    let room = Id::new("voxa: the dots on the menu button");
    let dots = Button::new(Atom::custom(room, Vec2::splat(symbols::SIZE)))
        .min_size(vec2(ICON_BUTTON, 0.0));
    let (response, _) = MenuButton::from_button(dots).ui(ui, |ui| contents(ui, route));
    let response = commands::named(ui, &menus::MORE, "", response);
    let colour = ui.style().interact(&response).fg_stroke.color;
    let rect = egui::Rect::from_center_size(response.rect.center(), Vec2::splat(symbols::SIZE));
    symbols::paint(ui.painter(), rect, Symbol::More, colour);
}

/// What the menu holds.
fn contents(ui: &mut Ui, route: &mut Route<'_>) {
    if let Some(size) = size::menu(ui, route.state) {
        route.asked.push(Request::Size(size));
        ui.close();
    }
    for group in GROUPS {
        ui.separator();
        for id in group {
            let item = commands::button_made(ui, id, route.state, "", |command| {
                let item = Button::new(command.name);
                match command.key {
                    Some(key) => item.right_text(key.shown),
                    None => item,
                }
            });
            if let Some((_, Some(does))) = item {
                route.take(Some(does));
                ui.close();
            }
        }
    }
}

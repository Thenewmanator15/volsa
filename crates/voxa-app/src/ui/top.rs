//! The bar across the top of the window, on one line: Open, the name and date of the study,
//! the search box, help, the theme and a menu. Its buttons are actions of the list of
//! actions, which gives their words and hints. What the person asks of the window, they ask
//! as a [`Request`] to be done by the window; a change of state goes onto the actions.
//!
//! In a narrow window the study's name gives way first, then the search box gets shorter; the
//! buttons keep their places, so that Open, search and the menu are always in reach.

use std::path::PathBuf;

use egui::{Align, Atom, AtomExt, Button, Id, Layout, RichText, Ui, Vec2, vec2};
use voxa_core::StudyInfo;

use super::browser::study_title;
use super::commands::{self, Does, Request, menus};
use super::search::Search;
use super::symbols::{self, Symbol};
use super::theme::{self, ThemeChoice};
use crate::action::Action;
use crate::state::AppState;

mod menu;
#[cfg(test)]
mod tests;

/// What a screen reader calls the button of the menu at the right.
#[cfg(test)]
pub(super) const MENU: &str = menus::MORE.name;

/// The kinds of thing a scan can be opened from. The system's own dialogs choose either a
/// folder or a file, never both, so there is a choice of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A folder of DICOM files.
    Folder,
    /// A zip file, a DICOMDIR or a single DICOM file.
    File,
}

/// How wide each of the buttons that hold only a symbol or a mark is, in points.
const ICON_BUTTON: f32 = 36.0;
/// How wide the search box is, at most.
const SEARCH_MOST: f32 = 300.0;
/// How wide the search box is, at least, before the buttons would no longer fit.
const SEARCH_LEAST: f32 = 96.0;
/// How much room the study's name is given before the search box is allowed to take the rest.
const TITLE_WANTED: f32 = 160.0;
/// The least room the study's name is worth showing in; with less, it is left out.
const TITLE_LEAST: f32 = 48.0;
/// What the search box says before anything is typed.
pub(super) const SEARCH_SAYS: &str = "Find a tool or part";
/// The key that opens the search, written in the box.
const SEARCH_KEY: &str = "Ctrl K";
/// Left over on a line, in points, so that rounding never pushes the last button off it.
const SPARE: f32 = 1.0;

/// Draw the bar: gives what the person asked of the window, in the order of the bar's
/// buttons, and puts a change of state onto `actions`. `theme` is the theme the window is
/// drawn in, which the theme button shows and moves on from. The search box tells `search`
/// where it is, so that the results hang under it.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    search: &mut Search,
    theme: ThemeChoice,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    ui.horizontal(|ui| {
        theme::closer_in_a_short_window(ui);
        let mut route = Route {
            state,
            actions,
            asked: &mut asked,
        };
        open_menu(ui, &mut route);
        let (title, find) = shares(ui);
        study(ui, state, title);
        find_box(ui, search, find, &mut route);
        icon_buttons(ui, theme, &mut route);
    });
    asked
}

/// Where what a button does goes: a change of state onto the actions, anything else onto what
/// is asked of the window.
struct Route<'a> {
    state: &'a AppState,
    actions: &'a mut Vec<Action>,
    asked: &'a mut Vec<Request>,
}

impl Route<'_> {
    /// Take what a button did, if it was clicked.
    fn take(&mut self, does: Option<Does>) {
        if let Some(does) = does {
            self.asked.extend(does.run(self.state, self.actions));
        }
    }
}

/// How the room left on the line, after Open, is shared: the width of the place for the
/// study's name, and the width of the search box.
fn shares(ui: &Ui) -> (f32, f32) {
    let gap = ui.spacing().item_spacing.x;
    // The three buttons at the right, and the gap after the name, the box and each button.
    let fixed = 3.0 * ICON_BUTTON + 4.0 * gap + SPARE;
    let room = (ui.available_width() - fixed).max(0.0);
    let find = (room - TITLE_WANTED)
        .clamp(SEARCH_LEAST, SEARCH_MOST)
        .min(room);
    (room - find, find)
}

/// The Open menu: a folder, or a zip file or a DICOMDIR.
fn open_menu(ui: &mut Ui, route: &mut Route<'_>) {
    let open = &menus::OPEN;
    commands::menu_button(ui, open, open.label, |ui| {
        for id in ["open.folder", "open.file"] {
            if let Some(does) = commands::button(ui, id, route.state, None) {
                route.take(Some(does));
                ui.close();
            }
        }
    });
}

/// The study the bar names: the one the active viewport's scan is in, or else the first.
fn study_shown(state: &AppState) -> Option<&StudyInfo> {
    let index = state.index.as_ref()?;
    let shown = state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.as_ref());
    let holds =
        |study: &&StudyInfo| shown.is_some_and(|id| study.series.iter().any(|s| s.id == *id));
    index
        .studies
        .iter()
        .find(holds)
        .or_else(|| index.studies.first())
}

/// The study's date and description, cut short to fit `width`, whole when the pointer rests
/// on it; a place that wide is kept clear when the name is left out.
fn study(ui: &mut Ui, state: &AppState, width: f32) {
    let height = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        vec2(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(vec2(width, height));
            let named = study_shown(state).filter(|_| width >= TITLE_LEAST);
            if let Some(study) = named {
                let title = study_title(study);
                let words = RichText::new(&title).color(theme::palette(ui.ctx()).quiet_text);
                ui.add(egui::Label::new(words).truncate())
                    .on_hover_text(title);
            }
        },
    );
}

/// The search box, `width` wide: a button that opens the search, which hangs under it.
fn find_box(ui: &mut Ui, search: &mut Search, width: f32, route: &mut Route<'_>) {
    let height = ui.spacing().interact_size.y;
    let magnifier = Id::new("voxa: the magnifier in the search box");
    let fill = ui.visuals().extreme_bg_color;
    let quiet = theme::palette(ui.ctx()).quiet_text;
    let drawn = ui
        .allocate_ui_with_layout(
            vec2(width, height),
            Layout::left_to_right(Align::Center),
            |ui| {
                commands::button_made(ui, "search.open", route.state, SEARCH_SAYS, |_| {
                    let saying = RichText::new(SEARCH_SAYS).color(quiet);
                    let key = RichText::new(SEARCH_KEY).small().color(quiet);
                    let icon = Atom::custom(magnifier, Vec2::splat(symbols::SIZE));
                    Button::new((icon, saying.atom_shrink(true)))
                        .right_text(key)
                        .truncate()
                        .fill(fill)
                        .min_size(vec2(width, 0.0))
                })
            },
        )
        .inner;
    if let Some((shown, does)) = drawn {
        if let Some(rect) = shown.rect(magnifier) {
            let colour = ui.style().interact(&shown.response).fg_stroke.color;
            symbols::paint(ui.painter(), rect, Symbol::Search, colour);
        }
        search.hang_from(shown.response.rect);
        route.take(does);
    }
}

/// The action that draws the window in `choice`.
const fn theme_action(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::System => "theme.system",
        ThemeChoice::Light => "theme.light",
        ThemeChoice::Dark => "theme.dark",
    }
}

/// What the theme button shows for the theme the window is drawn in.
const fn theme_symbol(choice: ThemeChoice) -> Symbol {
    match choice {
        ThemeChoice::System => Symbol::Screen,
        ThemeChoice::Light => Symbol::Sun,
        ThemeChoice::Dark => Symbol::Moon,
    }
}

/// A button `ICON_BUTTON` wide with room for a symbol in it.
fn icon_button<'a>(room: Id) -> Button<'a> {
    Button::new(Atom::custom(room, Vec2::splat(symbols::SIZE))).min_size(vec2(ICON_BUTTON, 0.0))
}

/// Help, the theme button and the menu, at the right of the line.
fn icon_buttons(ui: &mut Ui, theme: ThemeChoice, route: &mut Route<'_>) {
    let shows = commands::find("help.keys").map_or("", |command| command.label);
    let made = commands::button_made(ui, "help.keys", route.state, shows, |command| {
        Button::new(command.label).min_size(vec2(ICON_BUTTON, 0.0))
    });
    route.take(made.and_then(|(_, does)| does));
    // The button shows the theme now, and does what the choice after it does.
    let room = Id::new("voxa: the symbol on the theme button");
    let next = theme_action(theme.next());
    let made = commands::button_made(ui, next, route.state, "", |_| icon_button(room));
    if let Some((shown, does)) = made {
        let colour = ui.style().interact(&shown.response).fg_stroke.color;
        symbols::paint(
            ui.painter(),
            shown.rect(room).unwrap_or(shown.response.rect),
            theme_symbol(theme),
            colour,
        );
        route.take(does);
    }
    menu::button(ui, route);
}

/// What has just been dropped on the window to open, if anything. While something is being
/// dragged over the window, say that it can be dropped.
pub fn dropped(ctx: &egui::Context) -> Option<PathBuf> {
    let (hovering, dropped) = ctx.input(|input| {
        let dropped = input
            .raw
            .dropped_files
            .first()
            .map(|file| file.path().to_owned());
        (!input.raw.hovered_files.is_empty(), dropped)
    });
    if hovering {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop to open"));
        let painter = ctx.layer_painter(layer);
        let screen = ctx.content_rect();
        painter.rect_filled(screen, 0, egui::Color32::from_black_alpha(160));
        let font = egui::FontId::proportional(24.0);
        let middle = egui::Align2::CENTER_CENTER;
        painter.text(
            screen.center(),
            middle,
            "Drop to open",
            font,
            egui::Color32::WHITE,
        );
    }
    dropped
}

/// What the person chose in the system's dialog for this kind of thing to open, if anything.
pub fn picked(pick: Pick) -> Option<PathBuf> {
    match pick {
        Pick::Folder => rfd::FileDialog::new()
            .set_title("Open a folder of DICOM files")
            .pick_folder(),
        Pick::File => rfd::FileDialog::new()
            .set_title("Open a zip file or a DICOMDIR")
            .pick_file(),
    }
}

//! Walking the window for its buttons: the states the window can be in that show buttons the
//! plain arrangements do not (a labelling run, a kept result, labels that Voxa made, a notice,
//! a floating window, a question, each menu open), and the names every button in them may
//! have. A button is an action of the list or a menu, or a row that belongs to one item, named
//! exactly by that item: no name is let through for beginning with a word.

use std::collections::HashSet;

use voxa_labels::{Made, Verdict};
use voxa_segment::Stage;

use super::measured_in_four;
use super::window::{Heard, Told, Window};
use crate::labels::NamesFrom;
use crate::state::{AppState, Notice, NoticeKind};
use crate::ui::commands::{self, Confirm, menus};
use crate::ui::panel::Tab;
use crate::ui::parts::{self, labelling, title};
use crate::ui::tabs::made_up::{able, labelled_by};
use crate::ui::{document, top};

/// What is done to the window before it is read: a floating window opened, a question asked, or
/// menus opened by clicking their buttons, one after another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// Nothing.
    Nothing,
    /// The About window.
    About,
    /// The question asked before this is thrown away.
    Question(Confirm),
    /// The buttons to click, in turn, each named as a screen reader names it.
    Clicks(Vec<String>),
}

/// A state of the window to walk.
pub struct Scene {
    /// What the state is, for a failure to say.
    pub name: &'static str,
    /// The state.
    pub state: AppState,
    /// What is opened in it.
    pub opened: Opened,
}

impl Scene {
    /// A state with nothing opened in it.
    pub fn plain(name: &'static str, state: AppState) -> Self {
        let opened = Opened::Nothing;
        Self {
            name,
            state,
            opened,
        }
    }

    /// The same state with this opened.
    fn with(name: &'static str, state: AppState, opened: Opened) -> Self {
        Self {
            name,
            state,
            opened,
        }
    }
}

/// Who made labels: Voxa, with a model, on a card, on a day.
fn by_voxa() -> Made {
    Made {
        by: "Voxa 9.8.7".to_owned(),
        model: "Made-up model, robust".to_owned(),
        model_sum: String::new(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        notes: vec!["A made-up note.".to_owned()],
    }
}

/// The notices of a made-up window: one with files named in its details, and a newer one.
fn with_notices(mut state: AppState) -> AppState {
    state.notices = vec![
        Notice {
            kind: NoticeKind::Info,
            text: "2 files are not scans or documents Voxa lists".to_owned(),
            details: vec!["notes.txt".to_owned(), "table.json".to_owned()],
            key: Some("skipped"),
        },
        Notice {
            kind: NoticeKind::Warning,
            text: "Made-up: a slice could not be read".to_owned(),
            details: Vec::new(),
            key: None,
        },
    ];
    state
}

/// The window on a computer that can label, with a run on.
fn running() -> AppState {
    let mut state = able();
    state.labelling.running = Some(Stage::Finding { step: 1, of: 2 });
    state.labelling.on = state.scene.series.clone();
    state
}

/// The window on a computer that can label, with a result kept for the series.
fn kept() -> AppState {
    let mut state = able();
    state.labelling.kept = Some("2099-01-02".to_owned());
    state.labelling.kept_files = Some(("labels.nii.gz".into(), "labels.json".into()));
    state
}

/// Labels that Voxa made open on the series, with a result kept and the model's check of them.
fn made_by_voxa() -> AppState {
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("shell".to_owned(), 0.52)],
    };
    let mut state = labelled_by(Some(verdict), NamesFrom::Model, Some(by_voxa()));
    state.labelling.ready = crate::labelling::Ready::Yes;
    state.labelling.cards = crate::ui::tabs::made_up::cards();
    state.labelling.kept = Some("2099-01-02".to_owned());
    state
}

/// The words a screen reader says of a menu's button, which writes its label on itself.
fn menu_button(menu: &menus::Menu) -> String {
    menu.spoken(menu.label)
}

/// Every state of the window the walk reads, besides the arrangements and the report.
pub fn extra_windows() -> Vec<Scene> {
    let notices = || with_notices(measured_in_four());
    let area = notice_area_name(&notices());
    vec![
        Scene::plain("an empty window", AppState::default()),
        Scene::plain("a labelling run", running()),
        Scene::plain("a kept result", kept()),
        Scene::plain("labels that Voxa made", made_by_voxa()),
        Scene::plain("a notice", notices()),
        Scene::with("the notices listed", notices(), Opened::Clicks(vec![area])),
        Scene::with("About", measured_in_four(), Opened::About),
        Scene::with(
            "a question",
            made_by_voxa(),
            Opened::Question(Confirm::CloseLabels),
        ),
        Scene::with(
            "the Open menu",
            measured_in_four(),
            Opened::Clicks(vec![menu_button(&menus::OPEN)]),
        ),
        Scene::with(
            "the More menu",
            measured_in_four(),
            Opened::Clicks(vec![menu_button(&menus::MORE)]),
        ),
        Scene::with(
            "the interface size menu",
            measured_in_four(),
            Opened::Clicks(vec![
                menu_button(&menus::MORE),
                "Interface size 100%".to_owned(),
            ]),
        ),
        Scene::with(
            "the Views menu",
            measured_in_four(),
            Opened::Clicks(vec![menu_button(&menus::VIEWS)]),
        ),
        Scene::with(
            "the Export menu",
            measured_in_four(),
            Opened::Clicks(vec![menu_button(&menus::EXPORT)]),
        ),
        Scene::with(
            "the labelling options menu",
            able(),
            Opened::Clicks(vec![menu_button(&menus::OPTIONS)]),
        ),
    ]
}

/// The window for a scene with this tab open, as a screen reader is told of it.
pub fn told_in(scene: &Scene, tab: Tab) -> (Told, Window) {
    let mut window = Window::new();
    window.panel.tab = tab;
    match &scene.opened {
        Opened::Nothing | Opened::Clicks(_) => {}
        Opened::About => window.about = true,
        Opened::Question(confirm) => window.confirming = Some(*confirm),
    }
    let mut told = window.told(&scene.state);
    if let Opened::Clicks(names) = &scene.opened {
        for name in names {
            told = window.click_named(&scene.state, name).unwrap_or(told);
        }
    }
    (told, window)
}

/// What a screen reader calls the notice area for a state with notices: how many there are, the
/// newest, and the action the button is.
pub fn notice_area_name(state: &AppState) -> String {
    let newest = state
        .notices
        .last()
        .map_or("", |notice| notice.text.as_str());
    let written = match state.notices.len() {
        0 => return String::new(),
        1 => format!("1 notice: {newest}"),
        count => format!("{count} notices, the newest: {newest}"),
    };
    let area = commands::find("notices.show").expect("listed");
    commands::spoken(area, &written).into_owned()
}

/// Every name a button of the window may have in this state, exactly: the actions of the list,
/// as they are named and as they are spoken with their labels; the menus; and each row that
/// belongs to one item, by the item it belongs to.
pub fn known_names(state: &AppState) -> HashSet<String> {
    let mut names = HashSet::new();
    for command in commands::all() {
        names.insert(command.name.to_owned());
        names.insert(commands::spoken(command, command.label).into_owned());
    }
    // The search box writes what it asks of the person on itself.
    let search = commands::find("search.open").expect("listed");
    names.insert(commands::spoken(search, top::SEARCH_SAYS).into_owned());
    for menu in commands::MENUS {
        names.insert(menu.name.to_owned());
        names.insert(menu.spoken(menu.label));
    }
    names.insert(menus::SIZE.spoken("Interface size 100%"));
    names.insert(notice_area_name(state));
    // The button egui puts in the title bar of every window that can be closed.
    names.insert("Close window".to_owned());
    names.extend(rows_of(state));
    names
}

/// The buttons that are rows: each belongs to one item of a list, and is named by it.
fn rows_of(state: &AppState) -> Vec<String> {
    let mut rows = vec![document::SAVE_PDF.to_owned(), "Show only this".to_owned()];
    // A saved view, and its Delete.
    for view in &state.views {
        rows.push(view.name.clone());
        rows.push(format!("Delete {}", view.name));
    }
    // A measurement's Remove; its Go to carries the value, which is tested by its start.
    for item in &state.annotations.items {
        rows.push(format!("Remove {}", item.label));
    }
    // A part, and a group's Show or Hide and Go to.
    rows.extend(state.labels.parts.iter().map(|part| part.name.clone()));
    for group in parts::rows(&state.labels, "") {
        let name = title(&group);
        rows.extend(["Show", "Hide", "Go to"].map(|word| format!("{word} {name}")));
    }
    // A notice's Dismiss.
    rows.extend(
        state
            .notices
            .iter()
            .map(|notice| format!("Dismiss: {}", notice.text)),
    );
    // Where a run can go: a graphics card by its name, and the two the system always has.
    rows.extend(labelling::places_named(&state.labelling));
    // The answers to a question.
    for confirm in [
        Confirm::CloseLabels,
        Confirm::DeleteKept,
        Confirm::DeleteView(0),
    ] {
        if let Some(asked) = crate::ui::confirm::question(confirm, state) {
            rows.extend([asked.keep.to_owned(), asked.yes.to_owned()]);
        }
    }
    rows
}

/// Whether the name is a measurement's Go to, which says where it goes.
fn goes_to_a_measurement(state: &AppState, name: &str) -> bool {
    let goes = |item: &crate::annotations::Annotation| {
        name.starts_with(&format!("Go to {}, ", item.label))
    };
    state.annotations.items.iter().any(goes)
}

/// The buttons of a pass that are not in the rail, by name.
fn buttons<'a>(told: &'a Told, window: &Window) -> impl Iterator<Item = &'a Heard> {
    let rail = window.rail_at;
    let outside = move |node: &&Heard| node.at.is_some_and(|at| !rail.contains_rect(at));
    let button = |node: &&Heard| node.role == egui::accesskit::Role::Button && !node.hidden;
    told.nodes.iter().filter(outside).filter(button)
}

/// Fails if a button of the window in this scene and tab has a name that is not exactly one the
/// list, the menus or a row gives.
pub fn check_buttons(scene: &Scene, tab: Tab) {
    let (told, window) = told_in(scene, tab);
    let known = known_names(&scene.state);
    for button in buttons(&told, &window) {
        let name = button.name.as_str();
        let ok = known.contains(name) || goes_to_a_measurement(&scene.state, name);
        assert!(
            ok,
            "{}, {tab:?}: {name:?} is not a name the list of actions gives",
            scene.name
        );
    }
}

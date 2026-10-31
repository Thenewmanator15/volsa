//! The bar across the top: on one line in the smallest window, with Open, search and the menu
//! in it, and each of its buttons doing what the list of actions says.

use egui::{Context, Event, Pos2, Rect, vec2};

use super::*;
use crate::testing::thick_scan_ready;
use crate::ui::seen::{self, Seen, click_at};

/// The smallest window, in points of an interface drawn at its usual size.
const SMALLEST: [f32; 2] = [640.0, 480.0];
/// What the study of [`named_study`] is called on the bar.
const STUDY: &str = "2 January 2020 · Head MRI";

/// What one pass of the bar gave.
struct Pass {
    asked: Vec<Request>,
    actions: Vec<Action>,
    seen: Seen,
}

/// The bar in a panel across the top of a window, pass after pass.
struct Bar {
    ctx: Context,
    size: [f32; 2],
    state: AppState,
    search: Search,
    theme: ThemeChoice,
}

impl Bar {
    /// A window `size` points across at interface size `zoom`, with this state.
    fn new(size: [f32; 2], zoom: f32, state: AppState) -> Self {
        Self {
            ctx: seen::context(zoom),
            size,
            state,
            search: Search::default(),
            theme: ThemeChoice::System,
        }
    }

    /// One pass, with these events.
    fn pass(&mut self, events: Vec<Event>) -> Pass {
        let (state, theme, search) = (&self.state, self.theme, &mut self.search);
        let (given, seen) = seen::pass(&self.ctx, self.size, events, |ui| {
            let mut actions = Vec::new();
            let bar = egui::Panel::top("top bar");
            let asked = bar.show(ui, |ui| show(ui, state, search, theme, &mut actions));
            (asked.inner, actions)
        });
        let (asked, actions) = given;
        Pass {
            asked,
            actions,
            seen,
        }
    }

    /// A few passes, for the panel to take its size and a menu to have been laid out.
    fn settle(&mut self) -> Pass {
        let mut last = self.pass(Vec::new());
        for _ in 0..3 {
            last = self.pass(Vec::new());
        }
        last
    }

    /// Press and let go of the main button at a point: what the second pass gave.
    fn click(&mut self, at: Pos2) -> Pass {
        let [press, release] = click_at(at);
        let _ = self.pass(press);
        self.pass(release)
    }

    /// Click the thing a screen reader calls `name`, as the last pass drew it, and settle.
    fn click_named(&mut self, seen: &Seen, name: &str) -> Pass {
        let place = seen
            .place_of(name)
            .unwrap_or_else(|| panic!("{name} is drawn"));
        let clicked = self.click(place.center());
        let settled = self.settle();
        Pass {
            asked: clicked.asked,
            actions: clicked.actions,
            seen: settled.seen,
        }
    }
}

/// A made-up study with its date and name, shown in the active viewport.
fn named_study() -> AppState {
    let mut state = thick_scan_ready();
    let index = state.index.as_mut().expect("an export is open");
    let study = index.studies.first_mut().expect("it has a study");
    study.date = Some("20200102".to_owned());
    study.description = "Head MRI".to_owned();
    state
}

/// What a screen reader calls the button of the action `id`.
fn named(id: &str) -> &'static str {
    commands::find(id).expect("listed").name
}

#[test]
fn the_top_bar_is_one_line_at_640_wide() {
    let mut bar = Bar::new(SMALLEST, 1.0, named_study());
    let laid = bar.settle();
    // The study's date and name fit beside the buttons at this width.
    let said = |(said, _): &(String, Rect)| said == STUDY;
    assert!(laid.seen.words.iter().any(said), "{:?}", laid.seen.words);
    // Everything on the bar shares one row: the vertical ranges of all its widgets overlap.
    let widgets = bar
        .ctx
        .viewport(|viewport| viewport.prev_pass.widgets.clone());
    let rects: Vec<Rect> = widgets
        .layers()
        .flat_map(|(_, widgets)| widgets.iter())
        .map(|widget| widget.rect)
        .collect();
    assert!(rects.len() >= 5, "the bar's widgets are seen: {rects:?}");
    for (one, a) in rects.iter().enumerate() {
        for b in rects.iter().skip(one + 1) {
            assert!(
                a.y_range().intersects(b.y_range()),
                "{a:?} is not on the row of {b:?}"
            );
        }
    }
    let tallest = rects.iter().map(Rect::height).fold(0.0, f32::max);
    assert!(tallest < 40.0, "a row, not a block: {tallest}");
}

#[test]
fn the_top_bar_keeps_open_search_and_the_menu_at_640() {
    // The smallest window at the usual size, and at twice it, which holds half as many points.
    for (size, zoom) in [
        (SMALLEST, 1.0),
        ([SMALLEST[0] / 2.0, SMALLEST[1] / 2.0], 2.0),
    ] {
        let mut bar = Bar::new(size, zoom, named_study());
        let laid = bar.settle();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
        let wanted = [
            "Open",
            named("search.open"),
            named("help.keys"),
            named("theme.light"),
            MENU,
        ];
        let places: Vec<Rect> = wanted
            .iter()
            .map(|name| {
                laid.seen
                    .place_of(name)
                    .unwrap_or_else(|| panic!("{name} is on the bar"))
            })
            .collect();
        for (name, place) in wanted.iter().zip(&places) {
            assert!(
                screen.contains_rect(*place),
                "{name} at {place:?} is off {screen:?}"
            );
        }
        for (one, a) in places.iter().enumerate() {
            for b in places.iter().skip(one + 1) {
                assert!(
                    !a.intersect(*b).is_positive(),
                    "{a:?} covers {b:?} at {size:?}"
                );
            }
        }
        // The last button is at the right edge of the window, give or take its margin.
        let menu = places[wanted.len() - 1];
        assert!(
            screen.right() - menu.right() < 24.0,
            "{menu:?} in {screen:?}"
        );
    }
}

#[test]
fn the_study_is_named_with_its_date_and_given_up_first_when_the_bar_is_full() {
    let said = |seen: &Seen| seen.words.iter().any(|(said, _)| said == STUDY);
    let wide = Bar::new([1400.0, 900.0], 1.0, named_study()).settle();
    assert!(said(&wide.seen));
    // At a half the width the name is left out before the box or any button is.
    let narrow = Bar::new([330.0, 480.0], 1.0, named_study()).settle();
    assert!(!said(&narrow.seen), "{:?}", narrow.seen.words);
    for name in ["Open", MENU, named("search.open")] {
        assert!(narrow.seen.place_of(name).is_some(), "{name} is kept");
    }
    // With no export open there is no study to name.
    let none = Bar::new(SMALLEST, 1.0, AppState::default()).settle();
    assert!(none.seen.words.iter().all(|(said, _)| !said.contains('·')));
}

#[test]
fn the_open_menu_offers_a_folder_and_a_zip_file_or_dicomdir() {
    for (id, pick) in [("open.folder", Pick::Folder), ("open.file", Pick::File)] {
        let mut bar = Bar::new(SMALLEST, 1.0, AppState::default());
        let laid = bar.settle();
        // Opening the menu asks for nothing; it then offers both.
        let opened = bar.click_named(&laid.seen, "Open");
        assert_eq!(opened.asked, []);
        for wanted in ["open.folder", "open.file"] {
            assert!(opened.seen.place_of(named(wanted)).is_some(), "{wanted}");
        }
        let chosen = bar.click_named(&opened.seen, named(id));
        assert_eq!(chosen.asked, [Request::Open(pick)], "{id}");
    }
    let file = commands::find("open.file").expect("listed");
    assert_eq!(file.label, "Zip file or DICOMDIR…");
}

#[test]
fn the_search_box_asks_for_the_search_and_the_results_hang_under_it() {
    for size in [[1400.0, 900.0], SMALLEST] {
        let mut bar = Bar::new(size, 1.0, AppState::default());
        let laid = bar.settle();
        let found = laid
            .seen
            .place_of(named("search.open"))
            .expect("the search box");
        let words = |wanted: &str| laid.seen.words.iter().any(|(said, _)| said == wanted);
        assert!(
            words(SEARCH_SAYS) && words(SEARCH_KEY),
            "{:?}",
            laid.seen.words
        );
        let clicked = bar.click(found.center());
        assert_eq!(clicked.asked, [Request::Search]);
        // With the search open, the results hang under the box, not at the top of the window.
        bar.search.open();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(size[0], size[1]));
        let (state, theme) = (bar.state.clone(), bar.theme);
        let mut under = Rect::NOTHING;
        for _ in 0..4 {
            let search = &mut bar.search;
            let _ = seen::pass(&bar.ctx, size, Vec::new(), |ui| {
                let mut actions = Vec::new();
                egui::Panel::top("top bar")
                    .show(ui, |ui| show(ui, &state, search, theme, &mut actions));
                search.show(ui.ctx(), &state)
            });
            let area = bar
                .ctx
                .memory(|memory| memory.area_rect(egui::Id::new("voxa: search")));
            under = area.unwrap_or(Rect::NOTHING);
        }
        assert!(
            under.top() >= found.bottom(),
            "{under:?} is not under {found:?}"
        );
        assert!(
            under.top() - found.bottom() < 12.0,
            "{under:?} hangs far from {found:?}"
        );
        assert!(
            screen.contains_rect(under),
            "{under:?} is off the window {screen:?}"
        );
        // It is wider than the box, and spans it.
        let spans = under.left() <= found.left() + 1.0 && found.right() <= under.right() + 1.0;
        assert!(spans, "{under:?} does not hang from {found:?}");
    }
}

#[test]
fn the_theme_button_shows_the_theme_and_goes_round_system_light_dark() {
    let mut chosen = Vec::new();
    for theme in [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark] {
        let mut bar = Bar::new(SMALLEST, 1.0, AppState::default());
        bar.theme = theme;
        let laid = bar.settle();
        // The button is named for what it does: the choice after the one it shows.
        let button = named(theme_action(theme.next()));
        let clicked = bar.click_named(&laid.seen, button);
        assert_eq!(clicked.asked.len(), 1, "from {theme:?}");
        chosen.extend(clicked.asked);
    }
    let theme = Request::Theme;
    assert_eq!(
        chosen,
        [
            theme(ThemeChoice::Light),
            theme(ThemeChoice::Dark),
            theme(ThemeChoice::System)
        ]
    );
}

#[test]
fn the_menu_holds_size_full_screen_the_scans_the_tags_the_keys_the_log_and_about() {
    let wanted = [
        ("window.full_screen", Some(Request::FullScreen)),
        ("rail.fold", Some(Request::FoldRail)),
        ("tags.show", Some(Request::Tags)),
        ("help.keys", Some(Request::Help(None))),
        ("about", Some(Request::About)),
        ("log.where", Some(Request::WhereTheLogIs)),
    ];
    for (id, request) in wanted {
        let mut bar = Bar::new(SMALLEST, 1.0, named_study());
        let laid = bar.settle();
        let opened = bar.click_named(&laid.seen, MENU);
        let size = opened.seen.place_starting("Interface size 100%");
        assert!(
            size.is_some(),
            "the size is in the menu: {:?}",
            opened.seen.names
        );
        // Each item gives what the list of actions says it does.
        let done = bar.click_named(&opened.seen, named(id));
        assert_eq!(done.asked, request.into_iter().collect::<Vec<_>>(), "{id}");
        assert_eq!(
            done.actions,
            [],
            "{id} asks the window, and changes no state"
        );
    }
}

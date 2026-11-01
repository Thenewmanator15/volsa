//! The windows that float: where they open, and what Esc does to them.

use egui::{Context, Event, Id, Key, Rect};

use super::*;
use crate::state::AppState;
use crate::ui::seen::{self, key};
use crate::ui::tags::{self, Listing};

/// A window set out as Volsa's is, with the floating windows in it.
struct Window {
    ctx: Context,
    size: [f32; 2],
    /// How wide the column of scans at the left is, if it is shown.
    rail: Option<f32>,
    /// How wide the panel at the right is.
    panel: f32,
    about: bool,
    help: bool,
    tags: bool,
    search: crate::ui::search::Search,
}

/// Where the parts of the window were laid out.
struct Laid {
    pictures: Rect,
    rail: Option<Rect>,
}

impl Window {
    fn new(size: [f32; 2], rail: Option<f32>, panel: f32) -> Self {
        Self {
            ctx: seen::context(1.0),
            size,
            rail,
            panel,
            about: false,
            help: false,
            tags: false,
            search: crate::ui::search::Search::default(),
        }
    }

    /// One pass, with these events: the rail, the panel, the pictures in what is left, and
    /// then the windows over the pictures, as the window lays them out.
    fn pass(&mut self, events: Vec<Event>) -> Laid {
        let state = AppState::default();
        let listing: Listing = Ok(Vec::new());
        let (rail, panel) = (self.rail, self.panel);
        let (about, help, tags) = (&mut self.about, &mut self.help, &mut self.tags);
        let search = &mut self.search;
        let (laid, _) = seen::pass(&self.ctx, self.size, events, |ui| {
            let list = rail.map(|width| {
                let column = egui::Panel::left("rail").exact_size(width);
                column
                    .show(ui, |ui| ui.allocate_space(ui.available_size()))
                    .response
                    .rect
            });
            let column = egui::Panel::right("panel").exact_size(panel);
            column.show(ui, |ui| ui.allocate_space(ui.available_size()));
            let middle = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(ui.available_size());
            });
            let pictures = middle.response.rect;
            let ctx = ui.ctx().clone();
            keep_to(&ctx, pictures);
            close_on_esc(&ctx, Open { about, help, tags });
            about::show(&ctx, about);
            help::show(&ctx, help);
            tags::show(&ctx, tags, &state, &listing, &mut Vec::new());
            search.show(&ctx, &state);
            Laid {
                pictures,
                rail: list,
            }
        });
        laid
    }

    /// Passes enough for the windows to have their sizes and places.
    fn settle(&mut self) -> Laid {
        let mut laid = self.pass(Vec::new());
        for _ in 0..3 {
            laid = self.pass(Vec::new());
        }
        laid
    }

    /// Where a window is, if it is shown.
    fn rect_of(&self, id: Id) -> Option<Rect> {
        self.ctx.memory(|memory| memory.area_rect(id))
    }
}

#[test]
fn floating_windows_open_over_the_pictures_and_close_with_esc() {
    // A big window with both columns, a medium one with both, and the smallest, where only the
    // panel is open.
    let layouts = [
        ([1400.0, 900.0], Some(260.0), 340.0),
        ([800.0, 600.0], Some(200.0), 260.0),
        ([640.0, 480.0], None, 260.0),
    ];
    for (size, rail, panel) in layouts {
        let mut window = Window::new(size, rail, panel);
        (window.about, window.help, window.tags) = (true, true, true);
        let laid = window.settle();
        let ids = [
            ("About", about::id()),
            ("the keys", help::id()),
            ("the tags", tags::id()),
        ];
        for (name, id) in ids {
            let shown = window
                .rect_of(id)
                .unwrap_or_else(|| panic!("{name} is shown at {size:?}"));
            assert!(
                laid.pictures.expand(0.5).contains_rect(shown),
                "{name} at {shown:?} is not over the pictures at {:?} in {size:?}",
                laid.pictures
            );
            if let Some(rail) = laid.rail {
                assert!(
                    !shown.intersect(rail).is_positive(),
                    "{name} at {shown:?} is over the rail at {rail:?}"
                );
            }
        }
        // Esc closes the window that is on top, and only that one: the last opened is.
        let escape = || vec![key(Key::Escape)];
        window.pass(escape());
        assert_eq!(
            (window.about, window.help, window.tags),
            (true, true, false),
            "{size:?}"
        );
        window.settle();
        window.pass(escape());
        assert_eq!(
            (window.about, window.help, window.tags),
            (true, false, false),
            "{size:?}"
        );
        window.settle();
        window.pass(escape());
        assert_eq!(
            (window.about, window.help, window.tags),
            (false, false, false),
            "{size:?}"
        );
    }
}

#[test]
fn a_window_that_is_moved_stays_over_the_pictures() {
    let mut window = Window::new([1400.0, 900.0], Some(260.0), 340.0);
    window.about = true;
    let laid = window.settle();
    // Drag the window by its title as far left as the pointer goes: over the rail.
    let shown = window.rect_of(about::id()).expect("About is shown");
    let from = shown.center_top() + egui::vec2(0.0, 8.0);
    let [press, _] = seen::click_at(from);
    window.pass(press);
    let to = egui::pos2(5.0, from.y);
    window.pass(vec![Event::PointerMoved(to)]);
    window.settle();
    let moved = window.rect_of(about::id()).expect("About is shown");
    let rail = laid.rail.expect("the rail is shown");
    assert!(
        !moved.intersect(rail).is_positive(),
        "{moved:?} is over the rail at {rail:?}"
    );
    assert!(laid.pictures.expand(0.5).contains_rect(moved), "{moved:?}");
}

#[test]
fn esc_is_left_to_the_search_box_while_it_is_open() {
    let mut window = Window::new([900.0, 600.0], Some(200.0), 260.0);
    window.help = true;
    window.search.open();
    window.settle();
    assert!(window.help);
    // The first Esc closes the search, and the window stays; the next closes the window.
    window.pass(vec![key(Key::Escape)]);
    assert!(window.help, "the search had the Esc");
    window.settle();
    window.pass(vec![key(Key::Escape)]);
    assert!(!window.help);
}

#[test]
fn esc_closes_the_licences_window_before_the_about_window_that_opened_it() {
    let mut window = Window::new([1400.0, 900.0], Some(260.0), 340.0);
    window.about = true;
    window.settle();
    about::open_notices(&window.ctx);
    window.settle();
    assert!(about::notices_are_open(&window.ctx));
    window.pass(vec![key(Key::Escape)]);
    assert!(
        !about::notices_are_open(&window.ctx),
        "the licences are closed first"
    );
    assert!(window.about);
    window.settle();
    window.pass(vec![key(Key::Escape)]);
    assert!(!window.about);
}

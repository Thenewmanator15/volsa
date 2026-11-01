//! The window that shows how far a turntable has got: where it opens, and what stops it.

use std::path::PathBuf;

use egui::{Event, Key, Pos2, Rect, vec2};
use volsa_core::{Point, WindowLevel};
use volsa_render::{Camera, Look, NamedView, SceneParams};

use super::*;
use crate::ui::floating::{self, Open, close_on_esc};
use crate::ui::seen::{self, key};

/// A turntable of a made-up scene, none of whose pictures has been drawn.
fn making() -> Turntable {
    let scene = SceneParams {
        camera: Camera::looking(NamedView::Anterior, Point::new(0.0, 0.0, 0.0), 0.5),
        window: WindowLevel::new(200.0, 400.0),
        look: Look::Surface,
        cuts: Vec::new(),
        slice: None,
        quality: 0.5,
        skip_empty: true,
    };
    Turntable::new(PathBuf::from("volsa-3d.gif"), &scene, [400.0, 300.0])
}

/// A window of 900 by 600 points with the pictures in the middle of it, as the window lays out
/// the floating windows, and the About window open too.
struct Window {
    ctx: Context,
    pictures: Rect,
    about: bool,
    help: bool,
    tags: bool,
    making: Option<Turntable>,
}

impl Window {
    fn new() -> Self {
        Self {
            ctx: seen::context(1.0),
            pictures: Rect::from_min_max(Pos2::new(260.0, 40.0), Pos2::new(620.0, 560.0)),
            about: false,
            help: false,
            tags: false,
            making: Some(making()),
        }
    }

    /// One pass, as the window runs one: the floating windows' Esc, then the turntable saving.
    /// Gives whether the person stopped the turntable in it.
    fn pass(&mut self, events: Vec<Event>) -> bool {
        let (about, help, tags) = (&mut self.about, &mut self.help, &mut self.tags);
        let (pictures, making) = (self.pictures, self.making.as_ref());
        let (stopped, _) = seen::pass(&self.ctx, [900.0, 600.0], events, |ui| {
            let ctx = ui.ctx().clone();
            floating::keep_to(&ctx, pictures);
            close_on_esc(&ctx, Open { about, help, tags });
            crate::ui::about::show(&ctx, about);
            making.is_some_and(|making| progress(&ctx, making))
        });
        if stopped {
            self.making = None;
        }
        stopped
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.pass(Vec::new());
        }
    }
}

#[test]
fn the_progress_window_draws_and_is_not_stopped_unless_asked() {
    let mut window = Window::new();
    assert!(!window.pass(Vec::new()));
    assert!(is_open(&window.ctx), "it was drawn");
}

#[test]
fn the_window_opens_over_the_pictures_not_the_middle_of_the_whole_window() {
    let mut window = Window::new();
    window.settle();
    let shown = window.ctx.memory(|memory| memory.area_rect(id()));
    let shown = shown.expect("the window is shown");
    assert!(
        window.pictures.expand(0.5).contains_rect(shown),
        "{shown:?} is not over the pictures at {:?}",
        window.pictures
    );
    let whole = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0));
    assert_ne!(
        shown.center(),
        whole.center(),
        "not centred on the whole window"
    );
}

#[test]
fn esc_stops_the_turntable_and_only_the_window_on_top_hears_it() {
    let mut window = Window::new();
    window.about = true;
    window.settle();
    assert!(is_open(&window.ctx));
    // The turntable was opened last, so the first Esc is its: About stays.
    assert!(window.pass(vec![key(Key::Escape)]), "Esc stopped it");
    assert!(window.about, "About was not closed by it");
    assert!(window.making.is_none());
    window.settle();
    assert!(!is_open(&window.ctx), "the window is gone");
    // The next Esc closes About, as before.
    window.pass(vec![key(Key::Escape)]);
    assert!(!window.about);
}

#[test]
fn an_esc_that_found_no_turntable_does_not_stop_the_next_one() {
    let mut window = Window::new();
    window.settle();
    // The turntable ends of itself, and an Esc comes as it does.
    window.making = None;
    window.pass(vec![key(Key::Escape)]);
    window.making = Some(making());
    assert!(
        !window.pass(Vec::new()),
        "the old Esc is not heard by a new turntable"
    );
}

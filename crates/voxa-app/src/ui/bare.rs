//! The window without its pictures, laid out with no screen behind it: the bar across the top,
//! the line along the bottom, the rail of scans, the tab panel with the tab that is open, and a
//! blank place where the pictures go. What the tests of the window's small sizes lay out, and
//! what the example `measure_window` times, to hold the window's own work (spec 7.17) to what
//! 0.6.0's bars and list took.

use egui::{Context, RawInput, Rect, Ui, pos2, vec2};

use super::browser::Thumbnails;
use super::screen::Screen;
use super::search::Search;
use super::theme::{self, ThemeChoice};
use super::{bars, layout, panel, rail, tabs};
use crate::prefs::Arrangement;
use crate::state::AppState;

/// Where the parts of the window were laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BareLayout {
    /// The rail of scans at the left, or its strip while it is folded.
    pub rail: Rect,
    /// The tab panel at the right, or its strip while it is folded.
    pub panel: Rect,
    /// The room the pictures have between them.
    pub pictures: Rect,
}

/// A window set out as Voxa sets it out, but for its pictures.
pub struct BareWindow {
    ctx: Context,
    size: [f32; 2],
    state: AppState,
    screen: Screen,
    search: Search,
    bars: bars::Kept,
    rail: rail::RailState,
    tabs: tabs::Kept,
    thumbnails: Thumbnails,
}

impl BareWindow {
    /// A window `size` points across and down showing `state`, with the rail open and the
    /// panel open on its first tab.
    pub fn new(state: AppState, size: [f32; 2]) -> Self {
        let ctx = Context::default();
        theme::apply(&ctx, ThemeChoice::Dark);
        Self::in_context(ctx, state, size)
    }

    /// As [`BareWindow::new`], drawn in this context, which a test may have listen to a screen
    /// reader.
    pub(super) fn in_context(ctx: Context, state: AppState, size: [f32; 2]) -> Self {
        let mut screen = Screen::new(Arrangement::default(), false, false);
        screen.list_shown = true;
        Self {
            ctx,
            size,
            state,
            screen,
            search: Search::default(),
            bars: bars::Kept::default(),
            rail: rail::RailState::default(),
            tabs: tabs::Kept::default(),
            thumbnails: Thumbnails::default(),
        }
    }

    /// Give the window another size, as dragging its edge does.
    pub fn resize(&mut self, size: [f32; 2]) {
        self.size = size;
    }

    /// One frame of the window, as the window library would run it: everything laid out and
    /// drawn, and the drawing turned into triangles. Gives where the parts are.
    pub fn lay_out(&mut self) -> BareLayout {
        let [wide, high] = self.size;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(wide, high))),
            ..RawInput::default()
        };
        let ctx = self.ctx.clone();
        let mut laid = None;
        let mut output = ctx.run_ui(input, |ui| laid = Some(self.body(ui)));
        // No renderer is behind these frames to take the font texture; the triangles are what
        // one would be given.
        output.textures_delta.clear();
        let _triangles = ctx.tessellate(output.shapes, output.pixels_per_point);
        laid.unwrap_or(BareLayout {
            rail: Rect::NOTHING,
            panel: Rect::NOTHING,
            pictures: Rect::NOTHING,
        })
    }

    /// What one frame of the window draws in `ui`, in the order the window draws it. A column
    /// the window is too narrow for is folded first, and the others give way to the pictures.
    pub(super) fn body(&mut self, ui: &mut Ui) -> BareLayout {
        let window = ui.ctx().content_rect().width();
        self.screen.settle(window);
        let mut actions = Vec::new();
        let state = &self.state;
        bars::show(ui, state, &mut self.search, &mut self.bars, &mut actions);
        let widths = self.screen.widths(window);
        let open = self.screen.list_shown;
        let thumbnails = &self.thumbnails;
        let (rail, _) = rail::column(
            ui,
            widths.rail,
            open,
            state,
            thumbnails,
            &mut self.rail,
            &mut actions,
        );
        let mut middle = layout::beside_the_panel(ui, widths.panel);
        let blank = egui::CentralPanel::default().frame(super::middle());
        let pictures = blank.show(&mut middle, |ui| {
            ui.allocate_space(ui.available_size());
        });
        let kept = &mut self.tabs;
        let contents = |ui: &mut Ui, tab| tabs::show(ui, tab, state, kept, &mut actions);
        let panel = panel::show(ui, &mut self.screen.panel, state, widths.panel, contents);
        BareLayout {
            rail,
            panel: panel.rect,
            pictures: pictures.response.rect,
        }
    }
}

//! What the window does in each frame besides drawing: moving playback on, making the small
//! pictures of the scans, and the rail of scans down the left with the edge that is dragged.

use std::time::Instant;

use super::{VoxaApp, browser, commands, rail, shares, thumbnail};
use crate::action::Action;
use crate::state::VIEWPORTS;

/// Thumbnails are made at twice their size on screen, to stay sharp on dense displays.
const THUMBNAIL_PIXELS: u32 = 128;

impl VoxaApp {
    /// Make the small picture of each series whose middle slice has arrived. None is made, or
    /// kept, of a page while the patient's details are hidden.
    pub(super) fn make_thumbnails(&mut self, ctx: &egui::Context) {
        let Some(index) = &self.session.state().index else {
            self.thumbnails.clear();
            return;
        };
        let show_patient = self.session.state().show_patient;
        for series in &index.series {
            let id = &series.info.id;
            if !browser::may_have_thumbnail(&series.info, show_patient) {
                self.thumbnails.remove(id);
                continue;
            }
            if self.thumbnails.contains_key(id) {
                continue;
            }
            let middle = series.slices.len() / 2;
            let window = self.session.state().windows.get(&(id.clone(), middle));
            // Only with its window: a slice read out of a volume is a copy made for the asking.
            let frame = window.and_then(|_| self.session.cached(id, middle));
            let (Some(frame), Some([automatic, ..])) = (frame, window) else {
                continue;
            };
            let picture = thumbnail(&frame, *automatic, THUMBNAIL_PIXELS);
            let name = format!("thumbnail {}", id.0);
            let texture = ctx.load_texture(name, picture, egui::TextureOptions::LINEAR);
            self.thumbnails.insert(id.clone(), texture);
        }
    }
}

impl VoxaApp {
    /// Move playback on in every viewport where it is running, and ask to be woken when the
    /// next slice is due, so nothing is redrawn in between.
    pub(super) fn play(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        for viewport in 0..VIEWPORTS {
            let cine = self
                .session
                .state()
                .viewports
                .get(viewport)
                .and_then(|shown| shown.cine);
            let Some(clock) = self.clocks.get_mut(viewport) else {
                continue;
            };
            let Some(cine) = cine.filter(|cine| cine.playing) else {
                clock.stop();
                continue;
            };
            let (ticks, wait) = clock.advance(now, cine.fps);
            for _ in 0..ticks {
                self.dispatch(Action::CineTick { viewport });
            }
            ctx.request_repaint_after(wait);
        }
    }
}

impl VoxaApp {
    /// The rail of scans down the left of the window, taking its share of the window's width,
    /// or less where the pictures need it, or folded to its strip: where it is while it is
    /// open, and what it asked of the window.
    pub(super) fn list(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) -> (Option<egui::Rect>, Vec<commands::Request>) {
        let window = ui.ctx().content_rect().width();
        let width = self.screen.widths(window).rail;
        let open = self.screen.list_shown;
        let (state, thumbnails) = (self.session.state(), &self.thumbnails);
        let (rect, asked) =
            rail::column(ui, width, open, state, thumbnails, &mut self.rail, actions);
        (open.then_some(rect), asked)
    }

    /// Let the right edge of the series list, at `list`, be dragged to give it more or less
    /// of the window's width. Made after everything beside it, so that the drag is the edge's.
    pub(super) fn drag_the_list(&mut self, ui: &egui::Ui, list: Option<egui::Rect>) {
        let Some(list) = list else {
            return;
        };
        let edge = egui::Id::new("series list edge");
        let by = shares::divider(ui, edge, list.right(), list.y_range(), "Width of the scans");
        if by.abs() > 0.0 {
            let window = ui.ctx().content_rect().width();
            let share = shares::list_dragged(window, self.screen.list_share(), by);
            self.screen.set_list_share(share);
        }
    }
}

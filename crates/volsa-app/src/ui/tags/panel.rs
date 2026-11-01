//! The tag browser's window with the listing it shows, and the asking of a worker for it.

use std::sync::mpsc::{Receiver, Sender, channel};

use volsa_core::SeriesId;

use super::{Listing, show};
use crate::action::Action;
use crate::state::AppState;
use crate::ui::commands::TagsOf;
use crate::worker::Workers;

/// A file whose tags have been asked for: the one behind a slice of a series, or one named by
/// its entry in the export.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Asked {
    /// The file behind this slice of this series.
    Slice(SeriesId, usize),
    /// The file stored under this name.
    Entry(String),
}

/// The tag browser's window and the listing it shows. The rows hold the patient's details,
/// so they are kept here and never put into the application state or an action.
pub struct Panel {
    /// Whether the window is open.
    pub open: bool,
    /// Whose tags it shows.
    of: TagsOf,
    /// The file whose tags were last asked for.
    asked: Option<Asked>,
    listing: Listing,
    send: Sender<(Asked, Listing)>,
    replies: Receiver<(Asked, Listing)>,
}

impl Default for Panel {
    fn default() -> Self {
        let (send, replies) = channel();
        Self {
            open: false,
            of: TagsOf::Shown,
            asked: None,
            listing: Ok(Vec::new()),
            send,
            replies,
        }
    }
}

impl Panel {
    /// Open the window on the tags `of` a file.
    pub fn open_on(&mut self, of: TagsOf) {
        self.of = of;
        self.open = true;
    }

    /// While the window is open, ask a worker for the tags of the file it shows whenever that
    /// file changes, and take in any answer: the first file of a series, the file behind the
    /// slice the active viewport shows, or a file named by its entry.
    pub fn refresh(&mut self, state: &AppState, workers: &Workers, ctx: &egui::Context) {
        if !self.open {
            self.asked = None;
            return;
        }
        let wanted = match &self.of {
            TagsOf::Series(series) => Some(Asked::Slice(series.clone(), 0)),
            TagsOf::Entry(entry) => Some(Asked::Entry(entry.clone())),
            TagsOf::Shown => {
                let viewport = state.viewports.get(state.active);
                let shown = viewport.and_then(|v| v.series.clone().map(|series| (series, v.slice)));
                shown.map(|(series, slice)| Asked::Slice(series, slice))
            }
        };
        if wanted != self.asked {
            self.asked.clone_from(&wanted);
            self.listing = Ok(Vec::new());
            if let Some(asked) = wanted {
                self.ask(asked, state, workers, ctx);
            }
        }
        while let Ok((key, rows)) = self.replies.try_recv() {
            if self.asked.as_ref() == Some(&key) {
                self.listing = rows;
            }
        }
    }

    /// Ask a worker for the tags of a file, to be answered to [`Panel::refresh`].
    fn ask(&self, asked: Asked, state: &AppState, workers: &Workers, ctx: &egui::Context) {
        let (send, ctx, key) = (self.send.clone(), ctx.clone(), asked.clone());
        let reply = move |rows| {
            // If the window has gone there is no one left to tell.
            let _ = send.send((key, rows));
            ctx.request_repaint();
        };
        match asked {
            Asked::Slice(series, slice) => {
                if let Some(listed) = state.series(&series) {
                    workers.list_tags(listed, slice, reply);
                }
            }
            Asked::Entry(entry) => workers.list_tags_of_entry(entry, reply),
        }
    }

    /// Draw the window if it is open.
    pub fn show(&mut self, ctx: &egui::Context, state: &AppState, actions: &mut Vec<Action>) {
        show(ctx, &mut self.open, state, &self.listing, actions);
    }
}

#[cfg(test)]
mod tests;

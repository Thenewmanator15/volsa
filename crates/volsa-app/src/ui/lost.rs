//! The graphics card the window draws with, and the day it stops answering.
//!
//! A driver reset can take the card away from the window, and `eframe` has no way to give the
//! window another: the card it started with is the only one it will ever draw with. Neither
//! does it keep the program going when the card is gone; left to itself, the next thing
//! asked of the card stops the program. So the card is watched from the start. When it is
//! gone, the window carries on without it, the work in hand goes on, and the person is told
//! once to start Volsa again. A window that can paint nothing cannot show that, so it is said
//! in a message box of the system as well as in the window's own notices and the log.

mod message;

use egui_wgpu::RenderState;
use volsa_render::DeviceWatch;

use crate::action::Action;
use crate::state::{Notice, NoticeKind};

/// The key of the notice, which is Volsa's own words from first to last.
pub const KEY: &str = "graphics";
/// What is said when the graphics card stops answering.
pub const STOPPED: &str = "The graphics card stopped answering. Close Volsa and open it again. \
                           Nothing you made has been lost.";

/// How the words are put in front of the person in a window of the system.
pub type Show = Box<dyn FnMut(&str)>;

/// Watches the graphics card the window draws with.
pub struct Card {
    watch: Option<DeviceWatch>,
    /// Whether the person has been told that it stopped answering.
    told: bool,
    /// How to show the words in a window of the system.
    show: Show,
}

impl Card {
    /// Watch the card the window draws with, if it draws with one. The window is woken when
    /// the card stops answering, so that it looks.
    pub fn watching(render: Option<&RenderState>, ctx: &egui::Context) -> Self {
        let ctx = ctx.clone();
        let wake = move || ctx.request_repaint();
        let watch = render.map(|render| DeviceWatch::on(&render.device, wake));
        Self::new(watch, Box::new(message::in_a_window))
    }

    /// A card that `watch` keeps watch on; none, if the window has no card. `show` puts words
    /// in a window of the system.
    pub fn new(watch: Option<DeviceWatch>, show: Show) -> Self {
        Self {
            watch,
            told: false,
            show,
        }
    }

    /// Look at the card. If it has stopped answering, and the person has not been told, `say`
    /// is given the action that tells them, and a message box of the system is asked for with
    /// the same words; each is done once. The window paints nothing from then on, so the box
    /// is what the person sees.
    pub fn look(&mut self, say: impl FnOnce(Action)) {
        let Some(watch) = &self.watch else {
            return;
        };
        if watch.is_lost() && !self.told {
            self.told = true;
            say(Action::Post(stopped()));
            (self.show)(STOPPED);
        }
    }
}

/// The notice that the card stopped answering.
fn stopped() -> Notice {
    Notice {
        kind: NoticeKind::Error,
        text: STOPPED.to_owned(),
        details: Vec::new(),
        key: Some(KEY),
    }
}

#[cfg(test)]
mod tests;

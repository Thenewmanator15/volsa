//! The window: the top bar, the rail of scans, the pictures under the strip that arranges them,
//! the tab panel and the bottom line.

use crate::action::Action;
use crate::session::Session;
use crate::state::VIEWPORTS;
use layout::middle;

mod about;
mod access;
#[cfg(test)]
mod access_tests;
mod bare;
mod bars;
mod browser;
mod capture;
mod cine;
mod commands;
mod confirm;
#[cfg(test)]
mod described_tests;
mod document;
mod floating;
mod frame;
#[cfg(test)]
mod hand;
mod help;
mod icon;
mod keys;
mod labels;
mod labels_placed;
mod layout;
pub(crate) mod lost;
mod notices;
mod overlay;
mod panel;
mod parts;
mod pen;
mod pictures;
mod planes;
mod rail;
mod remembered;
mod requests;
mod saving;
mod scene;
mod screen;
mod scrolled;
mod search;
#[cfg(test)]
mod seen;
mod setup;
mod shares;
mod size;
mod start;
mod strip;
mod symbols;
mod tabs;
mod tags;
mod theme;
mod top;
mod typing;
mod viewport;
mod volumes;
mod welcome;

pub use bare::{BareLayout, BareWindow};
pub use browser::thumbnail;
pub use start::run;

/// The Voxa window.
pub struct VoxaApp {
    session: Session,
    /// One for each viewport; empty when the window has no graphics device to draw slices.
    gpus: Vec<viewport::Gpu>,
    /// What is being drawn on the slice of each viewport.
    drafts: viewport::Drafts,
    scene: scene::SceneView,
    planes: planes::PlanesView,
    thumbnails: browser::Thumbnails,
    /// What the rail of scans keeps between frames: whether the other files are open.
    rail: rail::RailState,
    about_open: bool,
    help_open: bool,
    /// The work the person is being asked about before it is thrown away, if any.
    confirming: Option<commands::Confirm>,
    tags: tags::Panel,
    /// What the bars keep between frames: the theme, and whether the list of notices is open.
    bars: bars::Kept,
    /// What is kept of the lines of the report shown between frames.
    document_view: document::Kept,
    /// The search box, which finds actions, parts, scans and help topics.
    search: search::Search,
    /// What the tabs keep between frames: the name being typed in the Measure tab, and the
    /// search typed in the Labels tab.
    tabs: tabs::Kept,
    /// The label file to open, and the one that is open.
    labels: labels::Opening,
    /// The labels on the graphics device.
    held: labels::Held,
    /// When playback in each viewport last moved on.
    clocks: [cine::Clock; VIEWPORTS],
    capture: Option<capture::Capture>,
    /// What a picture of the window wants done to the 3D scene once its volume is ready.
    pose: Vec<Action>,
    /// And to the labelled parts, once labels have been read.
    label_pose: Vec<Action>,
    /// A turntable of the 3D scene that is being drawn, a picture each frame.
    turntable: Option<scene::export::Turntable>,
    /// A picture or a turntable of the 3D scene asked for this frame, made once the frame's
    /// graphics device is in hand.
    scene_asked: Option<scene::export::Asked>,
    /// Whether labelling the scan was asked for this frame, done once the frame's graphics
    /// device is in hand to say how much of the card the window holds itself.
    label_asked: bool,
    /// What is remembered from one run to the next.
    kept: crate::prefs::Kept,
    /// The graphics card the window draws with, watched for the day it stops answering.
    card: lost::Card,
    /// How the window is set out: its size, the widths of the series list and the tab panel,
    /// whether the list is shown, and which tab the panel shows and whether it is folded.
    screen: screen::Screen,
}

impl VoxaApp {
    /// Apply an action and hand the work it calls for to the workers.
    fn dispatch(&mut self, action: Action) {
        self.session.dispatch(action);
    }
}

impl eframe::App for VoxaApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.screen.open(ctx, frame);
        self.session.pump();
        self.card.look(|action| self.session.dispatch(action));
        self.remember(ctx);
        self.open_waiting_labels();
        self.play(ctx);
        self.make_thumbnails(ctx);
        self.tags
            .refresh(self.session.state(), self.session.workers(), ctx);
    }

    /// The window, drawn in this order: the bars, the rail of scans, the middle beside the panel,
    /// the notices, then the windows that float. `BareWindow::body` (`ui/bare.rs`) lays out the
    /// same order by hand, and the small-window tests and UI-37 rest on it: change both.
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        // A narrow window folds a side column before any is drawn.
        let window = ui.ctx().content_rect().width();
        self.screen.settle(window);
        let state = self.session.state();
        let search = &mut self.search;
        let (asked, line) = bars::show(ui, state, search, &mut self.bars, &mut actions);
        for request in asked {
            self.ask(ui.ctx(), request);
        }
        if let Some(path) = top::dropped(ui.ctx()) {
            actions.push(self.labels.opening(path));
        }
        let (list, asked) = self.list(ui, &mut actions);
        for request in asked {
            self.ask(ui.ctx(), request);
        }
        let (panel, pictures) = self.pictures_then_panel(ui, frame, &mut actions);
        self.drag_the_list(ui, list);
        self.drag_the_panel(ui, panel);
        // The notice area is drawn last of the window's widgets, so that Tab reaches it last.
        let state = self.session.state();
        let asked = bars::draw_notices(ui, state, &line, &mut self.bars, &mut actions);
        for request in asked {
            self.ask(ui.ctx(), request);
        }
        self.windows(ui.ctx(), pictures, &mut actions);
        self.keys(ui.ctx(), &mut actions);
        self.show_search(ui.ctx(), &mut actions);
        actions.extend(confirm::show(
            ui.ctx(),
            self.session.state(),
            &mut self.confirming,
        ));
        let saving = self.scene_asked.take();
        self.save_scene(ui.ctx(), frame.wgpu_render_state(), saving, &mut actions);
        if std::mem::take(&mut self.label_asked) {
            let (state, render) = (self.session.state(), frame.wgpu_render_state());
            actions.push(parts::labelling::label_when_asked(state, render));
        }
        for action in actions {
            self.dispatch(action);
        }
        self.for_a_picture(ui.ctx());
        typing::note_the_pass(ui.ctx());
        // A column opened this frame in a narrow window folds the other in the next.
        if self.screen.unsettled(window) {
            ui.ctx().request_repaint();
        }
    }
}

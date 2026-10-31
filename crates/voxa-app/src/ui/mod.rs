//! The window: toolbar, series list, the pictures under the strip that arranges them, the tab
//! panel and status line.

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Instant;

use crate::action::Action;
use crate::labelling::LabellingAction;
use crate::session::Session;
use crate::state::VIEWPORTS;

mod about;
mod bars;
mod browser;
mod capture;
mod cine;
mod commands;
#[cfg(test)]
mod described_tests;
#[cfg(test)]
mod hand;
mod help;
mod icon;
mod keys;
mod labels;
mod layout;
pub(crate) mod lost;
mod overlay;
mod panel;
mod parts;
mod pen;
mod pictures;
mod planes;
mod remembered;
mod requests;
mod saving;
mod scene;
mod screen;
mod scrolled;
mod search;
mod shares;
mod size;
mod start;
mod status;
mod strip;
mod tabs;
mod tags;
mod theme;
mod toolbar;
mod viewport;
mod volumes;

pub use browser::thumbnail;
pub use start::run;

/// Decoded slices kept in memory: one gibibyte of pixel values.
const CACHE_BYTES: usize = 1 << 30;
/// Thumbnails are made at twice their size on screen, to stay sharp on dense displays.
const THUMBNAIL_PIXELS: u32 = 128;

/// The Voxa window.
pub struct VoxaApp {
    session: Session,
    /// One for each viewport; empty when the window has no graphics device to draw slices.
    gpus: Vec<viewport::Gpu>,
    scene: scene::SceneView,
    planes: planes::PlanesView,
    thumbnails: browser::Thumbnails,
    about_open: bool,
    help_open: bool,
    tags: tags::Panel,
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
    /// Tell the workers what the window's graphics device is like, and give it to them.
    fn tell_the_workers_of(&self, render: &egui_wgpu::RenderState) {
        let workers = self.session.workers();
        // Volumes are made ready on the workers to fit this device.
        workers.set_volume_side(render.device.limits().max_texture_dimension_3d);
        // What this device works out itself is not worked out on a worker first.
        let pipeline = self.scene.volume_pipeline();
        let blocks = pipeline.is_some_and(|pipeline| pipeline.makes_blocks(&render.device));
        let curves = pipeline.is_some_and(|pipeline| pipeline.makes_curves(&render.device));
        workers.leave_to_the_device(blocks, curves);
        // And a volume is handed to the device by the worker that made it ready.
        workers.give_device(render.device.clone(), render.queue.clone());
    }

    /// Tell labelling the name of the graphics card the window draws on, if it draws on one,
    /// and look on a worker at whether this computer can label a scan.
    fn look_for_labelling(&mut self, render: Option<&egui_wgpu::RenderState>) {
        if let Some(render) = render {
            // A run on the card Voxa draws on leaves room for what Voxa holds there.
            let drawing_on = render.adapter.get_info().name;
            self.dispatch(Action::Labelling(LabellingAction::DrawingOn(drawing_on)));
        }
        // Whether this computer can label a scan is looked at once as Voxa starts, on a
        // worker, and again whenever a folder for it is chosen. A picture of the strip is
        // taken as if on a computer that can, and makes no look.
        let look = capture::made_up_look().unwrap_or(LabellingAction::Look);
        self.dispatch(Action::Labelling(look));
    }

    /// Set the window up, and open `initial` if a path was given.
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        initial: Option<PathBuf>,
        labels: Option<PathBuf>,
    ) -> Self {
        theme::apply(&creation.egui_ctx);
        // Workers report on their own channel; a small thread passes each report on and wakes
        // the window, so the workers need know nothing about the interface.
        let (from_workers, to_relay) = channel();
        let (to_window, reports) = channel();
        let ctx = creation.egui_ctx.clone();
        std::thread::spawn(move || {
            for report in to_relay {
                if to_window.send(report).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        let render = creation.wgpu_render_state.as_ref();
        crate::log::device(render);
        let gpus =
            render.map(|render| (0..VIEWPORTS).map(|_| viewport::Gpu::new(render)).collect());
        let capture = capture::Capture::from_environment();
        let (settings, local) = capture::folders(capture.is_some());
        let prefs = crate::prefs::Kept::new(settings);
        let arranged = prefs.remembered().arrangement();
        creation.egui_ctx.set_zoom_factor(arranged.interface_size);
        // A picture is of a window the size it asks for, not one fitted to this screen,
        // unless it asks to see the fitting.
        let fitted = capture.is_none() || capture::wants("fitted");
        let hidden = capture.is_some() && capture::wants("nolist");
        let mut app = Self {
            screen: screen::Screen::new(arranged, fitted, hidden),
            kept: prefs,
            session: Session::new(from_workers, reports, CACHE_BYTES, local),
            gpus: gpus.unwrap_or_default(),
            scene: scene::SceneView::new(render),
            planes: planes::PlanesView::new(render),
            thumbnails: browser::Thumbnails::default(),
            about_open: false,
            help_open: false,
            tags: tags::Panel::default(),
            search: search::Search::default(),
            tabs: tabs::Kept::default(),
            labels: labels::Opening::waiting_for(labels),
            held: labels::Held::default(),
            clocks: [cine::Clock::default(); VIEWPORTS],
            capture,
            pose: Vec::new(),
            label_pose: Vec::new(),
            turntable: None,
            scene_asked: None,
            label_asked: false,
            card: lost::Card::watching(render, &creation.egui_ctx),
        };
        app.recall();
        if let Some(render) = render {
            app.tell_the_workers_of(render);
        }
        app.look_for_labelling(render);
        if app.capture.is_some() {
            app.set_up_a_picture(&creation.egui_ctx);
        }
        if let Some(path) = initial {
            app.dispatch(Action::OpenPath(path));
        }
        app
    }

    /// Apply an action and hand the work it calls for to the workers.
    fn dispatch(&mut self, action: Action) {
        self.session.dispatch(action);
    }

    /// Make the small picture of each series whose middle slice has arrived.
    fn make_thumbnails(&mut self, ctx: &egui::Context) {
        let Some(index) = &self.session.state().index else {
            self.thumbnails.clear();
            return;
        };
        for series in &index.series {
            let id = &series.info.id;
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
    fn play(&mut self, ctx: &egui::Context) {
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
    /// The series list down the left of the window, when it is shown, taking its share of
    /// the window's width, or less where the pictures need it: where it is.
    fn list(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) -> Option<egui::Rect> {
        if !self.screen.list_shown {
            return None;
        }
        let window = ui.ctx().content_rect().width();
        let width = self.screen.widths(window).rail;
        let column = egui::Panel::left("browser")
            .exact_size(width)
            .resizable(false);
        let shown = column.show(ui, |ui| {
            browser::show(ui, self.session.state(), &self.thumbnails, actions);
        });
        Some(shown.response.rect)
    }

    /// Let the right edge of the series list, at `list`, be dragged to give it more or less
    /// of the window's width. Made after everything beside it, so that the drag is the edge's.
    fn drag_the_list(&mut self, ui: &egui::Ui, list: Option<egui::Rect>) {
        let Some(list) = list else {
            return;
        };
        let edge = egui::Id::new("series list edge");
        let by = shares::divider(ui, edge, list.right(), list.y_range());
        if by.abs() > 0.0 {
            let window = ui.ctx().content_rect().width();
            let share = shares::list_dragged(window, self.screen.list_share(), by);
            self.screen.set_list_share(share);
        }
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

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        // A narrow window folds a side column before any is drawn.
        let window = ui.ctx().content_rect().width();
        self.screen.settle(window);
        let list_shown = self.screen.list_shown;
        let shown = bars::show(ui, self.session.state(), list_shown, &mut actions);
        let (asked, _disclaimer) = shown;
        for request in asked.requests() {
            self.ask(ui.ctx(), request);
        }
        if let Some(path) = toolbar::dropped(ui.ctx()) {
            actions.push(self.labels.opening(path));
        }
        let list = self.list(ui, &mut actions);
        let panel = self.tab_panel(ui, &mut actions);
        egui::CentralPanel::default()
            .frame(middle())
            .show(ui, |ui| {
                self.pictures(ui, frame.wgpu_render_state(), &mut actions);
            });
        self.drag_the_list(ui, list);
        self.drag_the_panel(ui, panel);
        about::show(ui.ctx(), &mut self.about_open);
        help::show(ui.ctx(), &mut self.help_open);
        self.tags.show(ui.ctx(), self.session.state(), &mut actions);
        self.keys(ui.ctx(), &mut actions);
        self.show_search(ui.ctx(), &mut actions);
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
        // A column opened this frame in a narrow window folds the other in the next.
        if self.screen.unsettled(window) {
            ui.ctx().request_repaint();
        }
    }
}

/// The middle of the window, where the pictures are: on the background, with the gap between
/// viewports around them.
fn middle() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::BACKGROUND)
        .inner_margin(viewport::GAP)
}

//! The window: toolbar, study browser, the 3D scene, the viewport grid and status line.

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Instant;

use egui::RichText;
use egui_wgpu::RenderState;

use crate::action::Action;
use crate::scene::{MainPanel, SceneStatus};
use crate::session::Session;
use crate::settings;
use crate::state::{NoticeKind, VIEWPORTS};

mod about;
mod browser;
mod capture;
mod cine;
mod help;
mod layout;
mod overlay;
mod pen;
mod saving;
mod scene;
mod scene_bar;
mod tags;
mod theme;
mod toolbar;
mod viewport;

pub use browser::thumbnail;

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
    thumbnails: browser::Thumbnails,
    about_open: bool,
    help_open: bool,
    tags: tags::Panel,
    /// When playback in each viewport last moved on.
    clocks: [cine::Clock; VIEWPORTS],
    capture: Option<capture::Capture>,
    /// What a picture of the window wants done to the 3D scene once its volume is ready.
    pose: Vec<Action>,
    /// A turntable of the 3D scene that is being drawn, a picture each frame.
    turntable: Option<scene::export::Turntable>,
}

impl VoxaApp {
    /// Set the window up, and open `initial` if a path was given.
    pub fn new(creation: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
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
        let gpus =
            render.map(|render| (0..VIEWPORTS).map(|_| viewport::Gpu::new(render)).collect());
        let mut app = Self {
            session: Session::new(from_workers, reports, CACHE_BYTES, settings::folder()),
            gpus: gpus.unwrap_or_default(),
            scene: scene::SceneView::new(render),
            thumbnails: browser::Thumbnails::default(),
            about_open: false,
            help_open: false,
            tags: tags::Panel::default(),
            clocks: [cine::Clock::default(); VIEWPORTS],
            capture: capture::Capture::from_environment(),
            pose: Vec::new(),
            turntable: None,
        };
        if let Some(render) = render {
            // Volumes are made ready on the workers to fit this device.
            let longest = render.device.limits().max_texture_dimension_3d;
            app.session.workers().set_volume_side(longest);
        }
        if app.capture.is_some() {
            // A picture for the documentation may want one of the windows open in it.
            let wanted = std::env::var(capture::SHOW_VARIABLE).unwrap_or_default();
            app.tags.open = wanted.contains("tags");
            app.help_open = wanted.contains("keys");
            app.about_open = wanted.contains("about");
            app.pose = capture::pose(&wanted);
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

    /// The line along the bottom: the disclaimer, and the newest notice with a way to
    /// dismiss it.
    fn status(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(theme::DISCLAIMER)
                    .small()
                    .color(theme::QUIET_TEXT),
            );
            let Some((at, notice)) = self.session.state().notices.iter().enumerate().next_back()
            else {
                return;
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Dismiss").clicked() {
                    actions.push(Action::DismissNotice(at));
                }
                let colour = match notice.kind {
                    NoticeKind::Info => theme::QUIET_TEXT,
                    NoticeKind::Warning | NoticeKind::Error => theme::WARNING,
                };
                let shown = ui.label(RichText::new(&notice.text).small().color(colour));
                if !notice.details.is_empty() {
                    shown.on_hover_text(notice.details.join("\n"));
                }
            });
        });
    }
}

impl VoxaApp {
    /// Draw the 3D scene and the 2D viewports in what is left of the window: one has the
    /// large part, the other a column beside it under a strip with a button that swaps them.
    fn pictures(
        &mut self,
        ui: &mut egui::Ui,
        render: Option<&RenderState>,
        actions: &mut Vec<Action>,
    ) {
        let state = self.session.state();
        let whole = ui.available_rect_before_wrap();
        let layout::Panels { main, side } = layout::panels(whole);
        let (strip, side) = layout::header_and_rest(side);
        // A series with no 3D view has its slices in the large part: they are all there is.
        let large = state.main.given(&state.scene.status);
        let there_is_3d = !matches!(state.scene.status, SceneStatus::Unavailable(_));
        layout::header(ui, strip, large, there_is_3d, actions);
        let (for_scene, for_slices) = match large {
            MainPanel::Scene | MainPanel::Planes => (main, side),
            MainPanel::Stack => (side, main),
        };
        let shown = state.scene.series.as_ref();
        let volume = shown.and_then(|series| self.session.volume(series));
        // If it has still to go on the graphics device, take what a worker made ready of it.
        let waiting = volume
            .as_ref()
            .is_some_and(|volume| self.scene.needs(volume));
        let ready = shown.filter(|_| waiting);
        self.scene
            .offer(ready.and_then(|series| self.session.take_prepared(series)));
        let scene = &mut self.scene;
        scene::show(
            ui,
            for_scene,
            state,
            volume.as_ref(),
            scene,
            render,
            actions,
        );
        let (session, gpus) = (&self.session, &mut self.gpus);
        let frames = |series: &_, slice| session.cached(series, slice);
        viewport::show(ui, for_slices, state, &frames, gpus, render, actions);
        ui.allocate_rect(whole, egui::Sense::hover());
    }

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

    /// Turn the keys pressed this frame into actions, unless the person is typing in a field.
    fn keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed: Vec<(egui::Key, bool)> = ctx.input(|input| {
            let down = |event: &egui::Event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some((*key, modifiers.shift)),
                _ => None,
            };
            input.events.iter().filter_map(down).collect()
        });
        for (key, shift) in pressed {
            // A question mark is its own key on some keyboards and Shift with slash on others.
            if key == egui::Key::Questionmark || (key == egui::Key::Slash && shift) {
                self.help_open = !self.help_open;
            } else if let Some(action) = overlay::shortcut_action(key, self.session.state().active)
            {
                actions.push(action);
            }
        }
    }
}

impl eframe::App for VoxaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.session.pump();
        self.play(ctx);
        self.make_thumbnails(ctx);
        self.tags
            .refresh(self.session.state(), self.session.workers(), ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        let bar = egui::Panel::top("toolbar");
        let (asked, saving) = bar
            .show(ui, |ui| {
                let asked = toolbar::show(ui, self.session.state(), &mut actions);
                (
                    asked,
                    scene_bar::show(ui, self.session.state(), &mut actions),
                )
            })
            .inner;
        self.about_open |= asked.about;
        self.help_open |= asked.shortcuts;
        self.tags.open ^= asked.tags;
        if let Some(path) = chosen(ui.ctx(), asked.open) {
            actions.push(Action::OpenPath(path));
        }
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui, &mut actions));
        let column = egui::Panel::left("browser")
            .exact_size(browser::WIDTH)
            .resizable(false);
        column.show(ui, |ui| {
            browser::show(ui, self.session.state(), &self.thumbnails, &mut actions);
        });
        let behind = egui::Frame::NONE
            .fill(theme::BACKGROUND)
            .inner_margin(viewport::GAP);
        egui::CentralPanel::default().frame(behind).show(ui, |ui| {
            self.pictures(ui, frame.wgpu_render_state(), &mut actions);
        });
        about::show(ui.ctx(), &mut self.about_open);
        help::show(ui.ctx(), &mut self.help_open);
        self.tags.show(ui.ctx(), self.session.state(), &mut actions);
        self.keys(ui.ctx(), &mut actions);
        self.save_scene(ui.ctx(), frame.wgpu_render_state(), saving, &mut actions);
        for action in actions {
            self.dispatch(action);
        }
        if self.session.state().scene.status == SceneStatus::Ready {
            for action in std::mem::take(&mut self.pose) {
                self.dispatch(action);
            }
        }
        // A picture of the window waits for the slice and, if there is to be one, the volume.
        let no_volume_to_come = !matches!(
            self.session.state().scene.status,
            SceneStatus::Building { .. } | SceneStatus::Ready
        );
        let ready = self.gpus.first().is_some_and(viewport::Gpu::has_picture)
            && (no_volume_to_come || self.scene.has_picture());
        if let Some(capture) = &mut self.capture {
            let pick = capture.pick(self.session.state(), |id| self.thumbnails.contains_key(id));
            capture.step(ui.ctx(), ready && pick.is_none());
            if let Some(pick) = pick {
                self.session.dispatch(pick);
            }
        }
    }
}

/// What the person has just asked to open, if anything: a file or folder dropped on the
/// window, or one chosen in the system's dialog after pressing an Open button. While
/// something is being dragged over the window, say that it can be dropped.
fn chosen(ctx: &egui::Context, asked: Option<toolbar::Pick>) -> Option<PathBuf> {
    let (hovering, dropped) = ctx.input(|input| {
        let dropped = input
            .raw
            .dropped_files
            .first()
            .map(|file| file.path().to_owned());
        (!input.raw.hovered_files.is_empty(), dropped)
    });
    if hovering {
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop to open"));
        let painter = ctx.layer_painter(layer);
        let screen = ctx.content_rect();
        painter.rect_filled(screen, 0, egui::Color32::from_black_alpha(160));
        let font = egui::FontId::proportional(24.0);
        let middle = egui::Align2::CENTER_CENTER;
        painter.text(
            screen.center(),
            middle,
            "Drop to open",
            font,
            egui::Color32::WHITE,
        );
    }
    dropped.or_else(|| match asked? {
        toolbar::Pick::Folder => rfd::FileDialog::new()
            .set_title("Open a folder of DICOM files")
            .pick_folder(),
        toolbar::Pick::File => rfd::FileDialog::new()
            .set_title("Open a zip file or a DICOMDIR")
            .pick_file(),
    })
}

/// Open the Voxa window and run until it is closed. `initial` is a scan to open at once.
///
/// # Errors
///
/// When the window or its graphics device cannot be created.
pub fn run(initial: Option<PathBuf>) -> eframe::Result {
    // A picture for the documentation may want the window a particular size.
    let asked = std::env::var(capture::SIZE_VARIABLE).ok();
    let size = asked.and_then(|size| capture::size_from(&size));
    let viewport = egui::ViewportBuilder::default()
        .with_title("Voxa")
        .with_inner_size(size.unwrap_or([1440.0, 900.0]))
        .with_min_inner_size([900.0, 600.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let create = move |creation: &eframe::CreationContext<'_>| -> Result<Box<dyn eframe::App>, _> {
        Ok(Box::new(VoxaApp::new(creation, initial)))
    };
    eframe::run_native("Voxa", options, Box::new(create))
}

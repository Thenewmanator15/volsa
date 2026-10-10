//! The window: toolbar, study browser, the 3D scene, the viewport grid and status line.

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Instant;

use egui::RichText;

use crate::action::Action;
use crate::session::Session;
use crate::settings;
use crate::state::{NoticeKind, VIEWPORTS};

mod about;
mod browser;
mod capture;
mod cine;
mod help;
mod keys;
mod labels;
mod layout;
mod measure;
mod overlay;
mod parts;
mod pen;
mod pictures;
mod planes;
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
    planes: planes::PlanesView,
    thumbnails: browser::Thumbnails,
    about_open: bool,
    help_open: bool,
    tags: tags::Panel,
    measure: measure::Panel,
    parts: parts::PartsView,
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
        let gpus =
            render.map(|render| (0..VIEWPORTS).map(|_| viewport::Gpu::new(render)).collect());
        let capture = capture::Capture::from_environment();
        let kept = capture::kept_between_runs(capture.is_some(), settings::folder());
        let mut app = Self {
            session: Session::new(from_workers, reports, CACHE_BYTES, kept),
            gpus: gpus.unwrap_or_default(),
            scene: scene::SceneView::new(render),
            planes: planes::PlanesView::new(render),
            thumbnails: browser::Thumbnails::default(),
            about_open: false,
            help_open: false,
            tags: tags::Panel::default(),
            measure: measure::Panel::default(),
            parts: parts::PartsView::default(),
            labels: labels::Opening::waiting_for(labels),
            held: labels::Held::default(),
            clocks: [cine::Clock::default(); VIEWPORTS],
            capture,
            pose: Vec::new(),
            label_pose: Vec::new(),
            turntable: None,
        };
        if let Some(render) = render {
            app.tell_the_workers_of(render);
        }
        if app.capture.is_some() {
            // A picture is of things as they come to rest, not part-way through fading in.
            creation
                .egui_ctx
                .all_styles_mut(|style| style.animation_time = 0.0);
            // A picture for the documentation may want one of the windows open in it.
            let wanted = std::env::var(capture::SHOW_VARIABLE).unwrap_or_default();
            app.tags.open = wanted.contains("tags");
            app.measure.open = wanted.contains("list");
            app.help_open = wanted.contains("keys");
            app.about_open = wanted.contains("about");
            app.pose = capture::pose(&wanted);
            app.label_pose = capture::label_pose(&wanted);
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

impl eframe::App for VoxaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.session.pump();
        self.open_waiting_labels();
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
        self.measure.open ^= asked.measure;
        if let Some(path) = chosen(ui.ctx(), asked.open) {
            actions.push(self.labels.opening(path));
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
        let state = self.session.state();
        let save = self.measure.show(ui.ctx(), state, &mut actions);
        let saved = save.and_then(|kind| measure::save(kind, state));
        actions.extend(saved.map(Action::Post));
        self.keys(ui.ctx(), &mut actions);
        self.save_scene(ui.ctx(), frame.wgpu_render_state(), saving, &mut actions);
        for action in actions {
            self.dispatch(action);
        }
        self.for_a_picture(ui.ctx());
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

/// Open the Voxa window and run until it is closed. `initial` is a scan to open at once,
/// and `labels` a label file to open on it as soon as it can be shown in 3D.
///
/// # Errors
///
/// When the window or its graphics device cannot be created.
pub fn run(initial: Option<PathBuf>, labels: Option<PathBuf>) -> eframe::Result {
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
        Ok(Box::new(VoxaApp::new(creation, initial, labels)))
    };
    eframe::run_native("Voxa", options, Box::new(create))
}

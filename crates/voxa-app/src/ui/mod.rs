//! The window: toolbar, study browser, viewport grid and status line.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};

use egui::RichText;

use crate::action::Action;
use crate::cache::FrameCache;
use crate::reducer::reduce;
use crate::state::{AppState, NoticeKind, VIEWPORTS};
use crate::worker::Workers;

mod about;
mod browser;
mod capture;
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
    state: AppState,
    workers: Workers,
    reports: Receiver<Action>,
    cache: Arc<Mutex<FrameCache>>,
    /// One for each viewport; empty when the window has no graphics device to draw slices.
    gpus: Vec<viewport::Gpu>,
    thumbnails: browser::Thumbnails,
    about_open: bool,
    capture: Option<capture::Capture>,
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
        let cache = Arc::new(Mutex::new(FrameCache::new(CACHE_BYTES)));
        let render = creation.wgpu_render_state.as_ref();
        let gpus =
            render.map(|render| (0..VIEWPORTS).map(|_| viewport::Gpu::new(render)).collect());
        let mut app = Self {
            state: AppState::default(),
            workers: Workers::new(from_workers, Arc::clone(&cache)),
            reports,
            cache,
            gpus: gpus.unwrap_or_default(),
            thumbnails: browser::Thumbnails::default(),
            about_open: false,
            capture: capture::Capture::from_environment(),
        };
        if let Some(path) = initial {
            app.dispatch(Action::OpenPath(path));
        }
        app
    }

    /// Apply an action and hand the work it calls for to the workers.
    fn dispatch(&mut self, action: Action) {
        for effect in reduce(&mut self.state, action) {
            self.workers.run(effect, self.state.index.as_ref());
        }
    }

    /// Make the small picture of each series whose middle slice has arrived.
    fn make_thumbnails(&mut self, ctx: &egui::Context) {
        let Some(index) = &self.state.index else {
            self.thumbnails.clear();
            return;
        };
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        for series in &index.series {
            let id = &series.info.id;
            if self.thumbnails.contains_key(id) {
                continue;
            }
            let middle = series.slices.len() / 2;
            let window = self.state.windows.get(&(id.clone(), middle));
            let (Some(frame), Some([automatic, ..])) = (cache.get(id, middle), window) else {
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
            let Some((at, notice)) = self.state.notices.iter().enumerate().next_back() else {
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

impl eframe::App for VoxaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(report) = self.reports.try_recv() {
            self.dispatch(report);
        }
        self.make_thumbnails(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        let bar = egui::Panel::top("toolbar");
        let asked = bar
            .show(ui, |ui| toolbar::show(ui, &self.state, &mut actions))
            .inner;
        self.about_open |= asked.about;
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui, &mut actions));
        let column = egui::Panel::left("browser")
            .exact_size(browser::WIDTH)
            .resizable(false);
        column.show(ui, |ui| {
            browser::show(ui, &self.state, &self.thumbnails, &mut actions);
        });
        let behind = egui::Frame::NONE
            .fill(theme::BACKGROUND)
            .inner_margin(viewport::GAP);
        egui::CentralPanel::default().frame(behind).show(ui, |ui| {
            let render = frame.wgpu_render_state();
            viewport::show(
                ui,
                &self.state,
                &self.cache,
                &mut self.gpus,
                render,
                &mut actions,
            );
        });
        about::show(ui.ctx(), &mut self.about_open);
        for action in actions {
            self.dispatch(action);
        }
        let ready = self.gpus.first().is_some_and(viewport::Gpu::has_picture);
        if let Some(capture) = &mut self.capture {
            capture.step(ui.ctx(), ready);
        }
    }
}

/// Open the Voxa window and run until it is closed. `initial` is a scan to open at once.
///
/// # Errors
///
/// When the window or its graphics device cannot be created.
pub fn run(initial: Option<PathBuf>) -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Voxa")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 500.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let create = move |creation: &eframe::CreationContext<'_>| -> Result<Box<dyn eframe::App>, _> {
        Ok(Box::new(VoxaApp::new(creation, initial)))
    };
    eframe::run_native("Voxa", options, Box::new(create))
}

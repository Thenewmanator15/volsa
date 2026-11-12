//! Setting the window up: what it tells the workers of its graphics device, how it is made, and
//! the thread that carries the workers' reports to it.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use super::{
    VolsaApp, bars, browser, capture, cine, document, labels, lost, planes, rail, remembered,
    scene, screen, search, tabs, tags, theme, viewport,
};
use crate::action::Action;
use crate::labelling::LabellingAction;
use crate::plugins::PluginAction;
use crate::session::Session;
use crate::state::VIEWPORTS;

impl VolsaApp {
    /// Tell the workers what the window's graphics device is like. They are never given the
    /// device itself: only this thread hands it work, so that no other does while the
    /// window sets itself up.
    fn tell_the_workers_of(&self, render: &egui_wgpu::RenderState) {
        let workers = self.session.workers();
        // A volume this device cannot hold whole is halved for it on a worker.
        workers.set_card_room(self.scene.card_room());
        // What this device works out itself is not worked out on a worker first.
        let pipeline = self.scene.volume_pipeline();
        let curves = pipeline.is_some_and(|pipeline| pipeline.makes_curves(&render.device));
        workers.leave_to_the_device(curves);
    }

    /// Tell labelling the name of the graphics card the window draws on, if it draws on one,
    /// and look on a worker at whether this computer can label a scan, and at the plugins the
    /// person added. A picture that asks for made-up plugins is given those, and makes no look.
    fn look_for_labelling(&mut self, render: Option<&egui_wgpu::RenderState>) {
        let plugins = capture::made_up_plugins().unwrap_or(PluginAction::Look);
        self.dispatch(Action::Plugins(plugins));
        if let Some(render) = render {
            // A run on the card Volsa draws on leaves room for what Volsa holds there.
            let drawing_on = render.adapter.get_info().name;
            self.dispatch(Action::Labelling(LabellingAction::DrawingOn(drawing_on)));
        }
        // Whether this computer can label a scan is looked at once as Volsa starts, on a
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
        let (from_workers, reports) = relay(creation.egui_ctx.clone());
        let render = creation.wgpu_render_state.as_ref();
        crate::log::device(render);
        let gpus =
            render.map(|render| (0..VIEWPORTS).map(|_| viewport::Gpu::new(render)).collect());
        let capture = capture::Capture::from_environment();
        let (settings, local) = capture::folders(capture.is_some());
        let prefs = crate::prefs::Kept::new(settings);
        let arranged = prefs.remembered().arrangement();
        let set = crate::budget::asked().or(prefs.remembered().memory_mib);
        let budget = crate::budget::bytes_for(set, volsa_models::total_memory());
        let choice = remembered::opening_theme(&arranged, capture.is_some());
        theme::apply(&creation.egui_ctx, choice);
        creation.egui_ctx.set_zoom_factor(arranged.interface_size);
        // A picture is of a window the size it asks for, not one fitted to this screen,
        // unless it asks to see the fitting.
        let fitted = capture.is_none() || capture::wants("fitted");
        let hidden = capture.is_some() && capture::wants("nolist");
        let mut app = Self {
            screen: screen::Screen::new(arranged, fitted, hidden),
            kept: prefs,
            session: Session::new(from_workers, reports, budget, local),
            gpus: gpus.unwrap_or_default(),
            drafts: viewport::Drafts::default(),
            scene: scene::SceneView::new(render),
            planes: planes::PlanesView::new(render),
            thumbnails: browser::Thumbnails::default(),
            rail: rail::RailState::default(),
            about_open: false,
            help_open: false,
            confirming: None,
            tags: tags::Panel::default(),
            bars: bars::Kept {
                theme: choice,
                ..bars::Kept::default()
            },
            document_view: document::Kept::default(),
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
}

/// Where the workers report, and where the window reads their reports. Workers report on their
/// own channel; a small thread passes each report on and wakes the window, so the workers need
/// know nothing about the interface.
fn relay<T: Send + 'static>(ctx: egui::Context) -> (Sender<T>, Receiver<T>) {
    let (from_workers, to_relay) = channel();
    let (to_window, reports) = channel();
    std::thread::spawn(move || {
        for report in to_relay {
            if to_window.send(report).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    });
    (from_workers, reports)
}

//! Opening the window: the size it opens at, and what a picture of it wants in it.

use std::path::PathBuf;

use super::{VoxaApp, capture, icon};
use crate::prefs::SMALLEST_WINDOW;

/// The size the window opens at, in points at the interface's usual size, when no size is
/// remembered: small enough for most screens, until it is fitted to the one it is on.
const OPENS_AT: [f32; 2] = [1024.0, 640.0];
/// The size of a picture's window when the picture asks for none, in points of the
/// interface.
const PICTURE: [f32; 2] = [1440.0, 900.0];

impl VoxaApp {
    /// Make ready what a picture of the window wants: the interface at the size it asks for,
    /// everything at rest rather than fading in, and the windows it wants open in it.
    pub(super) fn set_up_a_picture(&mut self, ctx: &egui::Context) {
        ctx.set_zoom_factor(capture::zoom().unwrap_or(1.0));
        // A picture is of things as they come to rest, not part-way through fading in.
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        // A picture for the documentation may want one of the windows open in it.
        let wanted = std::env::var(capture::SHOW_VARIABLE).unwrap_or_default();
        let asked = |word: &str| capture::asks_for(&wanted, word);
        self.tags.open = asked("tags");
        self.measure.open = asked("list");
        self.help_open = asked("keys");
        self.about_open = asked("about");
        self.pose = capture::pose(&wanted);
        self.label_pose = capture::label_pose(&wanted);
    }
}

/// The size the window opens at, in points at the interface's usual size: for a picture, the
/// size it asks for at the interface size it asks for; otherwise the size remembered, or one
/// small enough for most screens. Either way, never smaller than the smallest window.
fn opening_size(capturing: bool) -> [f32; 2] {
    let size = if capturing {
        let asked = std::env::var(capture::SIZE_VARIABLE).ok();
        let asked = asked.and_then(|size| capture::size_from(&size));
        capture::window_for(asked.unwrap_or(PICTURE), capture::zoom().unwrap_or(1.0))
    } else {
        let (settings, _) = capture::folders(false);
        let remembered = crate::prefs::Kept::new(settings).remembered().window;
        remembered.unwrap_or(OPENS_AT)
    };
    let ([wide, high], [least_w, least_h]) = (size, SMALLEST_WINDOW);
    [wide.max(least_w), high.max(least_h)]
}

/// Open the Voxa window and run until it is closed. `initial` is a scan to open at once,
/// and `labels` a label file to open on it as soon as it can be shown in 3D.
///
/// # Errors
///
/// When the window or its graphics device cannot be created.
pub fn run(initial: Option<PathBuf>, labels: Option<PathBuf>) -> eframe::Result {
    let capturing = capture::Capture::from_environment().is_some();
    let (settings, local) = capture::folders(capturing);
    crate::log::start(local.as_deref());
    crate::moved::into_local(settings.as_deref(), local.as_deref());
    // The screen the window is on is not known until the window is there: it opens at this
    // size and is fitted to its screen on its first frame.
    let viewport = egui::ViewportBuilder::default()
        .with_title("Voxa")
        .with_icon(icon::icon())
        .with_inner_size(opening_size(capturing))
        .with_min_inner_size(SMALLEST_WINDOW);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let create = move |creation: &eframe::CreationContext<'_>| -> Result<Box<dyn eframe::App>, _> {
        Ok(Box::new(VoxaApp::new(creation, initial, labels)))
    };
    eframe::run_native("Voxa", options, Box::new(create))
}

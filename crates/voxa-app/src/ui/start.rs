//! Opening the window: the size it opens at, and what a picture of it wants in it.

use std::path::PathBuf;

use egui_wgpu::wgpu::Backends;

use super::{VoxaApp, capture, icon};
use crate::prefs::SMALLEST_WINDOW;

#[cfg(test)]
mod tests;

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

/// The graphics backends the window may start on: those named from outside (`WGPU_BACKEND`,
/// which wgpu reads), when any are; otherwise, on Windows, Vulkan and DirectX 12, and
/// elsewhere eframe's own choice. Every backend tried at start-up loads its drivers, and on
/// Windows OpenGL was only ever tried, never drawn on: without it Voxa holds about 12 MiB less
/// of its own memory and four threads fewer. DirectX 12 stays for a computer without Vulkan,
/// and for one with no card to use, where it draws in software (WARP).
fn window_backends(named: Option<Backends>) -> Backends {
    let own = if cfg!(windows) {
        Backends::VULKAN | Backends::DX12
    } else {
        Backends::PRIMARY | Backends::GL
    };
    named.unwrap_or(own)
}

/// eframe's options, with the window to start on these graphics backends and all else as it
/// was.
fn with_backends(mut options: eframe::NativeOptions, backends: Backends) -> eframe::NativeOptions {
    if let egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup {
        create.instance_descriptor.backends = backends;
    }
    options
}

/// Open the Voxa window and run until it is closed. `initial` is a scan to open at once,
/// and `labels` a label file to open on it as soon as it can be shown in 3D.
///
/// If the window's code panics, the window is gone by the time the panic reaches here: the
/// person is told that Voxa stopped and where its log is, in a message window of the system
/// for a build with no console, and the panic then goes on to end the program. A picture of
/// the window tells no one.
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
    let options = with_backends(options, window_backends(Backends::from_env()));
    let create = move |creation: &eframe::CreationContext<'_>| -> Result<Box<dyn eframe::App>, _> {
        Ok(Box::new(VoxaApp::new(creation, initial, labels)))
    };
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native("Voxa", options, Box::new(create))
    }));
    ran.unwrap_or_else(|panic| {
        if let Some(said) = crate::said::after_a_crash(capturing, crate::log::path()) {
            crate::said::tell(&said, crate::said::WINDOWED);
        }
        std::panic::resume_unwind(panic)
    })
}

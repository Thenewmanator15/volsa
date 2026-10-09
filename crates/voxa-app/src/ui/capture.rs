//! Saving a picture of the window to a file, for checking by eye and for the documentation.
//!
//! Set the environment variable `VOXA_CAPTURE` to a `.png` path. Voxa then saves one picture
//! of its own window, once the first viewport has something to show or after a wait, and
//! closes. Only Voxa's window is in the picture, never the rest of the screen. With
//! `VOXA_CAPTURE_SIZE` set as well, to a size such as `900x600`, the window is that size.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use voxa_render::Look;

use crate::action::Action;

/// The environment variable naming the file to save.
pub const VARIABLE: &str = "VOXA_CAPTURE";
/// The environment variable naming what to have in the picture, separated by commas: the
/// windows `tags`, `keys` and `about`, and for the 3D scene `vr` or `mip`, `turned`, `cut`
/// and `slice`.
pub const SHOW_VARIABLE: &str = "VOXA_CAPTURE_SHOW";
/// The environment variable giving the size of the window for the picture, in points, as
/// width `x` height: `900x600`.
pub const SIZE_VARIABLE: &str = "VOXA_CAPTURE_SIZE";
/// Frames drawn with a picture showing before it is captured, so everything has settled.
const SETTLE_FRAMES: u32 = 30;
/// How long to wait for a picture before capturing whatever is there.
const PATIENCE: Duration = Duration::from_secs(20);

/// A window size written as width `x` height. `None` unless both are numbers above zero.
pub fn size_from(text: &str) -> Option<[f32; 2]> {
    let (width, height) = text.split_once('x')?;
    let side = |text: &str| text.trim().parse::<f32>().ok().filter(|side| *side > 0.0);
    Some([side(width)?, side(height)?])
}

/// What to do to the 3D scene, once its volume is ready, for a picture that asks for it by
/// the words in `wanted`: a look, a turn, a cut, the slice drawn in it.
pub fn pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| wanted.split(',').any(|part| part.trim() == word);
    let poses = [
        ("vr", Action::SceneLook(Look::Vr)),
        ("mip", Action::SceneLook(Look::Mip)),
        ("turned", Action::SceneOrbit { by: [70.0, 35.0] }),
        ("cut", Action::CutAdd),
        ("slice", Action::ShowSliceInScene(true)),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
}

/// A capture that has been asked for and not yet finished.
pub struct Capture {
    path: PathBuf,
    started: Instant,
    settled: u32,
    asked: bool,
}

impl Capture {
    /// A capture, if the environment asks for one.
    pub fn from_environment() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os(VARIABLE)?);
        Some(Self {
            path,
            started: Instant::now(),
            settled: 0,
            asked: false,
        })
    }

    /// Call once a frame. `ready` says whether the window shows what is wanted.
    pub fn step(&mut self, ctx: &egui::Context, ready: bool) {
        let taken = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(picture) = taken {
            let [width, height] = picture
                .size
                .map(|side| u32::try_from(side).unwrap_or(u32::MAX));
            let colour = image::ExtendedColorType::Rgba8;
            // Nothing can be said to anyone if saving fails: the window is about to close.
            let _ = image::save_buffer(&self.path, picture.as_raw(), width, height, colour);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ready {
            self.settled += 1;
        }
        let due = self.settled >= SETTLE_FRAMES || self.started.elapsed() >= PATIENCE;
        if due && !self.asked {
            self.asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_can_ask_for_the_scene_to_be_posed() {
        use voxa_render::Look;
        assert_eq!(pose("tags,keys"), []);
        assert_eq!(pose("cut"), [Action::CutAdd]);
        let all = pose("about, mip, turned, cut, slice");
        let expected = [
            Action::SceneLook(Look::Mip),
            Action::SceneOrbit { by: [70.0, 35.0] },
            Action::CutAdd,
            Action::ShowSliceInScene(true),
        ];
        assert_eq!(
            all, expected,
            "in an order that does not depend on how they were asked"
        );
        assert_eq!(pose("vr"), [Action::SceneLook(Look::Vr)]);
    }

    #[test]
    fn a_window_size_is_read_as_width_by_height() {
        assert_eq!(size_from("900x600"), Some([900.0, 600.0]));
        assert_eq!(size_from(" 1440 x 900 "), Some([1440.0, 900.0]));
        assert_eq!(size_from("900"), None);
        assert_eq!(size_from("0x600"), None, "a window has to have a size");
        assert_eq!(size_from("wide x tall"), None);
    }
}

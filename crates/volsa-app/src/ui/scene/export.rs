//! Saving the 3D scene as a picture or as a turntable that goes once round the patient. What
//! is saved is the scene alone: no letters, captions, handles or words.

use std::path::{Path, PathBuf};

use egui::Ui;
use volsa_render::{SceneParams, turntable};

use crate::export::{
    ExportError, SUGGESTED_NAME, TURNTABLE_FRAMES, TURNTABLE_SECONDS, gif, png, turntable_size,
    write,
};
use crate::state::{AppState, Notice, NoticeKind};
use crate::ui::commands::{self, Does, Request};
use crate::worker::Workers;

mod progress;

pub use progress::{is_open as turntable_is_open, progress, stop as stop_turntable};

/// The key of the notice that says how saving went.
const SAVING: &str = "saving";

/// What the person asked to save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// The scene as it is, as one picture.
    Picture,
    /// The scene going once round the patient, as an animation.
    Turntable,
}

/// What the Export menu holds: a button for each kind of file, each an action of the list of
/// actions. Greyed until there is a 3D view to save.
pub fn contents(ui: &mut Ui, state: &AppState) -> Option<Asked> {
    let picture = commands::button(ui, "scene.save.picture", state, None);
    let turning = commands::button(ui, "scene.save.turntable", state, None);
    match picture.or(turning)? {
        Does::Ask(Request::SaveScene(asked)) => Some(asked),
        _ => None,
    }
}

/// Ask the person where to save, offering a name that says nothing of the scan.
pub fn choose(asked: Asked) -> Option<PathBuf> {
    let (title, kind, ending) = match asked {
        Asked::Picture => ("Save the 3D view as a picture", "PNG picture", "png"),
        Asked::Turntable => ("Save a turntable of the 3D view", "Animated GIF", "gif"),
    };
    rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(format!("{SUGGESTED_NAME}.{ending}"))
        .add_filter(kind, &[ending])
        .save_file()
}

/// Save one picture of the scene, and say how it went.
pub fn save_picture(path: &Path, rgba: &[u8], size: [u32; 2]) -> Notice {
    said(path, png(rgba, size).and_then(|file| write(path, &file)))
}

/// What to tell the person once a file has been saved, or has not.
pub fn said(path: &Path, outcome: Result<(), ExportError>) -> Notice {
    match outcome {
        Ok(()) => about(NoticeKind::Info, format!("Saved {}", name_of(path))),
        Err(why) => about(NoticeKind::Error, why.to_string()),
    }
}

/// A notice about saving. Each takes the place of the one before.
fn about(kind: NoticeKind, text: String) -> Notice {
    Notice {
        kind,
        text,
        details: Vec::new(),
        key: Some(SAVING),
    }
}

/// The name of the file the person chose, without the folders it is in.
fn name_of(path: &Path) -> String {
    let name = path.file_name();
    name.map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A notice that a picture could not be drawn for saving.
pub fn not_drawn(why: &impl ToString) -> Notice {
    about(NoticeKind::Error, why.to_string())
}

/// Put a finished turntable's pictures together and write its file on a worker, so that the
/// window is not held up meanwhile. What to say until then is given back; the worker posts
/// how it went when it is done.
pub fn finish_on(workers: &Workers, making: Turntable) -> Notice {
    let (meanwhile, otherwise) = (making.saving(), making.not_saved());
    workers.post(otherwise, move || making.finish());
    meanwhile
}

/// A turntable being made. One picture is drawn for each frame the interface draws, so the
/// window stays alive and the person can stop it.
pub struct Turntable {
    path: PathBuf,
    /// The scene for each picture, in order.
    scenes: Vec<SceneParams>,
    /// The pictures drawn so far.
    drawn: Vec<Vec<u8>>,
    /// The size the scene is laid out at, and the size of each picture in pixels.
    viewport: [f32; 2],
    size: [u32; 2],
}

impl Turntable {
    /// A turntable of the scene as it is now, to be saved at `path`.
    pub fn new(path: PathBuf, scene: &SceneParams, viewport: [f32; 2]) -> Self {
        let cameras = turntable(&scene.camera, TURNTABLE_FRAMES);
        let full = |camera| SceneParams {
            camera,
            quality: 1.0,
            ..scene.clone()
        };
        Self {
            path,
            scenes: cameras.into_iter().map(full).collect(),
            drawn: Vec::new(),
            viewport,
            size: turntable_size(viewport),
        }
    }

    /// How many pictures have been drawn, and how many there will be.
    pub fn progress(&self) -> (usize, usize) {
        (self.drawn.len(), self.scenes.len())
    }

    /// The scene to draw next, the size it is laid out at and the picture's size in pixels;
    /// `None` when every picture has been drawn.
    pub fn next(&self) -> Option<(&SceneParams, [f32; 2], [u32; 2])> {
        let scene = self.scenes.get(self.drawn.len())?;
        Some((scene, self.viewport, self.size))
    }

    /// Take the picture drawn of the scene [`Turntable::next`] gave.
    pub fn take(&mut self, rgba: Vec<u8>) {
        self.drawn.push(rgba);
    }

    /// What to say while the pictures are put together and the file is written.
    pub fn saving(&self) -> Notice {
        about(NoticeKind::Info, format!("Saving {}…", name_of(&self.path)))
    }

    /// What to say if putting the pictures together stops before it can say how it went.
    pub fn not_saved(&self) -> Notice {
        let name = name_of(&self.path);
        let why = "putting its pictures together stopped part-way";
        about(
            NoticeKind::Error,
            format!("{name} could not be saved: {why}."),
        )
    }

    /// Put the pictures together, write the file, and say how it went.
    pub fn finish(self) -> Notice {
        let file = gif(&self.drawn, self.size, TURNTABLE_SECONDS);
        said(&self.path, file.and_then(|file| write(&self.path, &file)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use egui::{Event, Modifiers, PointerButton, RawInput, pos2};
    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;
    use volsa_core::{Point, WindowLevel};
    use volsa_render::{Camera, Look, NamedView};

    use super::*;

    fn scene() -> SceneParams {
        SceneParams {
            camera: Camera::looking(NamedView::Anterior, Point::new(0.0, 0.0, 0.0), 0.5),
            window: WindowLevel::new(200.0, 400.0),
            look: Look::Surface,
            cuts: Vec::new(),
            slice: None,
            quality: 0.5,
            skip_empty: true,
        }
    }

    /// Click down the menu and gather what was asked for, in the order met.
    fn sweep(there: bool) -> Vec<Asked> {
        let ctx = egui::Context::default();
        let mut state = AppState::default();
        if there {
            state.scene.status = crate::scene::SceneStatus::Ready;
        }
        let mut asked = Vec::new();
        let mut pass = |events: Vec<Event>| {
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| asked.extend(contents(ui, &state)));
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
        };
        pass(Vec::new());
        for y in (4..80u8).step_by(4) {
            let pos = pos2(30.0, f32::from(y));
            let button = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            pass(vec![Event::PointerMoved(pos), button(true)]);
            pass(vec![button(false)]);
        }
        asked.dedup();
        asked
    }

    #[test]
    fn the_menu_offers_a_picture_and_a_turntable_once_there_is_a_3d_view() {
        assert_eq!(sweep(true), [Asked::Picture, Asked::Turntable]);
        assert_eq!(sweep(false), []);
    }

    #[test]
    fn a_picture_is_saved_and_the_notice_names_the_file_and_nothing_else() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join("volsa-3d.png");
        let notice = save_picture(&path, &[9; 3 * 2 * 4], [3, 2]);
        assert_eq!(
            (notice.kind, notice.text.as_str()),
            (NoticeKind::Info, "Saved volsa-3d.png")
        );
        let read = image::load_from_memory(&std::fs::read(&path).expect("reads")).expect("a png");
        assert_eq!((read.width(), read.height()), (3, 2));
        let failed = save_picture(
            &folder.path().join("no folder").join("x.png"),
            &[9; 24],
            [3, 2],
        );
        assert_eq!(failed.kind, NoticeKind::Error);
        assert!(
            failed.text.starts_with("The file could not be written"),
            "{}",
            failed.text
        );
    }

    #[test]
    fn a_turntable_draws_seventy_two_scenes_at_full_quality_and_saves_them_as_a_gif() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join("volsa-3d.gif");
        let mut making = Turntable::new(path.clone(), &scene(), [200.0, 100.0]);
        assert_eq!(making.progress(), (0, 72));
        let mut yaws = Vec::new();
        while let Some((scene, viewport, size)) = making.next() {
            assert_eq!((viewport, size), ([200.0, 100.0], [200, 100]));
            assert_eq!((scene.quality, scene.look), (1.0, Look::Surface));
            yaws.push(scene.camera.yaw);
            let grey = u8::try_from(yaws.len()).expect("72 at most");
            making.take([grey, grey, grey, 255].repeat(200 * 100));
        }
        assert_eq!((yaws.len(), making.progress()), (72, (72, 72)));
        assert_eq!(yaws[0], scene().camera.yaw, "it starts as the scene is");
        yaws.dedup();
        assert_eq!(yaws.len(), 72, "and every picture is from a different side");
        let notice = making.finish();
        assert_eq!(notice.text, "Saved volsa-3d.gif");
        let file = Cursor::new(std::fs::read(&path).expect("reads"));
        let frames = GifDecoder::new(file).expect("a gif").into_frames().count();
        assert_eq!(frames, 72);
    }

    #[test]
    fn a_turntable_with_no_pictures_drawn_says_so_and_writes_nothing() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join("volsa-3d.gif");
        let notice = Turntable::new(path.clone(), &scene(), [400.0, 300.0]).finish();
        assert_eq!(notice.kind, NoticeKind::Error);
        assert_eq!(notice.text, "There is no picture of that size to save.");
        assert!(!path.exists());
    }

    /// A turntable of small pictures with every one drawn.
    fn drawn(path: PathBuf) -> Turntable {
        let mut making = Turntable::new(path, &scene(), [20.0, 10.0]);
        while let Some((_, _, [wide, high])) = making.next() {
            making.take(vec![200; (wide * high * 4) as usize]);
        }
        making
    }

    #[test]
    fn what_is_said_while_a_turntable_is_put_together_gives_way_to_how_it_went() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let making = drawn(folder.path().join("volsa-3d.gif"));
        let (saving, stopped) = (making.saving(), making.not_saved());
        assert_eq!(
            (saving.kind, saving.text.as_str()),
            (NoticeKind::Info, "Saving volsa-3d.gif…")
        );
        let said =
            "volsa-3d.gif could not be saved: putting its pictures together stopped part-way.";
        assert_eq!(
            (stopped.kind, stopped.text.as_str()),
            (NoticeKind::Error, said)
        );
        let done = making.finish();
        assert!(saving.key.is_some(), "so that the next takes its place");
        assert_eq!((saving.key, stopped.key), (done.key, done.key));
    }

    #[test]
    fn a_finished_turntable_is_put_together_on_a_worker_which_says_when_it_is_saved() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join("volsa-3d.gif");
        let (sender, reports) = std::sync::mpsc::channel();
        let mut session = crate::session::Session::new(sender, reports, 1 << 20, None);
        let meanwhile = finish_on(session.workers(), drawn(path.clone()));
        assert_eq!(meanwhile.text, "Saving volsa-3d.gif…");
        let said = |session: &crate::session::Session| {
            let notices = &session.state().notices;
            notices
                .iter()
                .any(|notice| notice.text == "Saved volsa-3d.gif")
        };
        while !said(&session) {
            let heard = session.wait(std::time::Duration::from_secs(60));
            assert!(heard > 0, "the worker says how it went");
        }
        let file = Cursor::new(std::fs::read(&path).expect("reads"));
        let frames = GifDecoder::new(file).expect("a gif").into_frames().count();
        assert_eq!(frames, 72);
    }
}

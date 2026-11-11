//! Views of the 3D scene that a person has saved: where it was seen from, in which look,
//! through which window, and how it was cut open. A saved view holds numbers and a name the
//! person typed; nothing from the scan's files.

use serde::{Deserialize, Serialize};
use voxa_core::{Point, SeriesId, WindowLevel};
use voxa_render::{Camera, CutPlane, Look};

use crate::scene::Scene;

/// The most views kept for one series. Saving another drops the oldest.
pub const MAX_VIEWS: usize = 20;
/// The most characters a view's name may have.
const MAX_NAME: usize = 40;

/// Everything that can happen to the saved views of the scene's series: what the person asks
/// for, and what a worker reports of the file they are kept in. It is carried by
/// [`Action::Views`](crate::Action::Views).
#[derive(Debug, Clone, PartialEq)]
pub enum ViewsAction {
    /// Save the scene as it is now, under a name; with no name it is numbered.
    Save {
        /// What to call it.
        name: String,
    },
    /// Put the scene back as a saved view has it, by the view's place in the list.
    Apply(usize),
    /// Forget a saved view, by its place in the list.
    Delete(usize),
    /// The views kept for a series from before have been read.
    Loaded {
        /// The series they are of.
        series: SeriesId,
        /// The views, oldest first.
        views: Vec<SavedView>,
    },
    /// The saved views could not be written to their file.
    NotStored {
        /// Why, in plain words.
        reason: String,
    },
}

/// One saved view of the scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    /// What the person called it.
    pub name: String,
    /// The camera's turn about the head-to-foot axis, in radians.
    pub yaw: f64,
    /// How far the camera looks down on the head, in radians.
    pub pitch: f64,
    /// Millimetres of the patient to one screen pixel.
    pub mm_per_px: f64,
    /// The point of the patient in the middle of the scene.
    pub target: [f64; 3],
    /// The look: `Surface`, `Vr` or `Mip`. Anything else is read as `Surface`.
    pub look: String,
    /// The window's centre and width.
    pub window: [f32; 2],
    /// The cut planes, each as a point on it and its normal.
    pub cuts: Vec<([f64; 3], [f64; 3])>,
}

impl SavedView {
    /// The scene as it is now, seen through `window`, under a name. A name of nothing but
    /// spaces becomes "View" and `number`; a long one is cut at 40 characters.
    pub fn of(name: &str, number: usize, scene: &Scene, window: WindowLevel) -> Self {
        let name = match name.trim() {
            "" => format!("View {number}"),
            given => given.chars().take(MAX_NAME).collect(),
        };
        let at = |point: Point<_>| [point.x, point.y, point.z].map(finite);
        let camera = scene.camera;
        let look = match scene.look {
            Look::Surface => "Surface",
            Look::Vr => "Vr",
            Look::Mip => "Mip",
            Look::Parts => "Parts",
        };
        Self {
            name,
            yaw: finite(camera.yaw),
            pitch: finite(camera.pitch),
            mm_per_px: finite(camera.mm_per_px),
            target: at(camera.target),
            look: look.to_owned(),
            window: [window.center, window.width]
                .map(|part| if part.is_finite() { part } else { 0.0 }),
            cuts: scene
                .cuts
                .iter()
                .map(|cut| (at(cut.point), cut.normal.map(finite)))
                .collect(),
        }
    }

    /// Where the scene was seen from.
    pub fn camera(&self) -> Camera {
        let [x, y, z] = self.target;
        let (target, mm_per_px) = (Point::new(x, y, z), self.mm_per_px);
        Camera {
            target,
            yaw: self.yaw,
            pitch: self.pitch,
            mm_per_px,
        }
    }

    /// How the volume was turned into a picture.
    pub fn look(&self) -> Look {
        match self.look.as_str() {
            "Vr" => Look::Vr,
            "Mip" => Look::Mip,
            "Parts" => Look::Parts,
            _ => Look::Surface,
        }
    }

    /// The window it was seen through.
    pub fn window(&self) -> WindowLevel {
        let [center, width] = self.window;
        WindowLevel::new(center, width)
    }

    /// The planes it was cut open with.
    pub fn cuts(&self) -> Vec<CutPlane> {
        let plane = |([x, y, z], normal): &([f64; 3], [f64; 3])| CutPlane {
            point: Point::new(*x, *y, *z),
            normal: *normal,
        };
        self.cuts.iter().map(plane).collect()
    }
}

/// A number as it can be written to a file: what is not a number is written as 0, since the
/// file's format has no way to write it and one such would spoil the whole file.
fn finite(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// The name a series' views are filed under: 16 hexadecimal digits of the 64-bit FNV-1a hash
/// of its identifier. The identifier itself, which could point back to the scan, is not kept.
pub fn series_key(id: &SeriesId) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = id.0.bytes().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use voxa_render::NamedView;

    use super::*;
    use crate::testing::id;

    fn scene() -> Scene {
        let camera = Camera::looking(NamedView::Left, Point::new(1.0, -2.0, 3.5), 0.75);
        let camera = camera.orbited([33.0, -12.0]);
        let cuts = vec![CutPlane {
            point: Point::new(4.0, 5.0, 6.0),
            normal: [0.0, 0.6, -0.8],
        }];
        Scene {
            camera,
            look: Look::Mip,
            cuts,
            ..Scene::default()
        }
    }

    #[test]
    fn the_key_of_a_series_is_a_hash_that_does_not_give_the_identifier_away() {
        let key = series_key(&id("1.2.3"));
        assert_eq!(key, series_key(&id("1.2.3")), "the same every time");
        assert_ne!(key, series_key(&id("1.2.4")));
        assert!(
            key.len() == 16 && key.chars().all(|digit| digit.is_ascii_hexdigit()),
            "{key}"
        );
        assert!(!key.contains("1.2.3"));
        // The published test value of 64-bit FNV-1a for the one letter "a".
        assert_eq!(series_key(&id("a")), "af63dc4c8601ec8c");
    }

    #[test]
    fn a_saved_view_gives_back_everything_it_was_made_from() {
        let (scene, window) = (scene(), WindowLevel::new(321.0, 123.0));
        let view = SavedView::of("From the side", 1, &scene, window);
        assert_eq!(view.name, "From the side");
        assert_eq!(view.camera(), scene.camera);
        assert_eq!((view.look(), view.window()), (Look::Mip, window));
        assert_eq!(view.cuts(), scene.cuts);
    }

    #[test]
    fn every_look_is_kept_under_its_own_name() {
        for look in [Look::Surface, Look::Vr, Look::Mip, Look::Parts] {
            let scene = Scene { look, ..scene() };
            let view = SavedView::of("", 1, &scene, WindowLevel::new(1.0, 2.0));
            assert_eq!(view.look(), look);
        }
    }

    #[test]
    fn a_view_with_no_name_is_numbered_and_a_long_name_is_cut() {
        let (scene, window) = (scene(), WindowLevel::new(1.0, 2.0));
        assert_eq!(SavedView::of("   ", 7, &scene, window).name, "View 7");
        assert_eq!(SavedView::of("  Ear  ", 7, &scene, window).name, "Ear");
        let long = "é".repeat(60);
        assert_eq!(SavedView::of(&long, 1, &scene, window).name, "é".repeat(40));
    }

    #[test]
    fn a_view_never_holds_what_is_not_a_number() {
        let mut scene = scene();
        scene.camera.yaw = f64::NAN;
        scene.camera.target = Point::new(f64::INFINITY, 2.0, 3.0);
        scene.cuts = vec![CutPlane {
            point: Point::new(f64::NAN, 0.0, 0.0),
            normal: [f64::NAN, 1.0, 0.0],
        }];
        let view = SavedView::of("", 1, &scene, WindowLevel::new(f32::NAN, 9.0));
        assert_eq!(
            (view.yaw, view.target, view.window),
            (0.0, [0.0, 2.0, 3.0], [0.0, 9.0])
        );
        assert_eq!(view.cuts, [([0.0, 0.0, 0.0], [0.0, 1.0, 0.0])]);
        assert!(serde_json::to_string(&view).is_ok_and(|text| !text.contains("null")));
    }

    #[test]
    fn a_look_that_is_not_known_is_read_as_surface() {
        let window = WindowLevel::new(1.0, 2.0);
        let mut view = SavedView::of("", 1, &Scene::default(), window);
        assert_eq!(
            (view.look.as_str(), view.look()),
            ("Surface", Look::Surface)
        );
        view.look = "Something newer".to_owned();
        assert_eq!(view.look(), Look::Surface);
        view.look = "Vr".to_owned();
        assert_eq!(view.look(), Look::Vr);
    }
}

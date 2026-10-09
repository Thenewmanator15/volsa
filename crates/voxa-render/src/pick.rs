//! Finding the point of a volume a person is pointing at in a picture of it.

use voxa_core::{Patient, Point, Volume, WindowLevel};
use voxa_volume::{Ray, first_hit};

use crate::scene::{MAX_CUTS, SceneParams};
use crate::vector::{dot, unit};
use crate::volume_pipeline::{SURFACE_BEGINS, SURFACE_RAMP};

/// The least a line of sight must lean towards a plane to be taken as meeting it.
const ALONGSIDE: f64 = 1.0e-6;

/// The value at which tissue counts as a surface: half way through becoming solid in the
/// Surface look, which is 15% of the way up the window.
pub fn surface_threshold(window: WindowLevel) -> f32 {
    window.low() + window.width * (SURFACE_BEGINS + SURFACE_RAMP / 2.0)
}

/// The point of the patient shown at `at`, in pixels from the top left corner of a picture
/// of this width and height: the first surface the line of sight meets, a cut face that
/// shows tissue, or the slice drawn in the scene, whichever comes first. `None` where the
/// picture shows nothing.
pub fn pick(
    volume: &Volume,
    scene: &SceneParams,
    viewport: [f32; 2],
    at: [f32; 2],
) -> Option<Point<Patient>> {
    let sight = scene.camera.ray_through(viewport, at);
    let leaning = |normal: [f64; 3]| dot(normal, sight.direction);
    let ahead_of = |normal: [f64; 3], point: Point<Patient>| {
        let origin = sight.origin;
        dot(
            normal,
            [origin.x - point.x, origin.y - point.y, origin.z - point.z],
        )
    };
    // The line starts where it comes out of the removed side of the last cut it crosses, and
    // ends where it goes into the removed side of the first.
    let (mut start, mut end) = (0.0_f64, f64::INFINITY);
    for cut in scene.cuts.iter().take(MAX_CUTS) {
        let Some(normal) = unit(cut.normal) else {
            continue;
        };
        let (removed_by, leaning) = (ahead_of(normal, cut.point), leaning(normal));
        if leaning.abs() < ALONGSIDE {
            if removed_by > 0.0 {
                return None;
            }
        } else if leaning > 0.0 {
            end = end.min(-removed_by / leaning);
        } else {
            start = start.max(-removed_by / leaning);
        }
    }
    if end <= start {
        return None;
    }
    // The slice drawn in the scene is an opaque sheet: nothing behind it can be pointed at.
    let sheet = scene.slice.and_then(|slice| {
        let normal = unit(slice.normal)?;
        let leaning = leaning(normal);
        let meets = -ahead_of(normal, slice.point) / leaning;
        (leaning.abs() >= ALONGSIDE && meets >= start && meets <= end).then_some(meets)
    });
    let end = sheet.unwrap_or(end);
    let from_the_cuts = Ray {
        origin: sight.at(start),
        direction: sight.direction,
    };
    let surface = first_hit(volume, &from_the_cuts, surface_threshold(scene.window));
    let reached = surface.filter(|hit| {
        let from = from_the_cuts.origin;
        dot(
            sight.direction,
            [hit.x - from.x, hit.y - from.y, hit.z - from.z],
        ) <= end - start
    });
    reached.or_else(|| sheet.map(|sheet| sight.at(sheet)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::NamedView;
    use crate::scene::{CutPlane, Look, SlicePlane};
    use crate::testing::{ball, scene_of};

    const PICTURE: [f32; 2] = [96.0, 96.0];
    const MIDDLE: [f32; 2] = [48.0, 48.0];

    fn from_the_front(change: impl FnOnce(&mut SceneParams)) -> (Volume, SceneParams) {
        let volume = ball();
        let mut scene = scene_of(&volume, NamedView::Anterior, Look::Surface);
        change(&mut scene);
        (volume, scene)
    }

    fn cut(y: f64) -> CutPlane {
        CutPlane {
            point: Point::new(0.0, y, 0.0),
            normal: [0.0, -1.0, 0.0],
        }
    }

    #[test]
    fn the_surface_is_where_tissue_is_half_way_to_solid() {
        assert!((surface_threshold(WindowLevel::new(200.0, 400.0)) - 60.0).abs() < 1e-3);
        assert!((surface_threshold(WindowLevel::new(300.0, 200.0)) - 230.0).abs() < 1e-3);
    }

    #[test]
    fn pointing_at_the_ball_finds_its_surface_nearest_the_viewer() {
        let (volume, scene) = from_the_front(|_| {});
        let hit = pick(&volume, &scene, PICTURE, MIDDLE).expect("the ball is there");
        // The shell's value climbs from 0 at 16.5 mm out to 100 at 15.5 mm, and passes 60
        // at 15.9 mm.
        assert!((hit.y + 15.9).abs() < 0.05, "{hit:?}");
        assert!(hit.x.abs() < 1e-6 && hit.z.abs() < 1e-6, "{hit:?}");
        assert_eq!(
            pick(&volume, &scene, PICTURE, [2.0, 2.0]),
            None,
            "nothing in the corner"
        );
    }

    #[test]
    fn pointing_at_a_cut_face_finds_the_point_on_the_cut() {
        let (volume, scene) = from_the_front(|scene| scene.cuts = vec![cut(0.0)]);
        let hit = pick(&volume, &scene, PICTURE, MIDDLE).expect("the core is on the cut");
        assert!(hit.y.abs() < 1e-6, "{hit:?}");
    }

    #[test]
    fn where_a_cut_is_in_the_air_the_surface_behind_it_is_found() {
        let (volume, scene) = from_the_front(|scene| scene.cuts = vec![cut(-10.0)]);
        // 13.6 mm to the right of the patient's middle the ball begins 8.4 mm in front of it.
        let hit = pick(&volume, &scene, PICTURE, [23.5, 48.0]).expect("the ball is behind");
        assert!((hit.y + 8.6).abs() < 0.6, "{hit:?}");
    }

    #[test]
    fn what_a_cut_has_removed_cannot_be_pointed_at() {
        let off = |normal| CutPlane {
            point: Point::new(0.0, 0.0, 0.0),
            normal,
        };
        // The patient's left half is gone, and the line of sight runs alongside the cut.
        let (volume, scene) = from_the_front(|scene| scene.cuts = vec![off([1.0, 0.0, 0.0])]);
        assert_eq!(pick(&volume, &scene, PICTURE, [60.0, 48.5]), None);
        assert!(
            pick(&volume, &scene, PICTURE, [36.0, 48.5]).is_some(),
            "the right half is there"
        );
        // Front and back both gone: nothing is left on any line.
        let both = vec![off([0.0, -1.0, 0.0]), off([0.0, 1.0, 0.0])];
        let (volume, scene) = from_the_front(|scene| scene.cuts = both);
        assert_eq!(pick(&volume, &scene, PICTURE, [60.0, 48.5]), None);
    }

    #[test]
    fn the_slice_drawn_in_the_scene_is_found_where_it_is_in_front() {
        let sheet = |y: f64| SlicePlane {
            point: Point::new(0.0, y, 0.0),
            normal: [0.0, 1.0, 0.0],
        };
        let (volume, scene) = from_the_front(|scene| scene.slice = Some(sheet(-20.0)));
        let hit = pick(&volume, &scene, PICTURE, MIDDLE).expect("the sheet is in the way");
        assert!((hit.y + 20.0).abs() < 1e-6, "{hit:?}");
        let hit = pick(&volume, &scene, PICTURE, [2.0, 2.0]).expect("even where it is empty");
        assert!((hit.y + 20.0).abs() < 1e-6, "{hit:?}");
        // Behind the ball's front surface, the surface is found first.
        let (volume, scene) = from_the_front(|scene| scene.slice = Some(sheet(0.0)));
        let hit = pick(&volume, &scene, PICTURE, MIDDLE).expect("the ball is in front");
        assert!((hit.y + 15.9).abs() < 0.05, "{hit:?}");
    }
}

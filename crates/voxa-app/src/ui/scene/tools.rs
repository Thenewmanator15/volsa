//! Measuring in the 3D scene: what a click on a surface makes with the tool in hand, and the
//! series' annotations drawn where their points fall.

use egui::{Rect, Ui};
use voxa_core::{Patient, Point};
use voxa_render::{Camera, on_screen};
use voxa_volume::distance_mm;

use super::super::pen::Pen;
use super::super::planes::{Drawn, beside, paint};
use super::super::theme;
use crate::action::Action;
use crate::annotations::{Shape, Tool, measured};
use crate::planes::grid;
use crate::state::{AppState, Notice, NoticeKind};

/// The key of the notice about measuring in the scene.
const MEASURING: &str = "measuring";
/// What is said of a click that met nothing.
pub const NO_SURFACE: &str = "There is no surface there to measure from.";
/// What the scene says while a tool that only draws in the plane views is in hand.
pub const PLANE_TOOLS: &str = "Angles and regions are drawn in the plane views.";
/// What the scene says of the annotations drawn over it.
pub const IN_FRONT: &str =
    "Measurements are drawn in front of the picture, even where tissue would hide them.";
/// The shortest a ruler can be, in millimetres: a second click nearer than this to the first
/// is taken as the same click.
const LEAST_RULER_MM: f64 = 1.0;
/// How many straight pieces a region's outline is drawn in.
const PIECES: u8 = 48;

/// What a click in the scene means with the tool in hand. `surface` is the point of the
/// patient on the surface under the pointer, if there is one; `begun` is the first end of a
/// ruler already clicked. Gives the first end as it now stands, and what to do.
///
/// Navigating, a click on a surface moves the planes' position there. With the ruler, two
/// clicks on surfaces give the distance between them; with the marker, one click places it.
/// Where there is nothing under the pointer those two say so and place nothing. Angles and
/// regions are drawn in the plane views: here a click with them does nothing.
pub fn clicked(
    tool: Tool,
    begun: Option<Point<Patient>>,
    surface: Option<Point<Patient>>,
) -> (Option<Point<Patient>>, Vec<Action>) {
    match (tool, surface, begun) {
        (Tool::Navigate, Some(at), _) => (None, vec![Action::SetPosition(at)]),
        (Tool::Marker, Some(at), _) => (None, vec![add(Shape::Marker { at: xyz(at) })]),
        (Tool::Ruler, Some(at), None) => (Some(at), Vec::new()),
        (Tool::Ruler, Some(at), Some(from)) if distance_mm(from, at) >= LEAST_RULER_MM => {
            let ruler = Shape::Ruler {
                a: xyz(from),
                b: xyz(at),
            };
            (None, vec![add(ruler)])
        }
        (Tool::Ruler, Some(_), Some(from)) => (Some(from), Vec::new()),
        (Tool::Ruler | Tool::Marker, None, begun) => (begun, vec![Action::Post(no_surface())]),
        (Tool::Navigate | Tool::Angle | Tool::Region, _, _) => (None, Vec::new()),
    }
}

fn add(shape: Shape) -> Action {
    Action::AnnotationAdd(shape)
}

fn no_surface() -> Notice {
    Notice {
        kind: NoticeKind::Info,
        text: NO_SURFACE.to_owned(),
        details: Vec::new(),
        key: Some(MEASURING),
    }
}

/// Where an annotation falls in the scene's picture, in points from its top left corner: its
/// points as the camera sees them, whatever lies between them and the eye. A region, which
/// may be seen at any slant, is the outline of its ellipse.
pub fn scene_drawn(shape: &Shape, camera: &Camera, viewport: [f32; 2]) -> Drawn {
    let seen = |at: [f64; 3]| on_screen(camera, viewport, point(at));
    match shape {
        Shape::Ruler { a, b } => Drawn::Line([seen(*a), seen(*b)]),
        Shape::Angle { a, vertex, b } => Drawn::Arms([seen(*a), seen(*vertex), seen(*b)]),
        Shape::Marker { at } => Drawn::Dot(seen(*at)),
        Shape::Region {
            centre,
            across,
            down,
            radii_mm: [wide, high],
            ..
        } => {
            let round = (0..PIECES).map(|piece| {
                let turn = std::f64::consts::TAU * f64::from(piece) / f64::from(PIECES);
                let (along, up) = (wide * turn.cos(), high * turn.sin());
                let at = [0, 1, 2].map(|part| {
                    part_of(*centre, part)
                        + along * part_of(*across, part)
                        + up * part_of(*down, part)
                });
                seen(at)
            });
            Drawn::Outline(round.collect())
        }
    }
}

/// What the scene says about measuring: that annotations are drawn in front of everything,
/// when there are any, and that the tool in hand draws elsewhere, when it does.
pub fn said(state: &AppState) -> Vec<&'static str> {
    let plane_tool = matches!(state.tool, Tool::Angle | Tool::Region);
    let any = !state.annotations.items.is_empty();
    let lines = [(any, IN_FRONT), (plane_tool, PLANE_TOOLS)];
    lines
        .into_iter()
        .filter(|(shown, _)| *shown)
        .map(|(_, line)| line)
        .collect()
}

/// Draw the series' annotations over the scene's picture at `area`, each with its name and
/// what it measures, and the first end of a ruler that has been begun.
pub fn draw(ui: &Ui, area: Rect, state: &AppState, begun: Option<Point<Patient>>) {
    let pen = Pen::over(ui, area);
    let viewport = [area.width(), area.height()];
    let camera = &state.scene.camera;
    let placement = grid(state).map(|(placement, _)| placement);
    for item in &state.annotations.items {
        let value = measured(&item.shape, placement.as_ref()).value;
        let words = beside(item, &value);
        let drawn = scene_drawn(&item.shape, camera, viewport);
        paint(&pen, area, &drawn, theme::ACCENT, &words);
    }
    if let Some(from) = begun {
        let drawn = Drawn::Dot(on_screen(camera, viewport, from));
        paint(&pen, area, &drawn, theme::WARNING, "Click the other end");
    }
}

fn point([x, y, z]: [f64; 3]) -> Point<Patient> {
    Point::new(x, y, z)
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn part_of([x, y, z]: [f64; 3], part: usize) -> f64 {
    match part {
        0 => x,
        1 => y,
        _ => z,
    }
}

#[cfg(test)]
mod tests {
    use voxa_core::{Affine, Volume, WindowLevel};
    use voxa_render::{Look, NamedView, SceneParams, pick};

    use super::*;
    use crate::testing::thick_scan_ready;

    fn at(x: f64, y: f64, z: f64) -> Point<Patient> {
        Point::new(x, y, z)
    }

    #[test]
    fn navigating_a_click_on_a_surface_moves_the_position_there() {
        let here = at(1.0, 2.0, 3.0);
        assert_eq!(
            clicked(Tool::Navigate, None, Some(here)),
            (None, vec![Action::SetPosition(here)])
        );
        assert_eq!(
            clicked(Tool::Navigate, None, None),
            (None, vec![]),
            "nothing under the pointer"
        );
    }

    #[test]
    fn two_clicks_on_surfaces_with_the_ruler_give_the_distance_between_them() {
        let (first, second) = (at(0.0, 0.0, 0.0), at(3.0, 4.0, 0.0));
        assert_eq!(
            clicked(Tool::Ruler, None, Some(first)),
            (Some(first), vec![])
        );
        let ruler = Shape::Ruler {
            a: [0.0; 3],
            b: [3.0, 4.0, 0.0],
        };
        let finished = clicked(Tool::Ruler, Some(first), Some(second));
        assert_eq!(finished, (None, vec![Action::AnnotationAdd(ruler)]));
        // A second click in the same place is the same click.
        let again = clicked(Tool::Ruler, Some(first), Some(at(0.2, 0.0, 0.0)));
        assert_eq!(again, (Some(first), vec![]));
        // A click that meets nothing leaves the ruler as it is and says why.
        let (begun, asked) = clicked(Tool::Ruler, Some(first), None);
        assert_eq!(begun, Some(first));
        let [Action::Post(notice)] = &asked[..] else {
            panic!("{asked:?}");
        };
        assert_eq!(
            (notice.kind, notice.text.as_str()),
            (NoticeKind::Info, NO_SURFACE)
        );
        assert!(notice.key.is_some(), "said once, however often it happens");
    }

    #[test]
    fn a_marker_is_one_click_and_the_plane_tools_do_nothing_here() {
        let here = at(1.0, 2.0, 3.0);
        let marker = Shape::Marker {
            at: [1.0, 2.0, 3.0],
        };
        assert_eq!(
            clicked(Tool::Marker, None, Some(here)),
            (None, vec![Action::AnnotationAdd(marker)])
        );
        let (_, asked) = clicked(Tool::Marker, None, None);
        assert!(matches!(asked[..], [Action::Post(_)]), "{asked:?}");
        for tool in [Tool::Angle, Tool::Region] {
            assert_eq!(clicked(tool, None, Some(here)), (None, vec![]), "{tool:?}");
            assert_eq!(clicked(tool, None, None), (None, vec![]), "{tool:?}");
        }
    }

    /// A ball of value 100 and 16 mm radius in the middle of 40 voxels of 1 mm each way.
    fn ball() -> Volume {
        let side = 40_u32;
        let from_middle = |at: u32| (f64::from(at) - 19.5).powi(2);
        let value = |at: u32| {
            let (col, row, slice) = (at % side, at / side % side, at / (side * side));
            let far = (from_middle(col) + from_middle(row) + from_middle(slice)).sqrt();
            if far <= 16.0 { 100.0 } else { 0.0 }
        };
        let values = (0..side.pow(3)).map(value).collect();
        let placement = Affine::from_columns(
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-19.5; 3],
        );
        Volume::new([side; 3], values, placement).expect("adds up")
    }

    #[test]
    fn a_ruler_clicked_across_a_ball_measures_its_width() {
        let volume = ball();
        let viewport = [200.0, 200.0];
        let scene = SceneParams {
            camera: Camera::looking(NamedView::Anterior, at(0.0, 0.0, 0.0), 0.25),
            window: WindowLevel::new(50.0, 100.0),
            look: Look::Surface,
            cuts: Vec::new(),
            slice: None,
            quality: 1.0,
            skip_empty: false,
        };
        // A quarter of a millimetre to the point: the ball's edges are 64 points either side
        // of the middle. Clicked a little inside each, and once outside the ball altogether.
        let surface = |x: f32| pick(&volume, &scene, viewport, [x, 100.0]);
        let (left, right, beyond) = (surface(40.0), surface(160.0), surface(10.0));
        assert_eq!(beyond, None);
        let (begun, _) = clicked(Tool::Ruler, None, left);
        let (still, asked) = clicked(Tool::Ruler, begun, beyond);
        assert!(
            still.is_some() && matches!(asked[..], [Action::Post(_)]),
            "the ruler stays open"
        );
        let (_, asked) = clicked(Tool::Ruler, still, right);
        let [Action::AnnotationAdd(Shape::Ruler { a, b })] = asked[..] else {
            panic!("{asked:?}");
        };
        let across = distance_mm(point(a), point(b));
        assert!(
            (29.0..=32.5).contains(&across),
            "60 points apart, on the ball's curve: {across}"
        );
        let (_, asked) = clicked(Tool::Marker, None, left);
        let [Action::AnnotationAdd(Shape::Marker { at: [x, y, z] })] = asked[..] else {
            panic!("{asked:?}");
        };
        let from_middle = (x * x + y * y + z * z).sqrt();
        assert!(
            (from_middle - 16.0).abs() <= 1.0,
            "on the surface: {from_middle}"
        );
    }

    #[test]
    fn an_annotation_is_drawn_in_the_scene_as_the_camera_sees_it() {
        let viewport = [200.0, 100.0];
        let front = Camera::looking(NamedView::Anterior, at(0.0, 0.0, 0.0), 0.5);
        let ruler = Shape::Ruler {
            a: [-10.0, 0.0, 0.0],
            b: [10.0, 0.0, 0.0],
        };
        let Drawn::Line([[ax, ay], [bx, by]]) = scene_drawn(&ruler, &front, viewport) else {
            panic!("a ruler is a line");
        };
        assert!(
            (bx - ax - 40.0).abs() < 1.0e-3 && (ay - by).abs() < 1.0e-3,
            "20 mm across"
        );
        let side = Camera::looking(NamedView::Left, at(0.0, 0.0, 0.0), 0.5);
        let Drawn::Line([[ax, _], [bx, _]]) = scene_drawn(&ruler, &side, viewport) else {
            panic!("a ruler is a line");
        };
        assert!((bx - ax).abs() < 1.0e-3, "seen end on it has no length");
        let region = Shape::Region {
            centre: [0.0; 3],
            across: [1.0, 0.0, 0.0],
            down: [0.0, 0.0, -1.0],
            radii_mm: [10.0, 5.0],
            stats: None,
        };
        let Drawn::Outline(round) = scene_drawn(&region, &front, viewport) else {
            panic!("a region is an outline");
        };
        let widest = round.iter().map(|[x, _]| *x).fold(f32::MIN, f32::max);
        let highest = round.iter().map(|[_, y]| *y).fold(f32::MAX, f32::min);
        assert!(
            (widest - 120.0).abs() < 0.5 && (highest - 40.0).abs() < 0.5,
            "{widest}, {highest}"
        );
    }

    #[test]
    fn the_scene_says_that_measurements_are_in_front_and_which_tools_draw_elsewhere() {
        use crate::reducer::reduce;
        let mut state = thick_scan_ready();
        assert_eq!(said(&state), [] as [&str; 0]);
        reduce(&mut state, Action::SetTool(Tool::Angle));
        assert_eq!(said(&state), [PLANE_TOOLS]);
        reduce(
            &mut state,
            Action::AnnotationAdd(Shape::Marker { at: [0.0; 3] }),
        );
        assert_eq!(said(&state), [IN_FRONT, PLANE_TOOLS]);
        reduce(&mut state, Action::SetTool(Tool::Ruler));
        assert_eq!(said(&state), [IN_FRONT]);
    }
}

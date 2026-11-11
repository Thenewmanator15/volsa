//! What is written over a slice, and what the mouse and keys do to it: pure functions.

use egui::Key;
use voxa_core::{Affine, Patient, Point, SeriesInfo, StudyInfo, Voxel, WindowLevel};
use voxa_render::ViewTransform;

use super::browser::date_in_words;
use crate::action::Action;
use crate::scene::MainPanel;
use crate::state::Layout;

/// How much one wheel notch with Ctrl held magnifies.
pub(super) const ZOOM_PER_NOTCH: f32 = 1.1;
/// A drag of one screen pixel changes the window by this fraction of its width.
pub(super) const WINDOW_PER_PIXEL: f32 = 1.0 / 256.0;
/// The lengths a scale bar may have, in millimetres, longest first.
const BAR_LENGTHS_MM: [f32; 7] = [100.0, 50.0, 20.0, 10.0, 5.0, 2.0, 1.0];
/// Said in a corner when the files do not say where the slices are.
pub const NO_POSITION: &str = "No position data: orientation and scale are unavailable";

/// How many slices Page Up and Page Down step.
const PAGE: i32 = 10;
/// The keys egui itself answers to for the size of the interface, as the reference names them.
pub const SIZE_KEYS: &str = "Ctrl+= / Ctrl+- / Ctrl+0";

/// Every key Voxa answers to, with what it does, in the order the reference shows them.
pub const SHORTCUTS: [(&str, &str); 20] = [
    ("H", "Mirror the picture left to right"),
    ("V", "Mirror the picture top to bottom"),
    ("R", "Turn the picture a quarter turn clockwise"),
    (
        "F",
        "Fit the whole picture in the viewport, and the whole volume in 3D",
    ),
    ("1", "One viewport"),
    ("2", "Two viewports side by side"),
    ("4", "Four viewports"),
    ("Space", "Play or pause through the slices"),
    ("Up and Down", "The slice before and the slice after"),
    ("Page Up and Page Down", "Ten slices back and ten on"),
    ("Home and End", "The first slice and the last"),
    (
        "[ and ]",
        "The volume before and the volume after, of a series that holds several",
    ),
    ("M", "Open or close the measurements"),
    ("Ctrl+Z", "Take back the last change to the measurements"),
    ("Ctrl+Y", "Do it again (Ctrl+Shift+Z too)"),
    (
        "Esc",
        "Put the tool down; with none in hand, give 3D the large part of the window",
    ),
    ("B", "Show or hide the series list"),
    ("F11", "Full screen, or back to a window"),
    (
        SIZE_KEYS,
        "Draw the interface larger, smaller, or at its usual size",
    ),
    ("?", "Show this list"),
];

/// The letter at each edge of a viewport saying which way in the patient that edge lies:
/// `L`eft, `R`ight, `A`nterior (front), `P`osterior (back), `S`uperior (head), `I`nferior.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EdgeLabels {
    /// At the left edge of the screen.
    pub left: String,
    /// At the right edge.
    pub right: String,
    /// At the top edge.
    pub top: String,
    /// At the bottom edge.
    pub bottom: String,
}

/// The view a slice is drawn with: the person's own view, mirrored in neurological convention
/// along whichever way across the screen the patient's left and right lie. A picture with
/// front and back across it, or with no geometry, is the same in both conventions.
pub fn display_view(
    voxel_to_patient: Option<&Affine<Voxel, Patient>>,
    view: &ViewTransform,
    radiological: bool,
) -> ViewTransform {
    let (false, Some(placement)) = (radiological, voxel_to_patient) else {
        return *view;
    };
    let Some((rightwards, downwards)) = screen_directions(placement, view) else {
        return *view;
    };
    let left_to_right = |direction| matches!(letter(direction), "L" | "R");
    if left_to_right(rightwards) {
        view.flipped(true)
    } else if left_to_right(downwards) {
        view.flipped(false)
    } else {
        *view
    }
}

/// The letters for the four edges of a viewport showing a slice placed by `voxel_to_patient`
/// through `view`. Each is the patient axis that edge lies most nearly along, so a tilted
/// slice is lettered like the upright one it is closest to.
pub fn orientation_labels(
    voxel_to_patient: &Affine<Voxel, Patient>,
    view: &ViewTransform,
    radiological: bool,
) -> EdgeLabels {
    let shown = display_view(Some(voxel_to_patient), view, radiological);
    let Some((rightwards, downwards)) = screen_directions(voxel_to_patient, &shown) else {
        return EdgeLabels::default();
    };
    let towards = |direction: [f64; 3]| letter(direction).to_owned();
    let away_from = |direction: [f64; 3]| letter(direction.map(|part| -part)).to_owned();
    EdgeLabels {
        left: away_from(rightwards),
        right: towards(rightwards),
        top: away_from(downwards),
        bottom: towards(downwards),
    }
}

/// The directions in the patient that lie towards the right and towards the bottom of the
/// screen when a slice is shown through `view`.
fn screen_directions(
    placement: &Affine<Voxel, Patient>,
    view: &ViewTransform,
) -> Option<([f64; 3], [f64; 3])> {
    // Only directions matter, so any image and viewport will do.
    let to_voxel = view.image_to_screen([2, 2], [2.0, 2.0], 1.0).inverse()?;
    let in_patient = |x: f64, y: f64| {
        let at = placement.apply(to_voxel.apply(Point::new(x, y, 0.0)));
        [at.x, at.y, at.z]
    };
    let [ox, oy, oz] = in_patient(0.0, 0.0);
    let from_origin = |[x, y, z]: [f64; 3]| [x - ox, y - oy, z - oz];
    Some((
        from_origin(in_patient(1.0, 0.0)),
        from_origin(in_patient(0.0, 1.0)),
    ))
}

/// The letter for the way a direction in the patient points, by its largest part. Patient
/// coordinates follow DICOM: +x is the patient's left, +y the back, +z the head.
pub(super) fn letter([x, y, z]: [f64; 3]) -> &'static str {
    let (across, through, along) = (x.abs(), y.abs(), z.abs());
    if across >= through && across >= along {
        if x > 0.0 { "L" } else { "R" }
    } else if through >= along {
        if y > 0.0 { "P" } else { "A" }
    } else if z > 0.0 {
        "S"
    } else {
        "I"
    }
}

/// A scale bar for a picture in which one screen pixel spans `mm_per_screen_px`: its length
/// in screen pixels and its label. It is the longest of 1, 2, 5, 10, 20, 50 and 100 mm no
/// longer than `max_px`; `None` when not even a millimetre fits, or the scale is not known.
pub fn scale_bar(mm_per_screen_px: f32, max_px: f32) -> Option<(f32, String)> {
    if !mm_per_screen_px.is_finite() || mm_per_screen_px <= 0.0 {
        return None;
    }
    let fitting = BAR_LENGTHS_MM
        .iter()
        .find(|mm| **mm / mm_per_screen_px <= max_px)?;
    Some((fitting / mm_per_screen_px, format!("{fitting} mm")))
}

/// How far along the way the slices face a slice lies, in millimetres: the slice location.
pub fn slice_position(voxel_to_patient: &Affine<Voxel, Patient>, slice: usize) -> f64 {
    let [_, _, [nx, ny, nz]] = voxel_to_patient.columns();
    let length = (nx * nx + ny * ny + nz * nz).sqrt();
    let at = voxel_to_patient.apply(Point::new(0.0, 0.0, slice as f64));
    if length > 0.0 {
        (at.x * nx + at.y * ny + at.z * nz) / length
    } else {
        0.0
    }
}

/// The lines written in the four corners of a viewport: top left, top right, bottom left,
/// bottom right. The patient's name and identifier appear only when `show_patient` is true.
/// `position_mm` is the slice location, or `None` for a series without geometry, which then
/// says so.
pub fn corner_text(
    study: &StudyInfo,
    series: &SeriesInfo,
    slice: usize,
    position_mm: Option<f64>,
    wl: WindowLevel,
    show_patient: bool,
) -> [Vec<String>; 4] {
    let mut who_and_when = Vec::new();
    if show_patient {
        who_and_when.extend(study.patient_name.clone());
        who_and_when.extend(study.patient_id.clone());
    }
    who_and_when.extend(study.date.as_deref().and_then(date_in_words));
    if !study.description.is_empty() {
        who_and_when.push(study.description.clone());
    }
    let named = if series.description.is_empty() {
        format!("Series {}", series.number)
    } else {
        series.description.clone()
    };
    // Which volume of several, under the name: the corner below has the scale beside it.
    let what = std::iter::once(named)
        .chain(super::volumes::label(series))
        .collect();
    let place = match position_mm {
        Some(at) => format!("{at:.1} mm"),
        None => NO_POSITION.to_owned(),
    };
    let which = vec![format!("Slice {} of {}", slice + 1, series.slices), place];
    // A picture in colour is shown in its own colours, through no window.
    let window = if series.colour {
        Vec::new()
    } else {
        vec![format!("W {:.0}  L {:.0}", wl.width, wl.center)]
    };
    [who_and_when, what, which, window]
}

/// What the mouse did over one viewport since the last frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewportInput {
    /// Wheel notches turned: positive towards the person, which moves on to later slices.
    pub wheel: i32,
    /// Whether Ctrl was held as the wheel turned, which zooms instead.
    pub zooming: bool,
    /// Where the pointer is, in screen pixels from the middle of the viewport.
    pub pointer: [f32; 2],
    /// How far the pointer moved with the middle button held.
    pub middle_drag: [f32; 2],
    /// How far the pointer moved with the right button held.
    pub right_drag: [f32; 2],
    /// Whether the main button was double-clicked.
    pub double_clicked: bool,
    /// Whether any button went down.
    pub pressed: bool,
    /// The width of the window in use, which sets how fast a drag changes it.
    pub window_width: f32,
}

/// What the mouse asks for: a press makes the viewport active, a double-click fits the
/// picture, the wheel moves through the slices (or zooms about the pointer with Ctrl), a
/// middle drag pans, and a right drag changes the window: across for width, down for centre.
pub fn input_to_actions(viewport: usize, input: &ViewportInput) -> Vec<Action> {
    let mut actions = Vec::new();
    let moved = |[x, y]: [f32; 2]| x.abs() > 0.0 || y.abs() > 0.0;
    if input.pressed {
        actions.push(Action::SetActive(viewport));
    }
    if input.double_clicked {
        actions.push(Action::Fit { viewport });
    }
    if input.wheel != 0 && input.zooming {
        // Turning the wheel away magnifies.
        let factor = ZOOM_PER_NOTCH.powi(input.wheel.saturating_neg());
        actions.push(Action::Zoom {
            viewport,
            factor,
            about: input.pointer,
        });
    } else if input.wheel != 0 {
        actions.push(Action::Scroll {
            viewport,
            delta: input.wheel,
        });
    }
    if moved(input.middle_drag) {
        actions.push(Action::Pan {
            viewport,
            by: input.middle_drag,
        });
    }
    if moved(input.right_drag) {
        let per_pixel = input.window_width * WINDOW_PER_PIXEL;
        let [across, down] = input.right_drag;
        let (d_center, d_width) = (down * per_pixel, across * per_pixel);
        actions.push(Action::AdjustWindow {
            viewport,
            d_center,
            d_width,
        });
    }
    actions
}

/// Whether a key steps through the slices, and so steps again while it is held down.
pub fn steps_slices(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown
    )
}

/// What a key asks of the active viewport or the layout. `None` for a key Voxa does not use,
/// and for `?`, which opens the reference without changing any state. Down and Page Down
/// move on to later slices, as the wheel does turned towards the person; End asks for the
/// largest slice there is, which stops at the last.
pub fn shortcut_action(key: Key, active: usize) -> Option<Action> {
    let viewport = active;
    Some(match key {
        Key::H => Action::Flip {
            viewport,
            horizontal: true,
        },
        Key::V => Action::Flip {
            viewport,
            horizontal: false,
        },
        Key::R => Action::Rotate { viewport },
        Key::F => Action::Fit { viewport },
        Key::Num1 => Action::SetLayout(Layout::One),
        Key::Num2 => Action::SetLayout(Layout::TwoAcross),
        Key::Num4 => Action::SetLayout(Layout::TwoByTwo),
        Key::Space => Action::CineToggle { viewport },
        Key::ArrowDown => Action::Scroll { viewport, delta: 1 },
        Key::ArrowUp => Action::Scroll {
            viewport,
            delta: -1,
        },
        Key::PageDown => Action::Scroll {
            viewport,
            delta: PAGE,
        },
        Key::PageUp => Action::Scroll {
            viewport,
            delta: -PAGE,
        },
        Key::Home => Action::SetSlice { viewport, slice: 0 },
        Key::End => Action::SetSlice {
            viewport,
            slice: usize::MAX,
        },
        Key::OpenBracket => Action::StepVolume { viewport, by: -1 },
        Key::CloseBracket => Action::StepVolume { viewport, by: 1 },
        Key::Escape => Action::SetMain(MainPanel::Scene),
        _ => return None,
    })
}

#[cfg(test)]
mod slice_keys_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod volumes_tests;

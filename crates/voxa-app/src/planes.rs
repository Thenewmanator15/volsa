//! The three plane views as the application remembers them: the one point they all pass
//! through, how each panel is zoomed and moved, and what the column beside the 3D scene shows.

use voxa_core::{Affine, Patient, Point, SeriesId, Voxel};
use voxa_volume::{Plane, PlaneKind, plane_of, position_on_slice};

use crate::scene::{MainPanel, SceneStatus};
use crate::state::AppState;

/// How one plane's panel is zoomed and moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePanel {
    /// How much larger than fitted the plane is shown: 1 shows the whole volume.
    pub zoom: f64,
    /// The point of the plane in the middle of the panel, in millimetres across and down from
    /// the plane's own centre.
    pub offset_mm: [f64; 2],
}

impl Default for PlanePanel {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset_mm: [0.0; 2],
        }
    }
}

/// What the column beside the 3D scene shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnShows {
    /// The 2D viewports: the series' own slices.
    #[default]
    Slices,
    /// The axial, coronal and sagittal planes through the position.
    Planes,
}

/// The plane views.
#[derive(Debug, Clone, PartialEq)]
pub struct Planes {
    /// The point of the patient all three planes pass through. `None` until the scene's
    /// volume is ready.
    pub position: Option<Point<Patient>>,
    /// Each plane's panel, in the order of [`PlaneKind::ALL`].
    pub panels: [PlanePanel; 3],
    /// What the column beside the 3D scene shows.
    pub column: ColumnShows,
    /// Whether the 2D viewports of the same study follow the position.
    pub linked: bool,
    /// Whether the planes are drawn smoothed between voxels.
    pub smooth: bool,
}

impl Default for Planes {
    fn default() -> Self {
        Self {
            position: None,
            panels: [PlanePanel::default(); 3],
            column: ColumnShows::default(),
            linked: true,
            smooth: false,
        }
    }
}

impl Planes {
    /// The panel of one plane.
    pub fn panel(&self, kind: PlaneKind) -> PlanePanel {
        let [axial, coronal, sagittal] = self.panels;
        match kind {
            PlaneKind::Axial => axial,
            PlaneKind::Coronal => coronal,
            PlaneKind::Sagittal => sagittal,
        }
    }

    /// The panel of one plane, to change it.
    pub(crate) fn panel_mut(&mut self, kind: PlaneKind) -> &mut PlanePanel {
        let [axial, coronal, sagittal] = &mut self.panels;
        match kind {
            PlaneKind::Axial => axial,
            PlaneKind::Coronal => coronal,
            PlaneKind::Sagittal => sagittal,
        }
    }
}

/// Whether the plane views are on the screen: in the column beside the 3D scene, or with the
/// large part of the window.
pub fn in_sight(state: &AppState) -> bool {
    match state.main.given(&state.scene.status) {
        MainPanel::Planes => true,
        MainPanel::Scene => state.planes.column == ColumnShows::Planes,
        MainPanel::Stack => false,
    }
}

/// Whether the 2D viewports are on the screen: with the large part, or in the column when it
/// is not given to the planes or the parts.
pub fn slices_in_sight(state: &AppState) -> bool {
    let shown = (state.main.given(&state.scene.status), state.planes.column);
    matches!(
        shown,
        (MainPanel::Stack, _) | (_, ColumnShows::Slices) | (MainPanel::Planes, ColumnShows::Planes)
    )
}

/// Where the volume in the scene lies in the patient, and its columns, rows and slices.
/// `None` unless that volume is ready.
pub fn grid(state: &AppState) -> Option<(Affine<Voxel, Patient>, [u32; 3])> {
    if state.scene.status != SceneStatus::Ready {
        return None;
    }
    let info = &state.series(state.scene.series.as_ref()?)?.info;
    let slices = u32::try_from(info.slices).ok()?;
    Some((info.voxel_to_patient?, [info.cols, info.rows, slices]))
}

/// Where the series a viewport shows lies in the patient, if that viewport is linked to the
/// position: the link is on, there is a position, and the series is one of the study of the
/// scene's series whose files say where its slices are.
pub fn linked_placement(state: &AppState, at: usize) -> Option<Affine<Voxel, Patient>> {
    if !state.planes.linked || state.planes.position.is_none() {
        return None;
    }
    let shown = state.viewports.get(at)?.series.as_ref()?;
    let scene = state.scene.series.as_ref()?;
    let studies = &state.index.as_ref()?.studies;
    let together = studies.iter().any(|study| {
        let has = |id: &SeriesId| study.series.iter().any(|series| series.id == *id);
        has(shown) && has(scene)
    });
    let placement = state.series(shown)?.info.voxel_to_patient;
    placement.filter(|_| together)
}

/// The column and row at which the position falls on the slice a viewport shows, when the
/// viewport is linked and the position is within half a slice of that slice.
pub fn position_in_viewport(state: &AppState, at: usize) -> Option<[f64; 2]> {
    let placement = linked_placement(state, at)?;
    let slice = state.viewports.get(at)?.slice;
    let (on_slice, away) = position_on_slice(&placement, slice, state.planes.position?)?;
    (away.abs() <= 0.5).then_some(on_slice)
}

/// One of the three planes of the scene's volume through the position, facing as the
/// Radiological setting has it. `None` while there is no volume ready.
pub fn plane(state: &AppState, kind: PlaneKind) -> Option<Plane> {
    let (placement, dims) = grid(state)?;
    let position = state.planes.position?;
    plane_of(&placement, dims, kind, position, state.radiological)
}
